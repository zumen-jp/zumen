//! 寸法スタイルの置き換え・追加・除去を、寸法スタイルの表のレコード（DIMSTYLE）に書く（決定 0165）。
//!
//! 置き換えは、元のレコードの値のうち、名前（2）・倍率（40。DIMSCALE）・端部記号の大きさ（41。DIMASZ）・
//! 斜線の大きさ（142。DIMTSZ）と、端部記号が変わったときだけ、両端を別にする印（173。DIMSAH）と端部記号の
//! ブロックのレコードのハンドル（341〜344）を書き直す。追加は、Standard（なければ表の初めの子でない寸法スタイル）のレコードを写し、ハンドル・拡張辞書・
//! 拡張データを除いてから同じ欄を書く。名前を変えたら、DXF では、寸法・引出線・公差の寸法スタイルの名前（3）と、
//! 見出しの変数の寸法スタイルの名前（2。`$DIMSTYLE`）も書き直す。DWG の図形は寸法スタイルをハンドルで指すので、
//! 図形は書き直さない

use std::collections::BTreeSet;

use super::add::{Born, Target, int};
use super::layer::{external, set_in, usable_name};
use super::{count_transform_values, real, work};
use crate::db::{AcadDb, Data, Object, Section, Value};
use zumen_base::{Cx, Ref};
use zumen_drawing::{Arrowhead, DimensionStyle};
use zumen_sdk::Error;

/// 欄を書く順。ない欄は、この順で前にある欄の後に足す（DXF Reference「DIMSTYLE」の並び）
const ORDER: [i16; 10] = [2, 70, 40, 41, 142, 173, 341, 342, 343, 344];

/// 端部記号のブロックのレコードのハンドル（DIMLDRBLK・DIMBLK・DIMBLK1・DIMBLK2）
const ARROW_CODES: [i16; 4] = [341, 342, 343, 344];

/// 寸法スタイルを名前（3）で指す図形の種類
const DIMENSIONED: [&[u8]; 5] = [
    b"DIMENSION",
    b"ARC_DIMENSION",
    b"LARGE_RADIAL_DIMENSION",
    b"LEADER",
    b"TOLERANCE",
];

/// 寸法スタイルの表のレコードか
pub(super) fn is_dimension_style(object: &Object) -> bool {
    object.section == Section::Tables && object.kind == b"DIMSTYLE"
}

/// 名前を変えられず、除けない寸法スタイル（Standard）か。`$` を持つ名前は、寸法の種類ごとの子の寸法スタイル
/// なので、これも変えない
pub(super) fn fixed_name(name: &[u8]) -> bool {
    name.eq_ignore_ascii_case(b"Standard") || name.contains(&b'$')
}

/// 名前 `name` の寸法スタイルの、寸法の種類ごとの子の寸法スタイル（`name$0` など）が、名前の一覧 `names`
/// （小文字）にあるか
pub(super) fn has_children<'n>(mut names: impl Iterator<Item = &'n Vec<u8>>, name: &[u8]) -> bool {
    let prefix = [name.to_ascii_lowercase().as_slice(), b"$"].concat();
    names.any(|other| other.starts_with(&prefix))
}

/// 足すレコードの元にする値。Standard（なければ表の初めの子でない寸法スタイル）のレコードを写し、ハンドル（5・105）・区切り（100）・拡張辞書と反応子の組
/// （102）・拡張データ（1000 から）を除く。端部記号は、書くときに決める
pub(super) fn template(object: &Object) -> Vec<Value> {
    let mut values = Vec::new();
    let mut depth = 0usize;
    for value in &object.values {
        match (value.code, &value.data) {
            (102, Data::Text(text)) if text.starts_with(b"{") => depth += 1,
            (102, Data::Text(text)) if text.starts_with(b"}") => depth = depth.saturating_sub(1),
            (code, _)
                if depth > 0
                    || matches!(code, 5 | 105 | 173)
                    || ARROW_CODES.contains(&code)
                    || code >= 1000 => {}
            _ => values.push(value.clone()),
        }
    }
    values
}

/// ブロックのレコードの名前から、そのハンドルを引く
pub(super) type Blocks<'a> = &'a dyn Fn(&str) -> Option<u64>;

/// レコードの値 `values` に、寸法スタイル `style` の名前と定義を書く。`was` は置き換える元のレコードと寸法
/// スタイル（足すなら `None`）。`blocks` は、ブロックのレコードのハンドルを書けるとき（R2000 からの DXF）、
/// 名前からそのハンドルを引く。書けない値なら `None`
pub(super) fn write(
    born: &Born,
    values: &mut Vec<Value>,
    style: &DimensionStyle,
    was: Option<(&Object, &DimensionStyle)>,
    blocks: Option<Blocks<'_>>,
) -> Option<()> {
    usable_name(&style.name).then_some(())?;
    let name = born.text_value(2, &style.name)?;
    match was {
        Some((record, _)) if record.first(2) != Some(&name.data) => {
            // Standard・子の寸法スタイルと、外部参照に依る寸法スタイルは、名前を変えない
            let Some(Data::Text(old)) = record.first(2) else {
                return None;
            };
            (!fixed_name(old) && !external(record)).then_some(())?;
            (!style.name.contains('$')).then_some(())?;
        }
        Some(_) => {}
        None => (!style.name.contains('$')).then_some(())?,
    }
    set(values, 2, Some(name));
    let finite = |value: f64| value.is_finite() && value >= 0.0;
    (finite(style.size) && finite(style.scale)).then_some(())?;
    let tick = match style.tick {
        None => 0.0,
        Some(tick) => Some(tick).filter(|tick| tick.is_finite() && *tick > 0.0)?,
    };
    set(values, 40, Some(real(40, style.scale)?));
    set(values, 41, Some(real(41, style.size)?));
    set(values, 142, Some(real(142, tick)?));
    // 端部記号は、変わったときだけ書く。足すレコードは端部記号を持たない（既定の矢印）ところから書く。
    // ブロックのレコードのハンドルを書けなければ、変えられない
    const DEFAULTS: [Arrowhead; 2] = [Arrowhead::Default, Arrowhead::Default];
    let old = match was {
        Some((_, old)) => (&old.ends, &old.leader),
        None => (&DEFAULTS, &Arrowhead::Default),
    };
    if old == (&style.ends, &style.leader) {
        return Some(());
    }
    let blocks = blocks?;
    let handle = |code: i16, arrow: &Arrowhead| -> Option<Option<Value>> {
        match arrow {
            Arrowhead::Default => Some(None),
            Arrowhead::Symbol(key) if key.namespace == "dxf.block" => Some(Some(Value {
                code,
                data: Data::Handle(blocks(&key.name)?),
            })),
            Arrowhead::Symbol(_) => None,
        }
    };
    let [first, second] = &style.ends;
    if first == second {
        set(values, 173, Some(int(173, 0)));
        set(values, 342, handle(342, first)?);
        set(values, 343, None);
        set(values, 344, None);
    } else {
        set(values, 173, Some(int(173, 1)));
        set(values, 343, handle(343, first)?);
        set(values, 344, handle(344, second)?);
    }
    set(values, 341, handle(341, &style.leader)?);
    Some(())
}

fn set(values: &mut Vec<Value>, code: i16, value: Option<Value>) {
    set_in(values, code, value, &ORDER);
}

/// 寸法スタイルを名前で指すか。寸法・引出線・公差は 3 で、見出しの変数は 2 で指す
fn names(value: &Value, code: i16, name: &[u8]) -> bool {
    value.code == code && matches!(&value.data, Data::Text(text) if text.eq_ignore_ascii_case(name))
}

/// 寸法スタイルを名前（3）で書き直す図形か
fn dimensioned(object: &Object) -> bool {
    matches!(object.section, Section::Entities | Section::Blocks)
        && DIMENSIONED.contains(&object.kind.as_slice())
}

/// 3 で寸法スタイルを名前で指しうるオブジェクトか。寸法スタイルのレコードの 3 は寸法値の接頭辞と接尾辞、辞書
/// （DICTIONARY）の 3 は項目の名前で、寸法スタイルを指さない。辞書の項目はハンドル（350）で指す
fn may_name(object: &Object) -> bool {
    !is_dimension_style(object) && object.kind != b"DICTIONARY"
}

/// まだ表にない名前を図形や見出しが指しているか。新しい定義で既存の参照を変えないために調べる。
pub(super) fn name_in_use(db: &AcadDb, name: &[u8], cx: &mut Cx, at: &Ref) -> Result<bool, Error> {
    for object in db.objects() {
        work(cx, 1, at)?;
        if may_name(object) {
            count_transform_values(object, at, cx)?;
            if object.values.iter().any(|value| names(value, 3, name)) {
                return Ok(true);
            }
        }
    }
    for (variable, values) in db.variables() {
        for value in values {
            let bytes = match &value.data {
                Data::Text(bytes) | Data::Binary(bytes) => bytes.len(),
                _ => 0,
            };
            work(cx, 1 + bytes as u64 / 1024, at)?;
        }
        if variable.eq_ignore_ascii_case(b"$DIMSTYLE")
            && values.iter().any(|value| names(value, 2, name))
        {
            return Ok(true);
        }
    }
    Ok(false)
}

/// DXF の寸法・引出線・公差と寸法スタイルのレコードでないオブジェクトが、寸法スタイルの名前と同じ文字列（3）を
/// 持つか。3 はほかの意味にも使われるので、名前で指すかどうか分からないものも、指すとみなす
pub(super) fn named_elsewhere(
    db: &AcadDb,
    name: &[u8],
    cx: &mut Cx,
    at: &Ref,
) -> Result<bool, Error> {
    for object in db.objects() {
        work(cx, 1, at)?;
        if !dimensioned(object) && may_name(object) {
            count_transform_values(object, at, cx)?;
            if object.values.iter().any(|value| names(value, 3, name)) {
                return Ok(true);
            }
        }
    }
    Ok(false)
}

/// 名前を変えた寸法スタイル `old` を、名前 `new` で指すように書き直す。DXF は、寸法・引出線・公差の寸法
/// スタイル（3）と、見出しの変数の寸法スタイル（2）を書き直す。DWG は、図形がハンドルで指すので書き直さない。
/// ほかのオブジェクトが名前で指せば、呼ぶ側が先に拒む（[`named_elsewhere`]）
pub(super) fn rename(
    db: &mut AcadDb,
    old: &[u8],
    new: &[u8],
    target: Target,
    cx: &mut Cx,
) -> Result<(), Error> {
    if target != Target::Dxf {
        return Ok(());
    }
    let at = Ref::default();
    for position in 0..db.objects().len() {
        let Some(object) = db.object_mut(position) else {
            continue;
        };
        work(cx, 1, &at)?;
        count_transform_values(object, &at, cx)?;
        if !dimensioned(object) {
            continue;
        }
        for value in &mut object.values {
            if names(value, 3, old) {
                work(cx, 1 + new.len() as u64 / 1024, &at)?;
                value.data = Data::Text(new.to_vec());
            }
        }
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
        .filter(|(name, values)| {
            name.eq_ignore_ascii_case(b"$DIMSTYLE")
                && values.iter().any(|value| names(value, 2, old))
        })
        .map(|(name, values)| {
            let values = values
                .iter()
                .map(|value| match names(value, 2, old) {
                    true => Value {
                        code: value.code,
                        data: Data::Text(new.to_vec()),
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

/// 除く寸法スタイルのレコード `record`（表の位置と、一緒に除く拡張辞書などの位置を `skip` に持つ）を、ほかが指しているか。すべてのオブジェクトの
/// ハンドル（DWG の寸法・引出線、多重引出線のスタイル、DXF の表のオブジェクトの 340 など）と、寸法スタイルの
/// レコードでないオブジェクトの名前（3）と、今の寸法スタイル（`$DIMSTYLE`）を見る
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
    for (at, object) in db.objects().iter().enumerate() {
        work(cx, 1, place)?;
        if skip.contains(&at) {
            continue;
        }
        count_transform_values(object, place, cx)?;
        let by_name = may_name(object);
        if object.values.iter().any(|value| match &value.data {
            Data::Handle(found) => handle == Some(*found),
            _ => by_name && names(value, 3, name),
        }) {
            return Ok(true);
        }
    }
    for (variable, values) in db.variables() {
        for value in values {
            let bytes = match &value.data {
                Data::Text(bytes) | Data::Binary(bytes) => bytes.len(),
                _ => 0,
            };
            work(cx, 1 + bytes as u64 / 1024, place)?;
        }
        let dimstyle = variable.eq_ignore_ascii_case(b"$DIMSTYLE");
        if values.iter().any(|value| match &value.data {
            Data::Handle(found) => handle == Some(*found),
            _ => dimstyle && names(value, 2, name),
        }) {
            return Ok(true);
        }
    }
    Ok(false)
}

/// 除ける寸法スタイルのレコードか。Standard・子の寸法スタイル・外部参照に依るものと、ハンドルを持たない
/// レコードは除かない。拡張辞書は一緒に除く（決定 0170）
pub(super) fn removable(record: &Object) -> bool {
    matches!(record.first(2), Some(Data::Text(name)) if !fixed_name(name))
        && !external(record)
        && record.handle().is_some_and(|handle| handle != 0)
}
