//! レイヤの置き換えと追加を、画層の表のレコード（LAYER）に書く（決定 0162）。
//!
//! 置き換えは、元のレコードの値のうち、名前（2）・印（70）・色（62・420）・線種（6）・印刷の印（290）・
//! 線幅（370）だけを書き直す。追加は、画層 0 のレコードを写し、ハンドル・拡張辞書・拡張データを除いてから
//! 同じ欄を書く。名前を変えたら、DXF では、図形の画層の名前（8）と、XDATA の画層の名前（1003）と、
//! 今の画層（`$CLAYER`）も書き直す。DWG の図形は画層をハンドルで指すので、足した図形のほかは書き直さない

use std::collections::BTreeSet;

use super::add::{Born, Target, int, line_weight};
use super::{count_transform_values, work};
use crate::db::{AcadDb, Data, Object, Origin, Section, Value};
use zumen_base::{Cx, Ref};
use zumen_drawing::{ColorSystem, Inherit, Layer, LayerState, LayerStyle};
use zumen_sdk::Error;

/// 名前に使えない文字（AutoCAD の名前の決まり）
const FORBIDDEN: &[char] = &['<', '>', '/', '\\', '"', ':', ';', '?', '*', '|', '=', '`'];

/// 欄を書く順。ない欄は、この順で前にある欄の後に足す
const ORDER: [i16; 7] = [2, 70, 62, 420, 6, 290, 370];

/// 画層の表のレコードか
pub(super) fn is_layer(object: &Object) -> bool {
    object.section == Section::Tables && object.kind == b"LAYER"
}

/// レコードの名前のバイト列
pub(super) fn name_of(object: &Object) -> Option<&[u8]> {
    match object.first(2) {
        Some(Data::Text(name)) => Some(name),
        _ => None,
    }
}

/// 組んだ値の名前のバイト列
pub(super) fn value_name(values: &[Value]) -> Option<&[u8]> {
    match find(values, 2) {
        Some(Data::Text(name)) => Some(name),
        _ => None,
    }
}

/// DXF の OBJECTS のオブジェクト（画層の状態など）が、画層を名前（8）で指すか
pub(super) fn named_in_objects(
    db: &AcadDb,
    name: &[u8],
    cx: &mut Cx,
    at: &Ref,
) -> Result<bool, Error> {
    for object in db.objects() {
        work(cx, 1, at)?;
        if object.section == Section::Objects {
            count_transform_values(object, at, cx)?;
            if object.values.iter().any(|value| {
                value.code == 8
                    && matches!(&value.data, Data::Text(text) if text.eq_ignore_ascii_case(name))
            }) {
                return Ok(true);
            }
        }
    }
    Ok(false)
}

/// 名前が、AutoCAD の画層の名前に使えるか
pub(super) fn usable_name(name: &str) -> bool {
    !name.is_empty()
        && name.chars().count() <= 255
        && name.trim() == name
        && !name
            .chars()
            .any(|c| c.is_control() || FORBIDDEN.contains(&c))
}

/// 名前を変えられない画層（0 と Defpoints）か
pub(super) fn fixed_name(name: &[u8]) -> bool {
    name == b"0" || name.eq_ignore_ascii_case(b"Defpoints")
}

/// 外部参照に依る画層か（70 の 16、名前の `|`）
pub(super) fn external(object: &Object) -> bool {
    let flags = match object.first(70) {
        Some(Data::Int(flags)) => *flags,
        _ => 0,
    };
    flags & 16 != 0 || name_of(object).is_some_and(|name| name.contains(&b'|'))
}

/// 足すレコードの元にする値。画層 0 のレコードを写し、ハンドル（5）・拡張辞書と反応子の組（102）・
/// 拡張データ（1000 から）と真の色（420・430）を除く。DWG は、足せる欄だけを残す
pub(super) fn template(object: &Object, target: Target) -> Vec<Value> {
    let mut values = Vec::new();
    let mut depth = 0usize;
    for value in &object.values {
        match (value.code, &value.data) {
            (102, Data::Text(text)) if text.starts_with(b"{") => depth += 1,
            (102, Data::Text(text)) if text.starts_with(b"}") => depth = depth.saturating_sub(1),
            (code, _) if depth > 0 || code == 5 || code >= 1000 || code == 420 || code == 430 => {}
            (code, _)
                if matches!(target, Target::Dwg(_))
                    && !matches!(code, 330 | 2 | 70 | 62 | 290 | 370 | 390 | 347 | 6) => {}
            _ => values.push(value.clone()),
        }
    }
    values
}

/// レコードの値 `values` に、レイヤ `layer` の名前・状態・スタイルを書く。`was` は置き換える元のレコードと
/// レイヤ（足すなら `None`）。書けない値なら `None`
pub(super) fn write(
    born: &Born,
    values: &mut Vec<Value>,
    layer: &Layer,
    was: Option<(&Object, &Layer)>,
) -> Option<()> {
    let (state, style) = (layer.state?, layer.style?);
    usable_name(&layer.name).then_some(())?;
    // 凍結した画層は表示しない（読むときと同じ）
    (!state.frozen || !state.visible).then_some(())?;
    let dwg = matches!(born.target(), Target::Dwg(_));
    let name = born.text_value(2, &layer.name)?;
    if let Some((was, _)) = was
        && name_of(was) != Some(text_bytes(&name)?)
    {
        // 0 と Defpoints と、外部参照に依る画層は、名前を変えない
        (!fixed_name(name_of(was)?) && !external(was)).then_some(())?;
    }
    set(values, 2, Some(name));
    let flags = match find(values, 70) {
        Some(Data::Int(flags)) => *flags,
        _ => 0,
    };
    let flags = (flags & !(1 | 4)) | i64::from(state.frozen) | (i64::from(!state.editable) << 2);
    set(values, 70, Some(int(70, flags)));
    color(born, values, &state, &style)?;
    match was {
        // DWG の今あるレコードは、線種をハンドルで持つ。線種は変えられない
        Some((_, old)) if dwg => {
            (old.style.and_then(|style| style.line_type) == style.line_type).then_some(())?;
        }
        _ => set(values, 6, Some(born.line_type_value(6, style.line_type?)?)),
    }
    if state.printable {
        if find(values, 290).is_some() {
            set(values, 290, Some(bool_value(290, true)));
        }
    } else {
        born.dxf_since(b"AC1015").then_some(())?;
        set(values, 290, Some(bool_value(290, false)));
    }
    match style.line_weight {
        Some(weight) => {
            born.dxf_since(b"AC1015").then_some(())?;
            set(
                values,
                370,
                Some(int(370, line_weight(Inherit::Own(weight))?)),
            );
        }
        None => set(values, 370, None),
    }
    Some(())
}

/// 色（62）と真の色（420）。番号の色が要る。表示しない画層は、番号を負にする
fn color(
    born: &Born,
    values: &mut Vec<Value>,
    state: &LayerState,
    style: &LayerStyle,
) -> Option<()> {
    let color = style.color?;
    let code = color.code?;
    (code.system == ColorSystem::Aci && (1..=255).contains(&code.value)).then_some(())?;
    let index = i64::from(code.value);
    let index = if !state.visible && !state.frozen {
        -index
    } else {
        index
    };
    set(values, 62, Some(int(62, index)));
    match color.rgb {
        Some([r, g, b]) => {
            let rgb = i64::from(u32::from_be_bytes([0, r, g, b]));
            // DWG は、真の色を変えずに残すときだけ受ける
            let kept = find(values, 420) == Some(&Data::Int(rgb));
            (kept || (born.target() == Target::Dxf && born.dxf_since(b"AC1018"))).then_some(())?;
            set(values, 420, Some(int(420, rgb)));
        }
        None => set(values, 420, None),
    }
    Some(())
}

fn text_bytes(value: &Value) -> Option<&[u8]> {
    match &value.data {
        Data::Text(text) => Some(text),
        _ => None,
    }
}

fn bool_value(code: i16, value: bool) -> Value {
    Value {
        code,
        data: Data::Bool(value),
    }
}

/// 拡張データの前の、欄 `code` の値
pub(super) fn find(values: &[Value], code: i16) -> Option<&Data> {
    values
        .iter()
        .take_while(|value| value.code < 1000)
        .find(|value| value.code == code)
        .map(|value| &value.data)
}

/// 拡張データの前の、欄 `code` の値を `value` にする。なければ [`ORDER`] の順で足し、`None` なら除く
fn set(values: &mut Vec<Value>, code: i16, value: Option<Value>) {
    set_in(values, code, value, &ORDER);
}

/// 拡張データの前の、欄 `code` の値を `value` にする。なければ `order` の順で足し、`None` なら除く
pub(super) fn set_in(values: &mut Vec<Value>, code: i16, value: Option<Value>, order: &[i16]) {
    let end = values
        .iter()
        .position(|value| value.code >= 1000)
        .unwrap_or(values.len());
    let at = values.iter().take(end).position(|value| value.code == code);
    match (at, value) {
        (Some(at), Some(value)) => {
            if let Some(slot) = values.get_mut(at) {
                *slot = value;
            }
        }
        (Some(at), None) => {
            values.remove(at);
        }
        (None, Some(value)) => {
            let before = order.iter().take_while(|&&c| c != code);
            let after = before
                .filter_map(|&c| values.iter().take(end).rposition(|value| value.code == c))
                .max()
                .map_or(end, |at| at + 1);
            values.insert(after, value);
        }
        (None, None) => {}
    }
}

/// 名前を変えた画層 `old` を、名前 `new` で指すように書き直す。DXF は、図形の画層（8）、XDATA の画層の名前
/// （1003）、`$CLAYER` を書き直す。DWG は、足した図形の画層だけを書き直す。OBJECTS のオブジェクトが名前で指せば
/// （画層の状態など）なら、呼ぶ側が先に拒む（[`named_in_objects`]）
pub(super) fn rename(
    db: &mut AcadDb,
    old: &[u8],
    new: &Value,
    target: Target,
    cx: &mut Cx,
) -> Result<(), Error> {
    let at = Ref::default();
    let same = |data: &Data| matches!(data, Data::Text(name) if name.eq_ignore_ascii_case(old));
    for position in 0..db.objects().len() {
        let Some(object) = db.object_mut(position) else {
            continue;
        };
        work(cx, 1, &at)?;
        count_transform_values(object, &at, cx)?;
        let entity = matches!(object.section, Section::Entities | Section::Blocks);
        let rewrite = match target {
            Target::Dxf => true,
            Target::Dwg(_) => object.origin == Origin::New,
        };
        if !rewrite {
            continue;
        }
        for value in &mut object.values {
            let named =
                (value.code == 8 && entity) || (value.code == 1003 && target == Target::Dxf);
            if named && same(&value.data) {
                if let Data::Text(text) = &new.data {
                    work(cx, 1 + (text.len() as u64 / 1024), &at)?;
                }
                value.data = new.data.clone();
            }
        }
    }
    if let Some(values) = db.variable(b"$CLAYER") {
        for value in values {
            let bytes = match &value.data {
                Data::Text(bytes) | Data::Binary(bytes) => bytes.len(),
                _ => 0,
            };
            work(cx, 1 + bytes as u64 / 1024, &at)?;
        }
    }
    if target == Target::Dxf
        && let Some(values) = db.variable(b"$CLAYER")
        && values.iter().any(|value| same(&value.data))
    {
        let values = values
            .iter()
            .map(|value| match same(&value.data) {
                true => Value {
                    code: value.code,
                    data: new.data.clone(),
                },
                false => value.clone(),
            })
            .collect();
        db.set_variable(b"$CLAYER", values);
    }
    Ok(())
}

/// 除く画層のレコード `record`（表の位置と、一緒に除く拡張辞書などの位置を `skip` に持つ）を、ほかが指しているか。要素の画層（DXF は名前の 8、DWG はハンドル）、
/// XDATA の画層の名前（1003）、ほかのオブジェクトのハンドル（ビューポートの凍結・画層の状態・画層の索引など）、
/// 見出しの変数（今の画層）を見る。DWG の表のオブジェクト（TABLE）の項目の並びは、共有の表に置かれず、
/// 書き戻しが直す。それ以外の TABLE の値は検査する
pub(super) fn referenced(
    db: &AcadDb,
    skip: &BTreeSet<usize>,
    record: &Object,
    cx: &mut Cx,
    place: &Ref,
) -> Result<bool, Error> {
    let Some(name) = name_of(record) else {
        return Ok(true);
    };
    let handle = record.handle();
    let points = |value: &Value| match &value.data {
        Data::Handle(found) => handle == Some(*found),
        Data::Text(text) => matches!(value.code, 8 | 1003) && text.eq_ignore_ascii_case(name),
        _ => false,
    };
    for (at, object) in db.objects().iter().enumerate() {
        work(cx, 1, place)?;
        count_transform_values(object, place, cx)?;
        if !skip.contains(&at) && object.values.iter().any(points) {
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

/// 除ける画層のレコードか。0・Defpoints・外部参照に依る画層と、ハンドルを持たない（ID が並びの順で決まり、
/// 除くと後の画層の ID が変わる）レコードは除かない。拡張辞書は一緒に除く（決定 0170）
pub(super) fn removable(record: &Object) -> bool {
    name_of(record).is_some_and(|name| !fixed_name(name))
        && !external(record)
        && record.handle().is_some_and(|handle| handle != 0)
}

/// DXF の表 `table`（LAYER・STYLE）の終わり（ENDTAB）の位置。足すレコードは、その前に置く
pub(super) fn table_end(db: &AcadDb, table: &[u8]) -> Option<usize> {
    let objects = db.objects();
    let start = objects.iter().position(|object| {
        object.section == Section::Tables
            && object.kind == b"TABLE"
            && matches!(object.first(2), Some(Data::Text(name)) if name.eq_ignore_ascii_case(table))
    })?;
    objects
        .iter()
        .skip(start)
        .position(|object| object.kind == b"ENDTAB")
        .map(|at| start + at)
}

/// 足す表のレコード（LAYER・STYLE）
pub(super) fn record(kind: &[u8], values: Vec<Value>, handle: Option<u64>) -> Object {
    let values = match handle {
        Some(handle) => std::iter::once(Value {
            code: 5,
            data: Data::Handle(handle),
        })
        .chain(values)
        .collect(),
        None => values,
    };
    Object {
        kind: kind.to_vec(),
        section: Section::Tables,
        values,
        origin: Origin::New,
    }
}
