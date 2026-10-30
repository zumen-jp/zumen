//! 文字スタイルの置き換え・追加・除去を、文字スタイルの表のレコード（STYLE）に書く（決定 0163）。
//!
//! 置き換えは、元のレコードの値のうち、名前（2）・印（70 の 4）・高さ（40）・幅の倍率（41）・傾き（50）・
//! 文字の作り方（71 の 2・4）・主のフォント（3）・大きいフォント（4）だけを書き直す。追加は、Standard の
//! レコードを写し、ハンドル・拡張辞書・拡張データを除いてから同じ欄を書く。名前を変えたら、DXF では、図形の
//! 文字スタイルの名前（7）と、見出しの変数の文字スタイルの名前（7。`$TEXTSTYLE` など）も書き直す。DWG の
//! 図形は文字スタイルをハンドルで指すので、足した図形のほかは書き直さない

use std::collections::BTreeSet;

use super::add::{Born, Target, int};
use super::layer::{external, find, set_in, table_end, usable_name};
use super::{
    Current, Plan, count_table_records, count_table_search, count_transform_values, insert_records,
    real, record_at, removal, unsupported, work,
};
use crate::db::{AcadDb, Data, Object, Origin, Section, Value};
use zumen_base::{Cx, Key, Ref};
use zumen_drawing::{Drawing2D, Font, FontKind, Seen, TextStyle};
use zumen_sdk::Error;

/// 欄を書く順。ない欄は、この順で前にある欄の後に足す
const ORDER: [i16; 9] = [2, 70, 40, 41, 50, 71, 42, 3, 4];

/// DWG に足すレコードが持てる欄（決定 0094）
const DWG_CODES: [i16; 10] = [330, 2, 70, 40, 41, 50, 71, 42, 3, 4];

/// 傾きの大きさの上限（度）。AutoCAD が受け付ける範囲（推定）
const OBLIQUE_LIMIT: f64 = 85.0;

/// 文字スタイルの表のレコードか。形のファイルのレコード（70 の 1）は文字スタイルでない
pub(super) fn is_style(object: &Object) -> bool {
    object.section == Section::Tables && object.kind == b"STYLE" && flags(object, 70) & 1 == 0
}

/// 名前を変えられず、除けない文字スタイル（Standard）か
pub(super) fn fixed_name(name: &[u8]) -> bool {
    name.eq_ignore_ascii_case(b"Standard")
}

/// TrueType のファイルの名前か。読む側と同じく、名前で指すフォントとして読まれる
fn true_type_file(name: &str) -> bool {
    [".ttf", ".ttc", ".otf"].iter().any(|extension| {
        name.len() >= extension.len()
            && name
                .get(name.len() - extension.len()..)
                .is_some_and(|tail| tail.eq_ignore_ascii_case(extension))
    })
}

/// 主のフォントを 3 に書く値。形のファイルと、属性のない TrueType のファイルだけを書ける。フォントがなければ空
fn font_file(font: Option<&Font>) -> Option<&str> {
    match font {
        None => Some(""),
        Some(Font {
            name,
            kind: FontKind::Shape,
        }) if !true_type_file(name) => Some(name),
        Some(Font {
            name,
            kind:
                FontKind::Named {
                    bold: false,
                    italic: false,
                    charset: None,
                    pitch: None,
                },
        }) if true_type_file(name) => Some(name),
        Some(_) => None,
    }
}

/// 足すレコードの元にする値。Standard のレコードを写し、ハンドル（5）・拡張辞書と反応子の組（102）・
/// 拡張データ（1000 から）を除く。DWG は、足せる欄だけを残す
pub(super) fn template(object: &Object, target: Target) -> Vec<Value> {
    let mut values = Vec::new();
    let mut depth = 0usize;
    for value in &object.values {
        match (value.code, &value.data) {
            (102, Data::Text(text)) if text.starts_with(b"{") => depth += 1,
            (102, Data::Text(text)) if text.starts_with(b"}") => depth = depth.saturating_sub(1),
            (code, _) if depth > 0 || code == 5 || code >= 1000 => {}
            (code, _) if matches!(target, Target::Dwg(_)) && !DWG_CODES.contains(&code) => {}
            _ => values.push(value.clone()),
        }
    }
    values
}

/// レコードの値 `values` に、文字スタイル `style` の名前と定義を書く。`was` は置き換える元のレコードと
/// 文字スタイル（足すなら `None`）。書けない値なら `None`
pub(super) fn write(
    born: &Born,
    values: &mut Vec<Value>,
    style: &TextStyle,
    was: Option<(&Object, &TextStyle)>,
) -> Option<()> {
    let definition = style.definition.as_ref()?;
    usable_name(&style.name).then_some(())?;
    let name = born.text_value(2, &style.name)?;
    if let Some((record, _)) = was
        && record.first(2) != Some(&name.data)
    {
        // Standard と、外部参照に依る文字スタイルは、名前を変えない
        let Some(Data::Text(old)) = record.first(2) else {
            return None;
        };
        (!fixed_name(old) && !external(record)).then_some(())?;
    }
    set(values, 2, Some(name));
    // XDATA の書体の名前で指すフォントは、XDATA を書き直さないので変えられない
    let face = was.is_some_and(|(record, _)| crate::drawing::true_type_face(record).is_some());
    let old = was.and_then(|(_, old)| old.definition.as_ref());
    if face {
        (old.map(|old| &old.font) == Some(&definition.font)).then_some(())?;
    } else {
        let file = font_file(definition.font.as_ref())?;
        set(values, 3, Some(born.text_value(3, file)?));
    }
    let big = match &definition.big_font {
        None => "",
        Some(Font {
            name,
            kind: FontKind::Shape,
        }) => name,
        Some(_) => return None,
    };
    set(values, 4, Some(born.text_value(4, big)?));
    let height = match definition.height {
        None => 0.0,
        Some(height) => Some(height).filter(|height| *height > 0.0)?,
    };
    (definition.width > 0.0 && definition.oblique.abs() <= OBLIQUE_LIMIT).then_some(())?;
    set(values, 40, Some(real(40, height)?));
    set(values, 41, Some(real(41, definition.width)?));
    set(values, 50, Some(real(50, definition.oblique)?));
    let generation = (value_flags(values, 71) & !(2 | 4))
        | (i64::from(definition.backward) << 1)
        | (i64::from(definition.upside_down) << 2);
    set(values, 71, Some(int(71, generation)));
    let kind = (value_flags(values, 70) & !4) | (i64::from(definition.vertical) << 2);
    set(values, 70, Some(int(70, kind)));
    Some(())
}

fn set(values: &mut Vec<Value>, code: i16, value: Option<Value>) {
    set_in(values, code, value, &ORDER);
}

/// レコードの印の欄 `code` の値。なければ 0
fn flags(object: &Object, code: i16) -> i64 {
    match object.first(code) {
        Some(Data::Int(flags)) => *flags,
        _ => 0,
    }
}

/// 組んだ値の印の欄 `code` の値。なければ 0
fn value_flags(values: &[Value], code: i16) -> i64 {
    match find(values, code) {
        Some(Data::Int(flags)) => *flags,
        _ => 0,
    }
}

/// 文字スタイルを名前（7）で指すか
fn names(value: &Value, name: &[u8]) -> bool {
    value.code == 7 && matches!(&value.data, Data::Text(text) if text.eq_ignore_ascii_case(name))
}

/// DXF の図形でないオブジェクト（表の書式・寸法スタイルなど）が、文字スタイルを名前（7）で指すか
pub(super) fn named_elsewhere(
    db: &AcadDb,
    name: &[u8],
    cx: &mut Cx,
    at: &Ref,
) -> Result<bool, Error> {
    for object in db.objects() {
        work(cx, 1, at)?;
        if !matches!(object.section, Section::Entities | Section::Blocks) {
            count_transform_values(object, at, cx)?;
            if object.values.iter().any(|value| names(value, name)) {
                return Ok(true);
            }
        }
    }
    Ok(false)
}

/// 名前を変えた文字スタイル `old` を、名前 `new` で指すように書き直す。DXF は、図形の文字スタイル（7）と、
/// 見出しの変数の文字スタイル（7）を書き直す。DWG は、足した図形の文字スタイルだけを書き直す。図形でない
/// オブジェクトが名前で指せば、呼ぶ側が先に拒む（[`named_elsewhere`]）
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
        let rewrite = matches!(object.section, Section::Entities | Section::Blocks)
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

/// 除く文字スタイルのレコード `record`（表の位置と、一緒に除く拡張辞書などの位置を `skip` に持つ）を、ほかが指しているか。図形と表の書式などの
/// 文字スタイルの名前（7）、ほかのオブジェクトのハンドル（寸法スタイル・複合線種など）、見出しの変数を見る。
/// DWG の表のオブジェクト（TABLE）の項目の並びは、共有の表に置かれず、書き戻しが直す。それ以外の TABLE の
/// 値は検査する
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

/// 除ける文字スタイルのレコードか。Standard・外部参照に依るものと、ハンドルを持たないレコードは除かない。
/// 拡張辞書は一緒に除く（決定 0170）
pub(super) fn removable(record: &Object) -> bool {
    matches!(record.first(2), Some(Data::Text(name)) if !fixed_name(name))
        && !external(record)
        && record.handle().is_some_and(|handle| handle != 0)
}

/// 足した図形のうち、名前 `name` の文字スタイルを指すものがあるか
pub(super) fn named_by_new(db: &AcadDb, name: &[u8], cx: &mut Cx) -> Result<bool, Error> {
    let at = Ref::default();
    for object in db.objects() {
        work(cx, 1, &at)?;
        if object.origin == Origin::New
            && matches!(object.section, Section::Entities | Section::Blocks)
        {
            count_transform_values(object, &at, cx)?;
            if object.values.iter().any(|value| names(value, name)) {
                return Ok(true);
            }
        }
    }
    Ok(false)
}

/// 文字スタイルの変更（決定 0163）。表のレコードの位置と、小文字にした名前は、要るときに一度だけ作る
#[derive(Default)]
pub(super) struct TextStyles {
    records: Option<Vec<usize>>,
    names: BTreeSet<Vec<u8>>,
    added: Vec<Vec<Value>>,
    renames: Vec<(Vec<u8>, Value)>,
    removed: Vec<Vec<u8>>,
}

impl TextStyles {
    /// 文字スタイルの表のレコードの位置と、小文字にした名前を作る。作ってあれば何もしない
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
            if object.section == Section::Tables && object.kind == b"STYLE" {
                count_transform_values(object, &Ref::default(), cx)?;
                if let Some(Data::Text(name)) = object.first(2)
                    && !name.is_empty()
                {
                    self.names.insert(name.to_ascii_lowercase());
                }
                if is_style(object) {
                    records.push(position);
                }
            }
        }
        // 図形だけが指す名前も、既存の文字スタイルとして重複を防ぐ。
        for style in drawing
            .text_styles
            .iter()
            .filter(|style| style.definition.is_none())
        {
            work(cx, 1 + style.name.len() as u64 / 1024, &style.source)?;
            if let Some(Value {
                data: Data::Text(name),
                ..
            }) = born.text_value(2, &style.name)
            {
                self.names.insert(name.to_ascii_lowercase());
            }
        }
        self.records = Some(records);
        Ok(())
    }

    /// 足す文字スタイルを表に置き、名前を変えた文字スタイルを新しい名前で指す。除いた文字スタイルを、同じ
    /// 一覧で足した図形が指していないかを確かめる。足した文字スタイルのハンドルを返す
    pub(super) fn write_back(
        self,
        db: &mut AcadDb,
        target: Target,
        cx: &mut Cx,
    ) -> Result<Vec<Option<u64>>, Error> {
        let handles = insert_records(db, b"STYLE", self.added, target, cx)?;
        for (old, new) in &self.renames {
            rename(db, old, new, target, cx)?;
        }
        // 同じ一覧で足した図形は、除いた文字スタイルを指せない
        for name in &self.removed {
            if named_by_new(db, name, cx)? {
                cx.report(unsupported("remove_text_style", "STYLE", target));
                return Err(Error::Rejected);
            }
        }
        Ok(handles)
    }
}

/// 文字スタイルを置き換える（`Edit::ReplaceTextStyle`）
pub(super) fn replace<'e>(
    current: &Current<'_>,
    plan: &mut Plan<'e>,
    target_of: &'e Seen,
    new: &TextStyle,
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
    plan.text_styles.index(db, drawing, born, cx)?;
    work(cx, 1 + new.name.len() as u64 / 1024, &Ref::default())?;
    let Some(was) = plan.seen(&index.text_styles, target_of, index, cx)? else {
        return Ok(());
    };
    let styles = plan.text_styles.records.as_deref().unwrap_or_default();
    count_table_records(db, styles, &was.source, cx)?;
    let found = (plan.targets.insert(target_of.id.as_str()) && was.source != Ref::default())
        .then(|| record_at(db, styles, &was.source))
        .flatten();
    // DXF で名前を変えるなら、図形でないオブジェクトが名前で指していないかを先に調べる
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
        write(born, &mut values, new, Some((object, was)))?;
        let Some(Data::Text(old)) = object.first(2) else {
            return None;
        };
        let Some(Data::Text(name)) = find(&values, 2) else {
            return None;
        };
        if !old.eq_ignore_ascii_case(name) {
            // 名前を変えるなら、ほかの文字スタイルと重ならず、DXF の図形でないオブジェクトが名前で
            // 指していないこと
            let free = !plan.text_styles.names.contains(&name.to_ascii_lowercase()) && !named;
            free.then_some(())?;
            plan.text_styles.names.remove(&old.to_ascii_lowercase());
            plan.text_styles.names.insert(name.to_ascii_lowercase());
        }
        if old != name {
            let name = Value {
                code: 7,
                data: Data::Text(name.clone()),
            };
            plan.text_styles.renames.push((old.clone(), name));
        }
        Some((position, values))
    });
    match planned {
        Some(planned) => plan.replaced.push(planned),
        None => plan.reject(
            unsupported("replace_text_style", "STYLE", target).at(was.source.clone()),
            cx,
        ),
    }
    Ok(())
}

/// 文字スタイルを除く（`Edit::RemoveTextStyle`）
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
        target,
        ..
    } = *current;
    plan.text_styles.index(db, drawing, born, cx)?;
    let Some(was) = plan.seen(&index.text_styles, target_of, index, cx)? else {
        return Ok(());
    };
    // 後ろのハンドルなし文字スタイルは並びの番号が ID なので、除くと別の ID になる
    work(cx, drawing.text_styles.len() as u64, &was.source)?;
    let unstable = drawing
        .text_styles
        .iter()
        .skip_while(|style| !std::ptr::eq(*style, was))
        .skip(1)
        .any(|style| !matches!(style.source.key(), Some(Key::Handle(_))));
    let styles = plan.text_styles.records.as_deref().unwrap_or_default();
    count_table_records(db, styles, &was.source, cx)?;
    let found =
        (plan.targets.insert(target_of.id.as_str()) && was.source != Ref::default() && !unstable)
            .then(|| record_at(db, styles, &was.source))
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
                plan.text_styles.names.remove(&name.to_ascii_lowercase());
                plan.text_styles.removed.push(name.clone());
            }
            plan.removed.extend(skip);
        }
        None => plan.reject(
            unsupported("remove_text_style", "STYLE", target).at(was.source.clone()),
            cx,
        ),
    }
    Ok(())
}

/// 文字スタイルを足す（`Edit::AddTextStyle`）。Standard を写して書く
pub(super) fn add(
    current: &Current<'_>,
    plan: &mut Plan<'_>,
    new: &TextStyle,
    cx: &mut Cx,
) -> Result<(), Error> {
    let Current {
        db,
        drawing,
        born,
        target,
        ..
    } = *current;
    plan.text_styles.index(db, drawing, born, cx)?;
    work(cx, 1 + new.name.len() as u64 / 1024, &Ref::default())?;
    // 定義のない文字スタイルは表の後に並ぶため、表への追加でその ID がずれる。
    work(cx, drawing.text_styles.len() as u64, &Ref::default())?;
    let unstable = drawing
        .text_styles
        .iter()
        .any(|style| style.definition.is_none());
    let styles = plan.text_styles.records.as_deref().unwrap_or_default();
    count_table_records(db, styles, &Ref::default(), cx)?;
    let standard = styles.iter().find_map(|&position| {
        db.objects()
            .get(position)
            .filter(|object| matches!(object.first(2), Some(Data::Text(name)) if fixed_name(name)))
    });
    if let Some(standard) = standard {
        count_transform_values(standard, &Ref::default(), cx)?;
    }
    if target == Target::Dxf {
        count_table_search(db, cx)?;
    }
    let planned = standard.and_then(|standard| {
        let mut values = template(standard, target);
        write(born, &mut values, new, None)?;
        let Some(Data::Text(name)) = find(&values, 2) else {
            return None;
        };
        let name = name.to_ascii_lowercase();
        (!unstable
            && (target != Target::Dxf || table_end(db, b"STYLE").is_some())
            && plan.text_styles.names.insert(name))
        .then_some(values)
    });
    match planned {
        Some(values) => plan.text_styles.added.push(values),
        None => plan.reject(unsupported("add_text_style", "STYLE", target), cx),
    }
    Ok(())
}
