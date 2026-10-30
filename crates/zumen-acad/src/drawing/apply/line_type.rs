//! 線種の置き換え・追加・除去を、線種の表のレコード（LTYPE）に書く（決定 0164）。
//!
//! 置き換えは、元のレコードの値のうち、名前（2）・説明（3）・要素の数（73）・全体の長さ（40）・要素の長さ
//! （49）だけを書き直す。形か文字を持つレコードと、DWG の今あるレコードは、要素の数を変えず、要素の長さだけを
//! 順に書き直す。それ以外は、要素の値（49 と 74 から 9 まで）を除いてから、形も文字もない要素として書き直す。
//! 追加は、Continuous のレコードを写し、ハンドル・拡張辞書・拡張データと要素を除いてから同じ欄を書く。名前を
//! 変えたら、DXF では、図形と画層のレコードの線種の名前（6）と、見出しの変数の線種の名前（6。`$CELTYPE`）も
//! 書き直す。DWG の図形と画層は線種をハンドルで指すので、足した図形と画層のほかは書き直さない

use std::collections::BTreeSet;

use super::add::{Born, Target, int};
use super::layer::{external, find, set_in, table_end, usable_name};
use super::{
    Current, Plan, count_line_type_change, count_table_records, count_table_search,
    count_transform_values, insert_records, real, record_at, removal, unsupported, work,
};
use crate::db::{AcadDb, Data, Object, Origin, Section, Value};
use zumen_base::{Cx, Key, Ref};
use zumen_drawing::{Drawing2D, Edit, Inherit, LineType, PatternBasis, Placed, Seen};
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

/// 除く線種のレコード `record`（表の位置と、一緒に除く拡張辞書などの位置を `skip` に持つ）を、ほかが指しているか。図形と画層の線種の名前（6）、
/// ほかのオブジェクトのハンドル（DWG の図形と画層・寸法スタイルの線種など）、見出しの変数を見る。DWG の
/// 表のオブジェクト（TABLE）の項目の並びは、共有の表に置かれず、書き戻しが直す
pub(super) fn referenced(
    db: &AcadDb,
    skip: &BTreeSet<usize>,
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

/// 除ける線種のレコードか。ByLayer・ByBlock・Continuous・外部参照に依るものと、ハンドルを持たない
/// レコードは除かない。拡張辞書は一緒に除く（決定 0170）
pub(super) fn removable(record: &Object) -> bool {
    matches!(record.first(2), Some(Data::Text(name)) if !fixed_name(name))
        && !external(record)
        && record.handle().is_some_and(|handle| handle != 0)
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

/// 線種の変更（決定 0164）。表のレコードの位置と、小文字にした名前は、要るときに一度だけ作る
#[derive(Default)]
pub(super) struct LineTypes {
    records: Option<Vec<usize>>,
    names: BTreeSet<Vec<u8>>,
    added: Vec<Vec<Value>>,
    renames: Vec<(Vec<u8>, Value)>,
    removed: Vec<Vec<u8>>,
}

impl LineTypes {
    /// 線種の表のレコードの位置と、小文字にした名前を作る。作ってあれば何もしない
    fn index(
        &mut self,
        db: &AcadDb,
        drawing: &Drawing2D,
        born: &Born,
        cx: &mut Cx,
    ) -> Result<(), Error> {
        if self.records.is_some() {
            return Ok(());
        }
        let mut records = Vec::new();
        for (position, object) in db.objects().iter().enumerate() {
            work(cx, 1, &Ref::default())?;
            if is_line_type(object) {
                count_transform_values(object, &Ref::default(), cx)?;
                records.push(position);
                if let Some(Data::Text(name)) = object.first(2) {
                    self.names.insert(name.to_ascii_lowercase());
                }
            }
        }
        // 図形だけが指す名前も、既存の線種として重複を防ぐ。
        for line_type in drawing
            .line_types
            .iter()
            .filter(|line_type| line_type.definition.is_none())
        {
            work(
                cx,
                1 + line_type.name.len() as u64 / 1024,
                &line_type.source,
            )?;
            if let Some(Value {
                data: Data::Text(name),
                ..
            }) = born.text_value(2, &line_type.name)
            {
                self.names.insert(name.to_ascii_lowercase());
            }
        }
        self.records = Some(records);
        Ok(())
    }

    /// 足す線種を表に置き、名前を変えた線種を新しい名前で指す。除いた線種を、同じ一覧で足した図形と画層が
    /// 指していないかを確かめる。足した線種のハンドルを返す
    pub(super) fn write_back(
        self,
        db: &mut AcadDb,
        target: Target,
        cx: &mut Cx,
    ) -> Result<Vec<Option<u64>>, Error> {
        let handles = insert_records(db, b"LTYPE", self.added, target, cx)?;
        for (old, new) in &self.renames {
            rename(db, old, new, target, cx)?;
        }
        // 同じ一覧で足した図形と画層は、除いた線種を指せない
        for name in &self.removed {
            if named_by_new(db, name, cx)? {
                cx.report(unsupported("remove_line_type", "LTYPE", target));
                return Err(Error::Rejected);
            }
        }
        Ok(handles)
    }
}

/// 線種を置き換える（`Edit::ReplaceLineType`）
pub(super) fn replace<'e>(
    current: &Current<'_>,
    plan: &mut Plan<'e>,
    target_of: &'e Seen,
    new: &LineType,
    cx: &mut Cx,
) -> Result<(), Error> {
    let Current {
        db,
        drawing,
        index,
        born,
        target,
        ..
    } = *current;
    plan.line_types.index(db, drawing, born, cx)?;
    work(cx, 1 + new.name.len() as u64 / 1024, &Ref::default())?;
    count_line_type_change(new, cx)?;
    let Some(was) = plan.seen(&index.line_types, target_of, index, cx)? else {
        return Ok(());
    };
    let line_type_positions = plan.line_types.records.as_deref().unwrap_or_default();
    count_table_records(db, line_type_positions, &was.source, cx)?;
    let found = (plan.targets.insert(target_of.id.as_str()) && was.source != Ref::default())
        .then(|| record_at(db, line_type_positions, &was.source))
        .flatten();
    // DXF で名前を変えるなら、図形と画層でないオブジェクトが名前で指していないかを先に調べる
    let named = match found {
        Some((_, object)) => {
            count_transform_values(object, &was.source, cx)?;
            match (object.first(2), born.text_value(2, &new.name)) {
                (
                    Some(Data::Text(old)),
                    Some(Value {
                        data: Data::Text(name),
                        ..
                    }),
                ) if target == Target::Dxf && !old.eq_ignore_ascii_case(&name) => {
                    named_elsewhere(db, old, cx, &was.source)?
                }
                _ => false,
            }
        }
        None => false,
    };
    let planned = found.and_then(|(position, object)| {
        let mut values = object.values.clone();
        write(born, &mut values, new, Some((object, was)), target)?;
        let Some(Data::Text(old)) = object.first(2) else {
            return None;
        };
        let name = value_name(&values)?;
        if !old.eq_ignore_ascii_case(name) {
            // 名前を変えるなら、ほかの線種と重ならず、ほかのオブジェクトが名前で指していないこと
            let free = !plan.line_types.names.contains(&name.to_ascii_lowercase())
                && !fixed_name(name)
                && !named;
            free.then_some(())?;
            plan.line_types.names.remove(&old.to_ascii_lowercase());
            plan.line_types.names.insert(name.to_ascii_lowercase());
        }
        if old.as_slice() != name {
            let name = Value {
                code: 6,
                data: Data::Text(name.to_vec()),
            };
            plan.line_types.renames.push((old.clone(), name));
        }
        Some((position, values))
    });
    match planned {
        Some(planned) => plan.replaced.push(planned),
        None => plan.reject(
            unsupported("replace_line_type", "LTYPE", target).at(was.source.clone()),
            cx,
        ),
    }
    Ok(())
}

/// 線種を除く（`Edit::RemoveLineType`）
pub(super) fn remove<'e>(
    current: &Current<'_>,
    plan: &mut Plan<'e>,
    target_of: &'e Seen,
    cx: &mut Cx,
) -> Result<(), Error> {
    let Current {
        db,
        drawing,
        index,
        born,
        edits,
        target,
        ..
    } = *current;
    plan.line_types.index(db, drawing, born, cx)?;
    let Some(was) = plan.seen(&index.line_types, target_of, index, cx)? else {
        return Ok(());
    };
    // 同じ一覧で足す要素・スタイルを変える要素・変えて足すレイヤも、除く線種を指せない
    work(cx, edits.len() as u64, &was.source)?;
    let own = Inherit::Own(was.id);
    let pointed = edits.iter().any(|other| match other {
        Edit::Add { element, .. } => match element.as_ref() {
            Placed::Model(element) => element.style.line_type == own,
            Placed::Paper(element) => element.style.line_type == own,
        },
        Edit::Restyle { style, .. } => style.line_type == Some(own),
        Edit::ReplaceLayer { layer, .. } | Edit::AddLayer { layer } => layer
            .style
            .is_some_and(|style| style.line_type == Some(was.id)),
        _ => false,
    });
    // 後ろのハンドルなし線種は並びの番号が ID なので、除くと別の ID になる
    work(cx, drawing.line_types.len() as u64, &was.source)?;
    let unstable = drawing.line_types.iter().any(|line_type| {
        line_type.id.0 > was.id.0 && !matches!(line_type.source.key(), Some(Key::Handle(_)))
    });
    let line_type_positions = plan.line_types.records.as_deref().unwrap_or_default();
    count_table_records(db, line_type_positions, &was.source, cx)?;
    let found = (plan.targets.insert(target_of.id.as_str())
        && was.source != Ref::default()
        && !pointed
        && !unstable)
        .then(|| record_at(db, line_type_positions, &was.source))
        .flatten();
    let planned = match found {
        Some((position, object)) => {
            count_transform_values(object, &was.source, cx)?;
            match removable(object) {
                true => removal(db, position, object, referenced, cx, &was.source)?
                    .map(|skip| (skip, object)),
                false => None,
            }
        }
        None => None,
    };
    match planned {
        Some((skip, object)) => {
            if let Some(Data::Text(name)) = object.first(2) {
                plan.line_types.names.remove(&name.to_ascii_lowercase());
                plan.line_types.removed.push(name.clone());
            }
            plan.removed.extend(skip);
        }
        None => plan.reject(
            unsupported("remove_line_type", "LTYPE", target).at(was.source.clone()),
            cx,
        ),
    }
    Ok(())
}

/// 線種を足す（`Edit::AddLineType`）。Continuous を写して書く
pub(super) fn add(
    current: &Current<'_>,
    plan: &mut Plan<'_>,
    new: &LineType,
    cx: &mut Cx,
) -> Result<(), Error> {
    let Current {
        db,
        drawing,
        born,
        target,
        ..
    } = *current;
    plan.line_types.index(db, drawing, born, cx)?;
    work(cx, 1 + new.name.len() as u64 / 1024, &Ref::default())?;
    count_line_type_change(new, cx)?;
    // 定義のない線種は表の後に並ぶため、表への追加でその ID がずれる。
    work(cx, drawing.line_types.len() as u64, &Ref::default())?;
    let unstable = drawing
        .line_types
        .iter()
        .any(|line_type| line_type.definition.is_none());
    let line_type_positions = plan.line_types.records.as_deref().unwrap_or_default();
    count_table_records(db, line_type_positions, &Ref::default(), cx)?;
    let continuous = line_type_positions.iter().find_map(|&position| {
        db.objects().get(position).filter(|object| {
            matches!(object.first(2), Some(Data::Text(name)) if name.eq_ignore_ascii_case(b"Continuous"))
        })
    });
    if let Some(continuous) = continuous {
        count_transform_values(continuous, &Ref::default(), cx)?;
    }
    if target == Target::Dxf {
        count_table_search(db, cx)?;
    }
    let planned = continuous.and_then(|continuous| {
        let mut values = template(continuous, target);
        write(born, &mut values, new, None, target)?;
        let name = value_name(&values)?.to_ascii_lowercase();
        (!unstable
            && !fixed_name(&name)
            && (target != Target::Dxf || table_end(db, b"LTYPE").is_some())
            && plan.line_types.names.insert(name))
        .then_some(values)
    });
    match planned {
        Some(values) => plan.line_types.added.push(values),
        None => plan.reject(unsupported("add_line_type", "LTYPE", target), cx),
    }
    Ok(())
}
