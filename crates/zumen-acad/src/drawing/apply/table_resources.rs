//! 最初の表を足す DXF に、表が参照する辞書・表スタイルとクラスの宣言を用意する。

use super::add::{Target, int, next_free_handle, text};
use super::parts::find_standard_tablestyle;
use super::{Charset, work};
use crate::db::{AcadDb, Data, Object, Origin, Section, Value};
use zumen_base::{Cx, Ref};
use zumen_sdk::Error;

fn handle(code: i16, value: u64) -> Value {
    Value {
        code,
        data: Data::Handle(value),
    }
}

fn object(kind: &[u8], section: Section, values: Vec<Value>) -> Object {
    Object {
        kind: kind.to_vec(),
        section,
        values,
        origin: Origin::New,
    }
}

fn dictionary(id: u64, parent: u64) -> Object {
    object(
        b"DICTIONARY",
        Section::Objects,
        vec![
            handle(5, id),
            handle(330, parent),
            text(100, "AcDbDictionary"),
            int(281, 1),
        ],
    )
}

/// 辞書に同じ名前が重なるか、その参照が読めない場合は書き換えない。
fn entry(dict: &Object, name: &str) -> Result<Option<u64>, ()> {
    let mut found = None;
    for (i, value) in dict.values.iter().enumerate() {
        if value.code != 3
            || !matches!(&value.data,Data::Text(key) if key.eq_ignore_ascii_case(name.as_bytes()))
        {
            continue;
        }
        let Some(Value {
            code: 350 | 360,
            data: Data::Handle(target),
        }) = dict.values.get(i + 1)
        else {
            return Err(());
        };
        if *target == 0 || found.replace(*target).is_some() {
            return Err(());
        }
    }
    Ok(found)
}

fn attach(db: &mut AcadDb, parent: usize, name: &str, id: u64) -> Result<(), Error> {
    let parent = db.object_mut(parent).ok_or(Error::Invalid)?;
    let at = parent
        .values
        .iter()
        .position(|value| value.code == 1001)
        .unwrap_or(parent.values.len());
    parent
        .values
        .splice(at..at, [text(3, name), handle(350, id)]);
    Ok(())
}

fn standard(id: u64, owner: u64) -> Object {
    // DXF Reference「TABLESTYLE」。新しい無地の表は 3 種類の行に同じ文字と枠を使う。
    let number = |code, value| Value {
        code,
        data: Data::Real(value),
    };
    let mut values = vec![
        handle(5, id),
        text(102, "{ACAD_REACTORS"),
        handle(330, owner),
        text(102, "}"),
        handle(330, owner),
        text(100, "AcDbTableStyle"),
        int(280, 0),
        text(3, "Standard"),
        int(70, 0),
        int(71, 0),
        number(40, 0.5),
        number(41, 0.5),
        int(280, 1),
        int(281, 1),
    ];
    for _ in 0..3 {
        values.extend([text(7, "Standard"), number(140, 2.5), int(170, 5)]);
    }
    object(b"TABLESTYLE", Section::Objects, values)
}

/// 既存の参照先と未知の書式を保つ。新しい参照のハンドルは図面と HANDSEED の両方の後から取る。
fn prepare_style(db: &mut AcadDb, charset: Charset, cx: &mut Cx) -> Result<bool, Error> {
    let place = Ref::default();
    let mut root = None;
    for (index, item) in db.objects().iter().enumerate() {
        work(cx, 1, &place)?;
        if item.section == Section::Objects
            && item.kind == b"DICTIONARY"
            && item.owner().unwrap_or(0) == 0
        {
            if root.is_some() {
                return Ok(false);
            }
            root = Some(index);
        }
    }
    let mut next = next_free_handle(db, Target::Dxf);
    let mut alloc = || {
        let id = next?;
        next = id.checked_add(1);
        next.map(|_| id)
    };
    let root = if let Some(index) = root {
        index
    } else {
        // 辞書のない古いサンプルに、名前で一意に指せるスタイルがあれば従来どおり使う。
        if find_standard_tablestyle(db, charset, cx, &place)?.is_some() {
            return Ok(true);
        }
        if db.objects().iter().any(|o| o.section == Section::Objects) {
            return Ok(false);
        }
        let Some(id) = alloc() else { return Ok(false) };
        let index = db.objects().len();
        db.push(dictionary(id, 0));
        index
    };
    let Some(root_obj) = db.objects().get(root) else {
        return Ok(false);
    };
    let Some(root_id) = root_obj.handle().filter(|id| *id != 0) else {
        return Ok(false);
    };
    let group = match entry(root_obj, "ACAD_TABLESTYLE") {
        Err(()) => return Ok(false),
        Ok(Some(id)) => {
            let Some(index) = db.objects().iter().position(|o| {
                o.handle() == Some(id)
                    && o.kind == b"DICTIONARY"
                    && o.section == Section::Objects
                    && o.owner() == Some(root_id)
            }) else {
                return Ok(false);
            };
            index
        }
        Ok(None) => {
            let Some(id) = alloc() else { return Ok(false) };
            attach(db, root, "ACAD_TABLESTYLE", id)?;
            let index = db.objects().len();
            db.push(dictionary(id, root_id));
            index
        }
    };
    let Some(group_obj) = db.objects().get(group) else {
        return Ok(false);
    };
    let Some(owner) = group_obj.handle() else {
        return Ok(false);
    };
    match entry(group_obj, "Standard") {
        Err(()) => Ok(false),
        Ok(Some(id)) => Ok(db.get(id).is_some_and(|o| {
            o.section == Section::Objects && o.kind == b"TABLESTYLE" && o.owner() == Some(owner)
        })),
        Ok(None) => {
            let Some(id) = alloc() else { return Ok(false) };
            // 作った行書式から指す文字スタイルが必要。
            if !db.objects().iter().any(|o|o.section==Section::Tables && o.kind==b"STYLE" && matches!(o.first(2), Some(Data::Text(raw)) if raw.eq_ignore_ascii_case(b"Standard"))) {return Ok(false)}
            attach(db, group, "Standard", id)?;
            db.push(standard(id, owner));
            Ok(true)
        }
    }
}

const CLASSES: [(&str, &str, i64, i64); 2] = [
    ("ACAD_TABLE", "AcDbTable", 1025, 1),
    ("TABLESTYLE", "AcDbTableStyle", 4095, 0),
];

/// 書き込み前の表にだけ変更を加える。当てられない編集や中断では元の DXF に何も反映しない。
pub(super) fn prepare(db: &mut AcadDb, cx: &mut Cx) -> Result<bool, Error> {
    let charset = crate::drawing::charset_of(db);
    if !prepare_style(db, charset, cx)? {
        return Ok(false);
    }
    for (name, cpp, flags, entity) in CLASSES {
        let mut found = false;
        for o in db.objects() {
            work(cx, 1, &Ref::default())?;
            if o.section != Section::Classes
                || o.kind != b"CLASS"
                || !matches!(o.first(1), Some(Data::Text(s)) if s.eq_ignore_ascii_case(name.as_bytes()))
            {
                continue;
            }
            if found
                || !matches!(o.first(2), Some(Data::Text(s)) if s == cpp.as_bytes())
                || o.first(281) != Some(&Data::Int(entity))
            {
                return Ok(false);
            }
            found = true;
        }
        if !found {
            db.push(object(
                b"CLASS",
                Section::Classes,
                vec![
                    text(1, name),
                    text(2, cpp),
                    text(3, "ObjectDBX Classes"),
                    int(90, flags),
                    int(91, 0),
                    int(280, 0),
                    int(281, entity),
                ],
            ));
        }
    }
    Ok(true)
}

/// 追加と削除が同じ要求にあるときも、確定した表の実数を CLASS に書く。
pub(super) fn counts(db: &mut AcadDb, cx: &mut Cx) -> Result<(), Error> {
    for (name, _, _, _) in CLASSES {
        let mut count = 0i64;
        let mut class = None;
        for (index, o) in db.objects().iter().enumerate() {
            work(cx, 1, &Ref::default())?;
            if o.kind == name.as_bytes() {
                count += 1
            }
            if o.section == Section::Classes
                && o.kind == b"CLASS"
                && matches!(o.first(1), Some(Data::Text(raw)) if raw.eq_ignore_ascii_case(name.as_bytes()))
            {
                class = Some(index)
            }
        }
        if let Some(object) = class.and_then(|i| db.object_mut(i)) {
            if let Some(value) = object.values.iter_mut().find(|v| v.code == 91) {
                *value = int(91, count)
            } else if let Some(at) = object.values.iter().position(|v| v.code == 280) {
                object.values.insert(at, int(91, count))
            }
        }
    }
    Ok(())
}
