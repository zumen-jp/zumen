//! 図面モデルの変更を、共有の表を通して DXF・DWG に書き戻す（決定 0139）。

use std::collections::{BTreeMap, BTreeSet};

use super::{MODEL, PaperFrame, framed, mtext, source, text, work};
use add::{Born, Target, kind_of};

mod add;
use crate::db::{AcadDb, Data, Object, Section, Value};
use crate::{Dwg, DwgFile, Dxf, DxfFile};
use zumen_base::{Cx, Diagnostic, Diagnostics, Ref, codes};
use zumen_drawing::{
    Affine, Applied, Changes, Drawing2D, Edge, Edit, Element, FormattedText, Hash, Horizontal,
    LayerId, Length, ModelLength, ModelPoint, PaperLength, PaperPoint, Placed, Placement, Polyline,
    Restyle, Rotation, Shape, SheetId, SpaceId, Style, Symbol, Text, Vertical, id, transform,
};
use zumen_sdk::{Apply, Codec, Error};

impl Apply<Changes> for DxfFile {
    type Output = Vec<Applied>;

    /// 今のファイルから表を作り、変更を当ててから書き戻す
    fn apply(&mut self, changes: &Changes, cx: &mut Cx) -> Result<Vec<Applied>, Error> {
        let current = document(self, changes, Dxf::write, cx)?;
        let mut db = self.table(cx, false)?;
        let applied = edit(&mut db, changes, Target::Dxf, cx)?;
        if !current {
            return Err(Error::Rejected);
        }
        self.write_back(&db, cx)?;
        Ok(applied)
    }
}

impl Apply<Changes> for DwgFile {
    type Output = Vec<Applied>;

    /// 今のファイルから表を作り、変更を当ててから書き戻す
    fn apply(&mut self, changes: &Changes, cx: &mut Cx) -> Result<Vec<Applied>, Error> {
        let current = document(self, changes, Dwg::write, cx)?;
        let mut db = self.objects(cx)?;
        let seed = self.handle_seed();
        let applied = edit(&mut db, changes, Target::Dwg(seed), cx)?;
        if !current {
            return Err(Error::Rejected);
        }
        self.write_back(&db, cx)?;
        Ok(applied)
    }
}

/// 変更の一覧が文書のハッシュを持てば、今のファイルを書いたバイト列のハッシュと比べる。違えば診断を足して
/// `false` を返す。ほかの変更も確かめるので、ここでは止めない。変えていないファイルは元のバイトのまま書くので、
/// 読んだバイト列のハッシュと同じになる
fn document<F>(
    file: &F,
    changes: &Changes,
    write: fn(&F, &mut Vec<u8>, &mut Cx) -> Result<(), Error>,
    cx: &mut Cx,
) -> Result<bool, Error> {
    let Some(hash) = changes.document else {
        return Ok(true);
    };
    let mut bytes = Vec::new();
    write(file, &mut bytes, cx)?;
    let current = Hash::document_checked(&bytes, cx).map_err(|stop| {
        cx.report(stop.to_diagnostic());
        Error::Stopped(stop)
    })? == hash;
    if !current {
        cx.report(Diagnostic::new(codes::CHANGE_DOCUMENT_STALE));
    }
    Ok(current)
}

/// ID が指す要素
#[derive(Clone, Copy)]
enum Found<'a> {
    Model(&'a Element<ModelPoint>),
    Paper(&'a Element<PaperPoint>),
}

/// 足す先
#[derive(Clone, Copy, PartialEq)]
enum Place {
    Space(SpaceId),
    Sheet(SheetId),
}

/// ID から要素と足す先への索引（決定 0156）。図面モデルを 1 回なめて作る
struct Index<'a> {
    elements: BTreeMap<String, Found<'a>>,
    places: BTreeMap<String, Place>,
    /// 2 つ以上の要素を作った出所。1 つの要素だけを書き戻せない
    shared: BTreeSet<&'a Ref>,
}

impl<'a> Index<'a> {
    fn new(drawing: &'a Drawing2D, cx: &mut Cx<'_>) -> Result<Self, Error> {
        let mut elements = BTreeMap::new();
        let mut places = BTreeMap::new();
        let mut seen = BTreeSet::new();
        let mut shared = BTreeSet::new();
        let mut note = |source: &'a Ref| {
            if *source != Ref::default() && !seen.insert(source) {
                shared.insert(source);
            }
        };
        for space in &drawing.spaces {
            work(cx, 1, &Ref::default())?;
            let place = id::space(space.id);
            let mut namer = id::Namer::new(&place);
            for (index, element) in space.elements.iter().enumerate() {
                work(cx, 1, &element.source)?;
                note(&element.source);
                elements.insert(namer.next(index, element), Found::Model(element));
            }
            places.insert(place, Place::Space(space.id));
        }
        let mut sheet_namer = id::SheetNamer::default();
        for sheet in &drawing.sheets {
            work(cx, 1, &sheet.source)?;
            let place = sheet_namer.next(sheet);
            let mut namer = id::Namer::new(&place);
            for (index, element) in sheet.elements.iter().enumerate() {
                work(cx, 1, &element.source)?;
                note(&element.source);
                elements.insert(namer.next(index, element), Found::Paper(element));
            }
            places.insert(place, Place::Sheet(sheet.id));
        }
        Ok(Self {
            elements,
            places,
            shared,
        })
    }

    /// ID の要素の、今のハッシュ
    fn hash(&self, id: &str, cx: &mut Cx<'_>) -> Result<Option<Hash>, Error> {
        let Some(found) = self.elements.get(id) else {
            return Ok(None);
        };
        let (result, source) = match found {
            Found::Model(element) => (Hash::element_checked(element, cx), &element.source),
            Found::Paper(element) => (Hash::element_checked(element, cx), &element.source),
        };
        result.map(Some).map_err(|stop| {
            cx.report(stop.to_diagnostic().at(source.clone()));
            Error::Stopped(stop)
        })
    }
}

/// 当てた変更の、書き戻した後の ID とハッシュ。鍵（ハンドル）を持たない要素の ID は、読み直すとずれうる
/// ので返さない
fn applied(index: &Index, id: Option<String>, cx: &mut Cx<'_>) -> Result<Applied, Error> {
    let keyed = |id: &String| {
        index.elements.get(id).is_some_and(|found| {
            match found {
                Found::Model(element) => &element.source,
                Found::Paper(element) => &element.source,
            }
            .key()
            .is_some()
        })
    };
    let Some(id) = id.filter(keyed) else {
        return Ok(Applied {
            id: None,
            hash: None,
        });
    };
    let Some(hash) = index.hash(&id, cx)? else {
        return Ok(Applied {
            id: None,
            hash: None,
        });
    };
    Ok(Applied {
        id: Some(id),
        hash: Some(hash),
    })
}

/// 当てられない変更の診断
fn unsupported(edit: &str, kind: &str, target: Target) -> Diagnostic {
    let format = match target {
        Target::Dxf => "DXF",
        Target::Dwg(_) => "DWG",
    };
    Diagnostic::new(codes::CHANGE_EDIT_UNSUPPORTED)
        .with("edit", edit)
        .with("kind", kind)
        .with("format", format)
}

/// 変更を表に当てる。すべての変更を確かめ、当てられない変更が 1 つでもあれば、表を変えずに失敗する。
/// `target` は書き戻す先。当てたら、表から図面モデルを作り直し、変更ごとの新しい ID とハッシュを返す
fn edit(
    db: &mut AcadDb,
    changes: &Changes,
    target: Target,
    cx: &mut Cx,
) -> Result<Vec<Applied>, Error> {
    // ハンドルが重複すると、参照や ID が別のオブジェクトを指しうる。書き戻しは拒否する。
    let mut handles = BTreeSet::new();
    for object in db.objects() {
        work(cx, 1, &source(object))?;
        if let Some(handle) = object.handle()
            && handle != 0
            && !handles.insert(handle)
        {
            cx.report(unsupported("apply", "duplicate_handle", target).at(source(object)));
            return Err(Error::Rejected);
        }
    }
    // 今の図面モデル。変換の診断は、書き戻しの診断に混ぜない
    let mut quiet = Diagnostics::new();
    let (drawing, frames) = framed(db, &mut Cx::new(cx.budget, &mut quiet, cx.cancel))?;
    let index = Index::new(&drawing, cx)?;
    // 2007 からの版は文字を UTF-8 で持つ。それより前は、ASCII の文字だけを書く（読む側と同じ）
    let unicode = db
        .variable(b"$ACADVER")
        .and_then(|values| match values.first().map(|v| &v.data) {
            Some(Data::Text(text)) => Some(text.trim_ascii() >= super::UTF8),
            _ => None,
        })
        .unwrap_or(false);
    let born = Born::new(db, &drawing, unicode, target);
    let mut replaced = Vec::new();
    let mut removed = Vec::new();
    let mut added = Vec::new();
    let mut added_records = 0u64;
    let mut rejected = false;
    let mut targets = BTreeSet::new();
    let missing = |id: &str, cx: &mut Cx| {
        cx.report(Diagnostic::new(codes::CHANGE_ELEMENT_MISSING).with("id", id));
    };
    for change in &changes.edits {
        let (edit, target_of) = match change {
            Edit::Add { place, element } => {
                let Some(&place) = index.places.get(place) else {
                    missing(place, cx);
                    rejected = true;
                    continue;
                };
                let (record, kind) = match (place, element.as_ref()) {
                    (Place::Space(space), Placed::Model(element)) => {
                        add::work(&element.shape, cx, &Ref::default())?;
                        let record = (space == MODEL)
                            .then(|| {
                                born.record(element, &InSpace(MODEL), born.model(), added_records)
                            })
                            .flatten();
                        (record, kind_of(&element.shape))
                    }
                    (Place::Sheet(sheet), Placed::Paper(element)) => {
                        add::work(&element.shape, cx, &Ref::default())?;
                        let record = frames.get(&sheet).and_then(|&frame| {
                            born.record(element, &OnSheet(sheet, frame), frame.home, added_records)
                        });
                        (record, kind_of(&element.shape))
                    }
                    (_, Placed::Model(element)) => (None, kind_of(&element.shape)),
                    (_, Placed::Paper(element)) => (None, kind_of(&element.shape)),
                };
                match record {
                    Some(record) => {
                        added_records += record.count();
                        added.push(record);
                    }
                    None => {
                        cx.report(unsupported("add", kind, target));
                        rejected = true;
                    }
                }
                continue;
            }
            Edit::Replace {
                target: target_of, ..
            } => ("replace", target_of),
            Edit::Restyle {
                target: target_of, ..
            } => ("restyle", target_of),
            Edit::Transform {
                target: target_of, ..
            } => ("transform", target_of),
            Edit::Remove { target: target_of } => ("remove", target_of),
        };
        let Some(&found) = index.elements.get(&target_of.id) else {
            missing(&target_of.id, cx);
            rejected = true;
            continue;
        };
        if index.hash(&target_of.id, cx)? != Some(target_of.hash) {
            cx.report(
                Diagnostic::new(codes::CHANGE_ELEMENT_STALE).with("id", target_of.id.as_str()),
            );
            rejected = true;
            continue;
        }
        let at = match found {
            Found::Model(element) => &element.source,
            Found::Paper(element) => &element.source,
        };
        if !targets.insert(target_of.id.as_str()) {
            let kind = match found {
                Found::Model(element) => kind_of(&element.shape),
                Found::Paper(element) => kind_of(&element.shape),
            };
            cx.report(unsupported(edit, kind, target).at(at.clone()));
            rejected = true;
            continue;
        }
        let object = (!index.shared.contains(at))
            .then(|| entity(db, at))
            .flatten()
            .and_then(|position| Some((position, db.objects().get(position)?)));
        let Some((position, object)) = object else {
            let kind = match found {
                Found::Model(element) => kind_of(&element.shape),
                Found::Paper(element) => kind_of(&element.shape),
            };
            cx.report(unsupported(edit, kind, target).at(at.clone()));
            rejected = true;
            continue;
        };
        let reshaped = match (change, found) {
            (Edit::Replace { element, .. }, Found::Model(was)) => {
                matches!(element.as_ref(), Placed::Model(element) if element.shape != was.shape)
            }
            (Edit::Replace { element, .. }, Found::Paper(was)) => {
                matches!(element.as_ref(), Placed::Paper(element) if element.shape != was.shape)
            }
            _ => false,
        };
        if reshaped && let Some(code) = moving_xdata(object) {
            cx.report(
                Diagnostic::new(codes::CHANGE_XDATA_UNSUPPORTED)
                    .with("kind", text(&object.kind))
                    .with("code", i64::from(code))
                    .at(at.clone()),
            );
            rejected = true;
            continue;
        }
        let planned = match (change, found) {
            (Edit::Replace { element, .. }, Found::Model(was)) => match element.as_ref() {
                Placed::Model(element) => anchor(&was.shape)
                    .and_then(|anchor| {
                        replacement(was, element, object, unicode, &InSpace(anchor.space))
                    })
                    .map(|values| replaced.push((position, values))),
                Placed::Paper(_) => None,
            },
            (Edit::Replace { element, .. }, Found::Paper(was)) => match element.as_ref() {
                Placed::Paper(element) => anchor(&was.shape)
                    .and_then(|anchor| {
                        let frame = OnSheet(anchor.sheet, *frames.get(&anchor.sheet)?);
                        replacement(was, element, object, unicode, &frame)
                    })
                    .map(|values| replaced.push((position, values))),
                Placed::Model(_) => None,
            },
            (Edit::Restyle { style, .. }, found) => {
                let (layer, was) = match found {
                    Found::Model(element) => (element.layer, &element.style),
                    Found::Paper(element) => (element.layer, &element.style),
                };
                restyled(&born, object, layer, was, style, target, cx)?
                    .map(|values| replaced.push((position, values)))
            }
            (Edit::Transform { transform, .. }, Found::Model(was)) => anchor(&was.shape)
                .and_then(|anchor| {
                    let frame = InSpace(anchor.space);
                    moved(db, was, object, transform, unicode, &frame, true)
                })
                .map(|values| {
                    replaced.push((position, values.object));
                    replaced.extend(values.followers);
                }),
            (Edit::Transform { transform, .. }, Found::Paper(was)) => anchor(&was.shape)
                .and_then(|anchor| {
                    let frame = OnSheet(anchor.sheet, *frames.get(&anchor.sheet)?);
                    moved(db, was, object, transform, unicode, &frame, false)
                })
                .map(|values| {
                    replaced.push((position, values.object));
                    replaced.extend(values.followers);
                }),
            (Edit::Remove { .. }, _) => (!followed(object)).then(|| removed.push(position)),
            (Edit::Add { .. }, _) => None,
        };
        if planned.is_none() {
            cx.report(unsupported(edit, &text(&object.kind), target).at(at.clone()));
            rejected = true;
        }
    }
    if rejected {
        return Err(Error::Rejected);
    }
    let insertion_work = born.insertion_work(&added, db.objects().len());
    if insertion_work > 0 {
        work(cx, insertion_work, &Ref::default())?;
    }
    for (position, values) in replaced {
        db.object_mut(position).ok_or(Error::Rejected)?.values = values;
    }
    removed.sort_unstable();
    removed.dedup();
    for position in removed.into_iter().rev() {
        db.remove(position);
    }
    let mut handles = born.push(db, added).into_iter();
    // 書き戻した表から図面モデルを作り直し、置き換えた要素と足した要素の ID とハッシュを求める。ID は
    // ハンドルで作るので、読み直したファイルと同じになる
    let mut quiet = Diagnostics::new();
    let (after, _) = framed(db, &mut Cx::new(cx.budget, &mut quiet, cx.cancel))?;
    let after = Index::new(&after, cx)?;
    changes
        .edits
        .iter()
        .map(|change| match change {
            Edit::Replace { target, .. }
            | Edit::Restyle { target, .. }
            | Edit::Transform { target, .. } => applied(&after, Some(target.id.clone()), cx),
            Edit::Add { place, .. } => applied(
                &after,
                handles
                    .next()
                    .flatten()
                    .map(|handle| format!("{place}/h{handle:X}")),
                cx,
            ),
            Edit::Remove { .. } => applied(&after, None, cx),
        })
        .collect()
}

/// スタイルを変えた図形の値。変わった欄（レイヤ 8・線種 6・色 62 と 420 と 430・線幅 370・線種の倍率 48）
/// だけを、図形に共通の値の中で除いてから、足すときと同じ書き方（[`Born::entity`]）でレイヤの後に書く。
/// ほかの値は元のまま写す。DWG は、今ある図形のレイヤと線種を書き戻せないので扱わない
fn restyled(
    born: &Born,
    object: &Object,
    layer: LayerId,
    style: &Style,
    change: &Restyle,
    target: Target,
    cx: &mut Cx<'_>,
) -> Result<Option<Vec<Value>>, Error> {
    let at = source(object);
    let (new_layer, new) = change.over(layer, style);
    let mut changed = Vec::new();
    if new_layer != layer {
        changed.push(8);
    }
    if new.line_type != style.line_type {
        changed.push(6);
    }
    if new.color != style.color {
        changed.extend([62, 420, 430]);
    }
    if new.line_weight != style.line_weight {
        changed.push(370);
    }
    if new.line_type_scale != style.line_type_scale {
        changed.push(48);
    }
    if changed.is_empty() {
        return object
            .values
            .iter()
            .map(|value| copy_value(value, cx, &at))
            .collect::<Result<Vec<_>, _>>()
            .map(Some);
    }
    if matches!(target, Target::Dwg(_)) && changed.iter().any(|code| matches!(code, 6 | 8)) {
        return Ok(None);
    }
    let mut written = Vec::new();
    if changed.contains(&8) {
        let Some(layer) = born.layer_value(new_layer) else {
            return Ok(None);
        };
        written.push(layer);
    }
    let Some(style) = born.style_values(&new, &changed) else {
        return Ok(None);
    };
    written.extend(style);
    // 図形に共通の値の範囲。区切り（100）を持てば AcDbEntity の区切りの中、持たなければ XDATA の前まで
    let mut marker = None;
    for (index, value) in object.values.iter().enumerate() {
        work(cx, 1, &at)?;
        if value.code == 100 && value.data == Data::Text(b"AcDbEntity".to_vec()) {
            marker = Some(index);
            break;
        }
    }
    let start = marker.map_or(0, |index| index + 1);
    let mut end = object.values.len();
    for (index, value) in object.values.iter().enumerate().skip(start) {
        work(cx, 1, &at)?;
        if value.code == 1001 || (marker.is_some() && value.code == 100) {
            end = index;
            break;
        }
    }
    let common = start..end;
    let mut values = Vec::with_capacity(object.values.len() + written.len());
    let mut insert_at = None;
    for (index, value) in object.values.iter().enumerate() {
        let inside = common.contains(&index);
        if inside && value.code == 8 && insert_at.is_none() {
            if !changed.contains(&8) {
                values.push(copy_value(value, cx, &at)?);
            } else {
                work(cx, 1, &at)?;
            }
            insert_at = Some(values.len());
        } else if !(inside && changed.contains(&value.code)) {
            values.push(copy_value(value, cx, &at)?);
        } else {
            work(cx, 1, &at)?;
        }
    }
    let insert_at = insert_at.unwrap_or(common.start);
    values.splice(insert_at..insert_at, written);
    Ok(Some(values))
}

/// 共通値を写すときも、要素と可変長のバイト列を数える
fn copy_value(value: &Value, cx: &mut Cx<'_>, at: &Ref) -> Result<Value, Error> {
    work(cx, 1, at)?;
    let copy = |data: &[u8], cx: &mut Cx<'_>| -> Result<Vec<u8>, Error> {
        let mut result = Vec::new();
        for chunk in data.chunks(1024) {
            work(cx, 1, at)?;
            result.extend_from_slice(chunk);
        }
        Ok(result)
    };
    let data = match &value.data {
        Data::Text(data) => Data::Text(copy(data, cx)?),
        Data::Binary(data) => Data::Binary(copy(data, cx)?),
        Data::Int(value) => Data::Int(*value),
        Data::Real(value) => Data::Real(*value),
        Data::Bool(value) => Data::Bool(*value),
        Data::Handle(value) => Data::Handle(*value),
    };
    Ok(Value {
        code: value.code,
        data,
    })
}

/// 動かした図形と、続く ATTRIB の値
struct Moved {
    object: Vec<Value>,
    followers: Vec<(usize, Vec<Value>)>,
}

/// 変換 `transform` で動かした図形の値。形は置き換えと同じ書き方で書き、図形とともに動く XDATA（[`carried`]）と、
/// 記号の属性（続く ATTRIB の置く点・高さ・回転）も同じ変換で直す。`model` でなければ（用紙の図形）、
/// XDATA の点は用紙の mm と図面の単位の違いを直せないので、動く XDATA を持つ図形は扱わない
fn moved<P: Copy + PartialEq + transform::Point>(
    db: &AcadDb,
    was: &Element<P>,
    object: &Object,
    transform: &Affine,
    unicode: bool,
    frame: &impl Frame<P>,
    model: bool,
) -> Option<Moved> {
    let shape = transform::transform(&was.shape, transform)?;
    let element = Element {
        shape,
        ..was.clone()
    };
    // 属性は続く ATTRIB にあるので、INSERT は元の属性のまま書き、ATTRIB を下で直す
    let written = match (&element.shape, &was.shape) {
        (Shape::Symbol(symbol), Shape::Symbol(original)) => Element {
            shape: Shape::Symbol(Symbol {
                attributes: original.attributes.clone(),
                ..symbol.clone()
            }),
            ..element.clone()
        },
        _ => element.clone(),
    };
    let mut values = replacement(was, &written, object, unicode, frame)?;
    if moving_xdata(object).is_some() {
        if !model {
            return None;
        }
        carried(&mut values, transform)?;
    }
    let mut followers = Vec::new();
    if let (Shape::Symbol(symbol), Shape::Symbol(was)) = (&element.shape, &was.shape) {
        for (attribute, original) in symbol.attributes.iter().zip(&was.attributes) {
            if original.formatted.is_some() {
                return None;
            }
            let position = entity(db, &original.source)?;
            let follower = db.objects().get(position)?;
            if follower.kind != b"ATTRIB" {
                return None;
            }
            let mut values = follower.values.clone();
            lettering(&mut values, &attribute.text, &original.text, frame, unicode)?;
            if moving_xdata(follower).is_some() {
                if !model {
                    return None;
                }
                carried(&mut values, transform)?;
            }
            followers.push((position, values));
        }
    }
    Some(Moved {
        object: values,
        followers,
    })
}

/// 図形とともに動く XDATA を、AutoCAD と同じ規則で直す（DXF Reference「Group Codes That Apply to Extended
/// Data」）。位置（1011・1021）は変換で写し、変位（1012・1022）は平行移動を除いて写し、向き（1013・1023）は
/// 写してから元の長さにそろえる。距離（1041）と倍率（1042）は尺度を掛け、相似でない変換では直さない
fn carried(values: &mut [Value], transform: &Affine) -> Option<()> {
    let start = values.iter().position(|v| v.code == 1001)?;
    let tail = values.get_mut(start..)?;
    let scale = if tail.iter().any(|v| matches!(v.code, 1041 | 1042)) {
        transform.similarity()?.scale
    } else {
        1.0
    };
    // X の値を持ち、続く Y の値を待っている点
    let mut pending: Option<&mut Value> = None;
    for value in tail.iter_mut() {
        if let Some(first) = pending.take() {
            let (Data::Real(x), Data::Real(y)) = (&first.data, &value.data) else {
                return None;
            };
            if value.code != first.code + 10 {
                return None;
            }
            let [x, y] = match first.code {
                1011 => {
                    let (x, y) = transform.apply(*x, *y);
                    [x, y]
                }
                1012 => transform.vector([*x, *y]),
                _ => transform.direction([*x, *y]),
            };
            first.data = Data::Real(x);
            value.data = Data::Real(y);
            continue;
        }
        match value.code {
            1011..=1013 => pending = Some(value),
            1041 | 1042 => {
                let Data::Real(number) = value.data else {
                    return None;
                };
                value.data = Data::Real(number * scale);
            }
            _ => {}
        }
    }
    (pending.is_none()
        && tail
            .iter()
            .all(|v| !matches!(v.data, Data::Real(number) if !number.is_finite())))
    .then_some(())
}

/// 出所が `at` の図形の、表の中の番号。ほかのレイアウトの用紙の図形は、BLOCKS のペーパー空間のブロックにある
fn entity(db: &AcadDb, at: &Ref) -> Option<usize> {
    db.objects().iter().position(|object| {
        matches!(object.section, Section::Entities | Section::Blocks) && source(object) == *at
    })
}

/// 図面モデルの点と長さを、元の図形のレコードの値に戻す
trait Frame<P> {
    /// 点のレコードの X と Y。元の要素と同じ空間（用紙）の点でなければ `None`
    fn point(&self, point: &P) -> Option<[f64; 2]>;
    /// 長さのレコードの値。元の要素と同じ空間（用紙）の長さでなければ `None`
    fn length(&self, length: &Length) -> Option<f64>;
    /// レコードの長さを、読む側と同じく図面モデルの長さにする
    fn lifted(&self, value: f64) -> Length;
}

/// 実寸の空間。点と長さは、レコードの値のまま
struct InSpace(SpaceId);

impl Frame<ModelPoint> for InSpace {
    fn point(&self, point: &ModelPoint) -> Option<[f64; 2]> {
        (point.space == self.0).then_some([point.x, point.y])
    }

    fn length(&self, length: &Length) -> Option<f64> {
        match length {
            Length::Model(length) if length.space == self.0 => Some(length.value),
            _ => None,
        }
    }

    fn lifted(&self, value: f64) -> Length {
        Length::Model(ModelLength {
            space: self.0,
            value,
        })
    }
}

/// 用紙。用紙の mm を、読むときと逆に、図面の単位に戻して原点を足す
struct OnSheet(SheetId, PaperFrame);

impl Frame<PaperPoint> for OnSheet {
    fn point(&self, point: &PaperPoint) -> Option<[f64; 2]> {
        let PaperFrame {
            origin: [x, y], mm, ..
        } = self.1;
        (point.sheet == self.0).then_some([point.x / mm + x, point.y / mm + y])
    }

    fn length(&self, length: &Length) -> Option<f64> {
        match length {
            Length::Paper(PaperLength(value)) => Some(value / self.1.mm),
            _ => None,
        }
    }

    fn lifted(&self, value: f64) -> Length {
        Length::Paper(PaperLength(value * self.1.mm))
    }
}

/// 点を書く。変わっていなければ、元の値を残す（戻した値が最後の桁で違って、変えていない点を書き換えないため）
fn place<P: PartialEq>(
    values: &mut Vec<Value>,
    frame: &impl Frame<P>,
    [x, y]: [i16; 2],
    point: &P,
    was: &P,
) -> Option<()> {
    let at = frame.point(point)?;
    if point != was {
        put(values, x, at[0], &[])?;
        put(values, y, at[1], &[])?;
    }
    Some(())
}

/// 図形とともに動く XDATA の最初のグループ・コード。AutoCAD は、図形を動かし・回し・尺度を変えると、
/// 1011（位置）・1012（変位）・1013（向き）・1041（距離）・1042（倍率）を同じく直す（DXF Reference
/// 「Group Codes That Apply to Extended Data」）。置き換えから同じ変換を決められないので、書き戻さない（決定 0154）
fn moving_xdata(object: &Object) -> Option<i16> {
    let start = object.values.iter().position(|v| v.code == 1001)?;
    object
        .values
        .iter()
        .skip(start)
        .map(|v| v.code)
        .find(|code| matches!(code, 1011..=1013 | 1041 | 1042))
}

/// 続くレコード（POLYLINE の VERTEX、属性を持つ INSERT の ATTRIB、SEQEND）を持つ図形か
fn followed(object: &Object) -> bool {
    match object.kind.as_slice() {
        b"POLYLINE" => true,
        b"INSERT" => matches!(object.first(66), Some(Data::Int(n)) if *n != 0),
        _ => false,
    }
}

/// 置き換えで書く値。元の値を写し、変わった形の欄だけを直す。レイヤ・スタイルが同じで、形の種類が
/// 元のレコードで表せ、点と長さが元の要素と同じ空間にあるときだけ返す
fn replacement<P: PartialEq>(
    original: &Element<P>,
    element: &Element<P>,
    object: &Object,
    unicode: bool,
    frame: &impl Frame<P>,
) -> Option<Vec<Value>> {
    if element.layer != original.layer || element.style != original.style {
        return None;
    }
    let mut values = object.values.clone();
    if element.shape == original.shape {
        return Some(values);
    }
    match (&element.shape, &original.shape, object.kind.as_slice()) {
        (
            Shape::Line { start, end },
            Shape::Line {
                start: was_start,
                end: was_end,
            },
            b"LINE",
        ) => {
            place(&mut values, frame, [10, 20], start, was_start)?;
            place(&mut values, frame, [11, 21], end, was_end)?;
        }
        (
            Shape::Circle { center, radius },
            Shape::Circle {
                center: was_center,
                radius: was_radius,
            },
            b"CIRCLE",
        ) => {
            place(&mut values, frame, [10, 20], center, was_center)?;
            if radius != was_radius {
                put(&mut values, 40, positive(frame, radius)?, &[])?;
            }
        }
        // ARC（DXF Reference「ARC」）は反時計回りの角度（度）で持つ。時計回りの弧は、始まりと終わりを入れ替える
        (
            Shape::Curve(Edge::Arc {
                center,
                radius,
                start,
                end,
                counterclockwise,
            }),
            Shape::Curve(Edge::Arc {
                center: was_center,
                radius: was_radius,
                ..
            }),
            b"ARC",
        ) => {
            place(&mut values, frame, [10, 20], center, was_center)?;
            let (start, end) = turned(*start, *end, *counterclockwise);
            if radius != was_radius {
                put(&mut values, 40, positive(frame, radius)?, &[])?;
            }
            put(&mut values, 50, start, &[])?;
            put(&mut values, 51, end, &[])?;
        }
        (
            Shape::Curve(Edge::Elliptic {
                center,
                major,
                ratio,
                start,
                end,
                counterclockwise,
            }),
            Shape::Curve(Edge::Elliptic {
                center: was_center,
                major: was_major,
                start: was_start,
                end: was_end,
                ..
            }),
            b"ELLIPSE",
        ) => {
            ellipse(
                &mut values,
                frame,
                [center, major, was_center, was_major],
                *ratio,
                turned(*start, *end, *counterclockwise),
                (*was_start, *was_end),
            )?;
        }
        (Shape::Polyline(line), Shape::Polyline(was), b"LWPOLYLINE") => {
            values = lwpolyline(&values, line, was, frame)?;
        }
        (Shape::Text(text), Shape::Text(was), b"TEXT") => {
            lettering(&mut values, text, was, frame, unicode)?;
        }
        (Shape::Formatted(text), Shape::Formatted(was), b"MTEXT") => {
            formatting(&mut values, text, was, frame, unicode)?;
        }
        (Shape::Symbol(symbol), Shape::Symbol(was), b"INSERT") => {
            placing(&mut values, symbol, was, frame)?;
        }
        _ => return None,
    }
    Some(values)
}

/// ELLIPSE（DXF Reference「ELLIPSE」）の欄を直す。長軸の端（11・21）は中心からの向き、始まりと終わりの
/// パラメーター（41・42）はラジアンで持つ。読むときに度へ直した値と比べ、変わった欄だけを書く。
/// 直し戻した値が元の値と最後の桁で違って、変えていない欄まで書き換えないようにするため
fn ellipse<P: PartialEq>(
    values: &mut Vec<Value>,
    frame: &impl Frame<P>,
    [center, major, was_center, was_major]: [&P; 4],
    ratio: f64,
    (start, end): (f64, f64),
    (was_start, was_end): (f64, f64),
) -> Option<()> {
    if !(ratio > 0.0 && ratio <= 1.0) {
        return None;
    }
    place(values, frame, [10, 20], center, was_center)?;
    let [x, y] = frame.point(center)?;
    let [end_x, end_y] = frame.point(major)?;
    if center != was_center || major != was_major {
        let axis = [end_x - x, end_y - y];
        if axis == [0.0, 0.0] {
            return None;
        }
        put(values, 11, axis[0], &[])?;
        put(values, 21, axis[1], &[])?;
    }
    put(values, 40, ratio, &[])?;
    if start != was_start {
        put(values, 41, start.to_radians(), &[40])?;
    }
    if end != was_end {
        put(values, 42, end.to_radians(), &[41, 40])?;
    }
    Some(())
}

/// LWPOLYLINE（DXF Reference「LWPOLYLINE」）の値を組み直す。頂点の前の値は写して、頂点の数（90）・閉じて
/// いること（70 の 1）・一定の幅（43）を直し、頂点ごとに 10・20、頂点の ID（91）、頂点ごとの幅（40・41）、
/// 0 でない膨らみ（42）を並べる。頂点の後の値（押し出しの向きなど）は写す。頂点の ID を持つのに頂点の数が
/// 変わるもの、頂点の間にほかの値があるもの、一定の幅と頂点ごとの幅の両方を持つものは書き戻さない
fn lwpolyline<P: PartialEq>(
    values: &[Value],
    line: &Polyline<P>,
    was: &Polyline<P>,
    frame: &impl Frame<P>,
) -> Option<Vec<Value>> {
    let vertex = |code: i16| matches!(code, 10 | 20 | 40 | 41 | 42 | 91);
    let first = values.iter().position(|v| v.code == 10)?;
    let last = values.iter().rposition(|v| vertex(v.code))?;
    let body = values.get(first..=last)?;
    if body.iter().any(|v| !vertex(v.code)) {
        return None;
    }
    let ids: Vec<&Value> = body.iter().filter(|v| v.code == 91).collect();
    // 元の頂点の 10・20。変わっていない頂点は、元の値を写す
    let xs: Vec<&Value> = body.iter().filter(|v| v.code == 10).collect();
    let ys: Vec<&Value> = body.iter().filter(|v| v.code == 20).collect();
    let count = line.vertices.len();
    if count == 0 || !(ids.is_empty() || ids.len() == count) {
        return None;
    }
    let widths = !line.vertex_widths.is_empty();
    if widths && (line.vertex_widths.len() != count || line.width.is_some()) {
        return None;
    }
    let mut out = values.get(..first)?.to_vec();
    let slot = out.iter_mut().find(|v| v.code == 90)?;
    slot.data = Data::Int(i64::try_from(count).ok()?);
    match out.iter_mut().find(|v| v.code == 70) {
        Some(Value {
            data: Data::Int(flags),
            ..
        }) => *flags = (*flags & !1) | i64::from(line.closed),
        Some(_) => return None,
        None if line.closed => insert(&mut out, 70, Data::Int(1), &[90])?,
        None => {}
    }
    match &line.width {
        Some(width) => {
            let width = Data::Real(nonnegative(frame, width)?);
            match out.iter_mut().find(|v| v.code == 43) {
                Some(slot) => slot.data = width,
                None => insert(&mut out, 43, width, &[70, 90])?,
            }
        }
        None => out.retain(|v| v.code != 43),
    }
    for (index, corner) in line.vertices.iter().enumerate() {
        let [x, y] = frame.point(&corner.point)?;
        let kept = was.vertices.get(index).map(|v| &v.point) == Some(&corner.point);
        match (kept, xs.get(index), ys.get(index)) {
            (true, Some(x), Some(y)) => out.extend([(*x).clone(), (*y).clone()]),
            _ => out.extend([real(10, x)?, real(20, y)?]),
        }
        if let Some(id) = ids.get(index) {
            out.push((*id).clone());
        }
        if let Some([start, end]) = line.vertex_widths.get(index) {
            out.push(real(40, nonnegative(frame, start)?)?);
            out.push(real(41, nonnegative(frame, end)?)?);
        }
        if corner.bulge != 0.0 {
            out.push(real(42, corner.bulge)?);
        }
    }
    out.extend_from_slice(values.get(last + 1..)?);
    Some(out)
}

/// TEXT（DXF Reference「TEXT」）の欄を直す。内容（1）・高さ（40）・回転（50）・幅の倍率（41）・斜体の角度（51）・
/// 反転（71 の 2 と 4）と、置く点を書く。位置合わせ（72・73）・2 点の間への合わせ方・文字スタイル・高さの意味は
/// 変えられない。左の基準線に置く文字は 10 を、ほかの位置合わせの文字は 11 を基点とし、10 は 11 と同じだけ動かす。
/// 10 は位置合わせから決まる点で、読み直すときに使わないが、ほかのソフトが読むので食い違わせない
fn lettering<P: PartialEq>(
    values: &mut Vec<Value>,
    text: &Text<P>,
    was: &Text<P>,
    frame: &impl Frame<P>,
    unicode: bool,
) -> Option<()> {
    if text.style != was.style || text.height.meaning != was.height.meaning {
        return None;
    }
    match (&text.placement, &was.placement) {
        (
            Placement::Anchor {
                point,
                horizontal,
                vertical,
            },
            Placement::Anchor {
                point: was_point,
                horizontal: was_horizontal,
                vertical: was_vertical,
            },
        ) if horizontal == was_horizontal && vertical == was_vertical => {
            let at = frame.point(point)?;
            let was_at = frame.point(was_point)?;
            let aligned = [72, 73].iter().any(|code| {
                values
                    .iter()
                    .any(|v| v.code == *code && v.data != Data::Int(0))
            });
            if point != was_point && aligned {
                let first = [first_real(values, 10)?, first_real(values, 20)?];
                put(values, 10, first[0] + (at[0] - was_at[0]), &[])?;
                put(values, 20, first[1] + (at[1] - was_at[1]), &[])?;
                put(values, 11, at[0], &[])?;
                put(values, 21, at[1], &[])?;
            } else if point != was_point {
                put(values, 10, at[0], &[])?;
                put(values, 20, at[1], &[])?;
            }
        }
        (
            Placement::Between { first, second, fit },
            Placement::Between {
                first: was_first,
                second: was_second,
                fit: was_fit,
            },
        ) if fit == was_fit => {
            place(values, frame, [10, 20], first, was_first)?;
            place(values, frame, [11, 21], second, was_second)?;
        }
        _ => return None,
    }
    if text.content != was.content {
        // DXF の値は 1 行なので、改行は書けない
        let content = &text.content;
        if !(unicode || content.is_ascii()) || content.contains(['\n', '\r']) {
            return None;
        }
        let slot = values.iter_mut().find(|v| v.code == 1)?;
        slot.data = Data::Text(content.as_bytes().to_vec());
    }
    if text.height.value != was.height.value {
        put(values, 40, positive(frame, &text.height.value)?, &[])?;
    }
    if text.width_factor.is_nan() || text.width_factor <= 0.0 {
        return None;
    }
    optional(values, 50, text.rotation, 0.0, &[1])?;
    optional(values, 41, text.width_factor, 1.0, &[50, 1])?;
    optional(values, 51, text.oblique, 0.0, &[41, 50, 1])?;
    let flags = match values.iter().find(|v| v.code == 71).map(|v| &v.data) {
        Some(Data::Int(flags)) => *flags,
        Some(_) => return None,
        None => 0,
    };
    let flags = (flags & !6) | (i64::from(text.backward) * 2) | (i64::from(text.upside_down) * 4);
    match values.iter_mut().find(|v| v.code == 71) {
        Some(slot) => slot.data = Data::Int(flags),
        None if flags != 0 => insert(values, 71, Data::Int(flags), &[7, 51, 41, 50, 1])?,
        None => {}
    }
    Some(())
}

/// MTEXT（DXF Reference「MTEXT」）の欄を直す。内容（250 バイトずつの 3 と、最後の 1）・基点（10）・高さ（40）・
/// 向き（50 のラジアンか、11・21 の向き）・枠の幅（41）と高さ（46）・位置合わせ（71）・行送り（44・73）を書く。
/// 文字スタイル・書き進める向き・背景・高さの意味は変えられない。向きは、元と同じ形（角度か向き）のときだけ
/// 書く。省いた欄は、DXF Reference の並び（内容の後に 50・73・44）で足す。区間は内容から決まるので、内容を解釈し直した区間と合わなければ受け付けない
fn formatting<P: PartialEq>(
    values: &mut Vec<Value>,
    text: &FormattedText<P>,
    was: &FormattedText<P>,
    frame: &impl Frame<P>,
    unicode: bool,
) -> Option<()> {
    if text.style != was.style
        || text.flow != was.flow
        || text.background != was.background
        || text.height.meaning != was.height.meaning
    {
        return None;
    }
    if text.spans != mtext::spans(&text.content, &|value| frame.lifted(value)) {
        return None;
    }
    place(values, frame, [10, 20], &text.point, &was.point)?;
    if text.content != was.content {
        contents(values, &text.content, unicode)?;
    }
    if text.height.value != was.height.value {
        put(values, 40, positive(frame, &text.height.value)?, &[])?;
    }
    if text.rotation != was.rotation {
        match (text.rotation, was.rotation) {
            (Rotation::Angle(angle), Rotation::Angle(_)) => {
                put(values, 50, angle.to_radians(), &[43, 42, 7, 1])?;
            }
            (Rotation::Direction([x, y]), Rotation::Direction(_)) if [x, y] != [0.0, 0.0] => {
                put(values, 11, x, &[])?;
                put(values, 21, y, &[11])?;
                // 角度と方向の両方があると、後の指定が優先される（DXF Reference「MTEXT」）。
                // 古い角度が新しい方向を上書きしないよう、方向だけを残す
                values.retain(|value| value.code != 50);
            }
            _ => return None,
        }
    }
    for (code, box_size, was_size) in [
        (41, &text.width, &was.width),
        (46, &text.frame_height, &was.frame_height),
    ] {
        if box_size != was_size {
            let size = match box_size {
                Some(size) => positive(frame, size)?,
                None => 0.0,
            };
            optional(values, code, size, 0.0, &[40])?;
        }
    }
    if (text.horizontal, text.vertical) != (was.horizontal, was.vertical) {
        let row = match text.vertical {
            Vertical::Top => 0,
            Vertical::Middle => 1,
            Vertical::Bottom => 2,
            _ => return None,
        };
        let column = match text.horizontal {
            Horizontal::Left => 1,
            Horizontal::Center => 2,
            Horizontal::Right => 3,
            _ => return None,
        };
        let attachment = Data::Int(row * 3 + column);
        match values.iter_mut().find(|v| v.code == 71) {
            Some(slot) => slot.data = attachment,
            None => insert(values, 71, attachment, &[41, 40])?,
        }
    }
    if text.line_spacing != was.line_spacing {
        let factor = text.line_spacing.factor;
        if !(0.25..=4.0).contains(&factor) {
            return None;
        }
        let exact = Data::Int(if text.line_spacing.exact { 2 } else { 1 });
        match values.iter_mut().find(|v| v.code == 73) {
            Some(slot) => slot.data = exact,
            None if text.line_spacing.exact => insert(values, 73, exact, &[50, 43, 42, 7, 1])?,
            None => {}
        }
        optional(values, 44, factor, 1.0, &[73, 50, 43, 42, 7, 1])?;
    }
    Some(())
}

/// INSERT（DXF Reference「INSERT」）の置く点（10・20）・尺度（41・42）・回転（50、度）を直す。定義（2）・
/// 格子（70・71・44・45）・属性・切り抜きは変えられない。属性と切り抜きは、続くレコードと拡張辞書にあるため
fn placing<P: PartialEq>(
    values: &mut Vec<Value>,
    symbol: &Symbol<P>,
    was: &Symbol<P>,
    frame: &impl Frame<P>,
) -> Option<()> {
    if symbol.key != was.key
        || symbol.grid != was.grid
        || symbol.attributes != was.attributes
        || symbol.clip != was.clip
    {
        return None;
    }
    place(values, frame, [10, 20], &symbol.position, &was.position)?;
    // 尺度と回転は、省けば既定の値になる。変わったときだけ、欄の並び（10・20・30・41・42・43・50）に合わせて書く
    let [x, y] = symbol.scale;
    if x != was.scale[0] {
        optional(values, 41, nonzero(x)?, 1.0, &[30, 20])?;
    }
    if y != was.scale[1] {
        optional(values, 42, nonzero(y)?, 1.0, &[41, 30, 20])?;
    }
    if symbol.rotation != was.rotation {
        optional(values, 50, symbol.rotation, 0.0, &[43, 42, 41, 30, 20])?;
    }
    Some(())
}

/// 0 でない有限の値
fn nonzero(value: f64) -> Option<f64> {
    (value.is_finite() && value != 0.0).then_some(value)
}

/// MTEXT の内容を、250 バイトずつの区切り（3）と最後の値（1）にして、元の内容の値の場所に置く。区切りは
/// 文字の途中と、制御文字の `^` の表記の途中で切らない。DXF の値は 1 行なので、改行は書けない（段落の区切りは `\P` で書く）
fn contents(values: &mut Vec<Value>, content: &str, unicode: bool) -> Option<()> {
    if !(unicode || content.is_ascii()) || content.contains(['\n', '\r']) {
        return None;
    }
    let at = values.iter().position(|v| matches!(v.code, 1 | 3))?;
    values.retain(|v| !matches!(v.code, 1 | 3));
    let mut chunks = Vec::new();
    let mut rest = content;
    while rest.len() > 250 {
        let mut cut = (0..=250).rev().find(|at| rest.is_char_boundary(*at))?;
        // バイナリの DXF と DWG は、区切りごとに `^ ` や `^J` を戻す。末尾の `^` が対を持たなければ
        // 次の区切りへ送る。`^^` は 1 組の表記なので、その間では切らない
        let carets = rest
            .as_bytes()
            .get(..cut)?
            .iter()
            .rev()
            .take_while(|&&byte| byte == b'^')
            .count();
        if !carets.is_multiple_of(2) {
            cut = cut.checked_sub(1)?;
        }
        let (chunk, tail) = rest.split_at(cut);
        chunks.push(Value {
            code: 3,
            data: Data::Text(chunk.as_bytes().to_vec()),
        });
        rest = tail;
    }
    chunks.push(Value {
        code: 1,
        data: Data::Text(rest.as_bytes().to_vec()),
    });
    values.splice(at..at, chunks);
    Some(())
}

/// `code` の最初の有限の実数
fn first_real(values: &[Value], code: i16) -> Option<f64> {
    values.iter().find_map(|v| match v.data {
        Data::Real(value) if v.code == code && value.is_finite() => Some(value),
        _ => None,
    })
}

/// 省けば `default` になる欄を直す。欄がなく、値が `default` なら足さない
fn optional(
    values: &mut Vec<Value>,
    code: i16,
    value: f64,
    default: f64,
    after: &[i16],
) -> Option<()> {
    if value == default && !values.iter().any(|v| v.code == code) {
        return Some(());
    }
    put(values, code, value, after)
}

/// 時計回りの弧を、同じ弧をたどる反時計回りの始まりと終わりに直す
fn turned(start: f64, end: f64, counterclockwise: bool) -> (f64, f64) {
    if counterclockwise {
        (start, end)
    } else {
        (end, start)
    }
}

/// 元の要素と同じ空間（用紙）の長さのレコードの値で、0 より大きいもの
fn positive<P>(frame: &impl Frame<P>, length: &Length) -> Option<f64> {
    nonnegative(frame, length).filter(|value| *value > 0.0)
}

/// 元の要素と同じ空間（用紙）の長さのレコードの値で、0 以上のもの
fn nonnegative<P>(frame: &impl Frame<P>, length: &Length) -> Option<f64> {
    frame
        .length(length)
        .filter(|value| value.is_finite() && *value >= 0.0)
}

/// 有限の実数の値
fn real(code: i16, value: f64) -> Option<Value> {
    value.is_finite().then_some(Value {
        code,
        data: Data::Real(value),
    })
}

/// `code` の最初の値を `value` にする。なければ、`after` のうち最初に見つかったコードの値の後に足す
fn put(values: &mut Vec<Value>, code: i16, value: f64, after: &[i16]) -> Option<()> {
    let value = real(code, value)?;
    match values.iter_mut().find(|v| v.code == code) {
        Some(slot) => slot.data = value.data,
        None => insert(values, code, value.data, after)?,
    }
    Some(())
}

/// `after` のうち最初に見つかったコードの値の後に、値を足す
fn insert(values: &mut Vec<Value>, code: i16, data: Data, after: &[i16]) -> Option<()> {
    let at = after
        .iter()
        .find_map(|after| values.iter().position(|v| v.code == *after))?;
    values.insert(at + 1, Value { code, data });
    Some(())
}

/// 置き換えられる形の初めの点。形の空間（用紙）を決める
fn anchor<P: Copy>(shape: &Shape<P>) -> Option<P> {
    match shape {
        Shape::Line { start, .. } => Some(*start),
        Shape::Circle { center, .. } => Some(*center),
        Shape::Curve(Edge::Arc { center, .. } | Edge::Elliptic { center, .. }) => Some(*center),
        Shape::Polyline(line) => line.vertices.first().map(|v| v.point),
        Shape::Text(text) => match &text.placement {
            Placement::Anchor { point, .. } => Some(*point),
            Placement::Between { first, .. } => Some(*first),
        },
        Shape::Formatted(text) => Some(text.point),
        Shape::Symbol(symbol) => Some(symbol.position),
        _ => None,
    }
}

#[cfg(test)]
mod duplicate_handle_tests {
    use super::*;
    use crate::db::Origin;
    use zumen_base::{Budget, CancelToken, Kind, Limit, Limits};

    #[test]
    fn checking_handles_obeys_work_budget_and_cancellation() {
        let mut db = AcadDb::default();
        for handle in [1, 2] {
            db.push(Object {
                kind: b"LINE".to_vec(),
                section: Section::Entities,
                values: vec![Value {
                    code: 5,
                    data: Data::Handle(handle),
                }],
                origin: Origin::New,
            });
        }
        let changes = Changes {
            edits: vec![],
            document: None,
        };
        let before = db.clone();
        let mut budget = Budget::new(Limits::default().with(Kind::Work, Limit::Finite(1)));
        let mut diag = Diagnostics::new();
        let cancel = CancelToken::new();
        assert!(matches!(
            edit(
                &mut db,
                &changes,
                Target::Dxf,
                &mut Cx::new(&mut budget, &mut diag, &cancel)
            ),
            Err(Error::Stopped(_))
        ));
        assert_eq!(budget.used(Kind::Work), 1);
        assert_eq!(db, before);
        let mut budget = Budget::default();
        cancel.cancel();
        assert!(matches!(
            edit(
                &mut db,
                &changes,
                Target::Dxf,
                &mut Cx::new(&mut budget, &mut diag, &cancel)
            ),
            Err(Error::Stopped(_))
        ));
        assert_eq!(budget.used(Kind::Work), 0);
        assert_eq!(db, before);
    }
}

#[cfg(test)]
mod hash_budget_tests {
    use super::*;
    use zumen_base::{Budget, CancelToken, Key, Kind, Limit, Limits};
    use zumen_drawing::{LayerId, ModelSpace, Style, Unit, Vertex};

    fn context<'a>(
        budget: &'a mut Budget,
        diagnostics: &'a mut Diagnostics,
        cancel: &'a CancelToken,
    ) -> Cx<'a> {
        Cx::new(budget, diagnostics, cancel)
    }

    #[test]
    fn indexes_sheets_with_the_same_names_as_entities() {
        use zumen_drawing::{PaperPoint, Sheet};
        let sheets = (0..2)
            .map(|number| Sheet {
                id: SheetId(number),
                name: None,
                size: None,
                elements: vec![Element {
                    source: Ref::at_record(u64::from(number) + 10)
                        .with_key(Key::Handle(u64::from(number) + 10)),
                    layer: LayerId(0),
                    style: Style::BLOCK,
                    shape: Shape::Line {
                        start: PaperPoint {
                            sheet: SheetId(number),
                            x: 0.0,
                            y: 0.0,
                        },
                        end: PaperPoint {
                            sheet: SheetId(number),
                            x: 1.0,
                            y: 1.0,
                        },
                    },
                }],
                views: vec![],
                order: vec![],
                source: Ref::at_record(u64::from(number) + 20).with_key(Key::Handle(5)),
            })
            .collect();
        let drawing = Drawing2D {
            sheets,
            ..Drawing2D::default()
        };
        let mut budget = Budget::default();
        let mut diagnostics = Diagnostics::new();
        let cancel = CancelToken::new();
        let index = Index::new(
            &drawing,
            &mut context(&mut budget, &mut diagnostics, &cancel),
        )
        .unwrap();
        assert!(matches!(
            index.places.get("sheet/h5"),
            Some(Place::Sheet(SheetId(0)))
        ));
        assert!(matches!(
            index.places.get("sheet/h5/1"),
            Some(Place::Sheet(SheetId(1)))
        ));
        assert!(index.elements.contains_key("sheet/h5/hA"));
        assert!(index.elements.contains_key("sheet/h5/1/hB"));
    }

    #[test]
    fn checks_element_hashes_with_work_and_cancellation() {
        let point = ModelPoint {
            space: SpaceId(0),
            x: 0.0,
            y: 0.0,
        };
        let element = Element {
            source: Ref::at_record(1).with_key(Key::Handle(1)),
            layer: LayerId(0),
            style: Style::BLOCK,
            shape: Shape::Polyline(Polyline {
                vertices: vec![Vertex { point, bulge: 0.0 }; 100],
                closed: false,
                width: None,
                vertex_widths: vec![],
            }),
        };
        let drawing = Drawing2D {
            spaces: vec![ModelSpace {
                id: SpaceId(0),
                unit: Unit::Unknown,
                elements: vec![element],
                order: vec![0],
            }],
            ..Drawing2D::default()
        };
        let mut diagnostics = Diagnostics::new();
        let cancel = CancelToken::new();
        let mut budget = Budget::new(Limits::default().with(Kind::Work, Limit::Finite(1)));
        assert!(matches!(
            Index::new(
                &drawing,
                &mut context(&mut budget, &mut diagnostics, &cancel)
            ),
            Err(Error::Stopped(_))
        ));
        let mut budget = Budget::default();
        let index = Index::new(
            &drawing,
            &mut context(&mut budget, &mut diagnostics, &cancel),
        )
        .unwrap();
        diagnostics = Diagnostics::new();
        let mut budget = Budget::new(Limits::default().with(Kind::Work, Limit::Finite(10)));
        let result = index.hash(
            "space/0/h1",
            &mut context(&mut budget, &mut diagnostics, &cancel),
        );
        assert!(matches!(result, Err(Error::Stopped(_))));
        assert_eq!(
            diagnostics.iter().next().unwrap().code(),
            codes::BUDGET_WORK_EXCEEDED
        );
        let mut budget = Budget::default();
        let actual = applied(
            &index,
            Some("space/0/h1".into()),
            &mut context(&mut budget, &mut diagnostics, &cancel),
        )
        .unwrap();
        assert_eq!(
            actual.hash,
            Some(Hash::element(&drawing.spaces[0].elements[0]))
        );
        cancel.cancel();
        let mut budget = Budget::default();
        assert!(matches!(
            applied(
                &index,
                Some("space/0/h1".into()),
                &mut context(&mut budget, &mut diagnostics, &cancel)
            ),
            Err(Error::Stopped(_))
        ));
    }

    #[test]
    fn restyle_copy_checks_work_and_preserves_all_bytes() {
        let value = Value {
            code: 1000,
            data: Data::Text(vec![42; 5000]),
        };
        let mut diagnostics = Diagnostics::new();
        let cancel = CancelToken::new();
        let mut budget = Budget::new(Limits::default().with(Kind::Work, Limit::Finite(3)));
        assert!(matches!(
            copy_value(
                &value,
                &mut context(&mut budget, &mut diagnostics, &cancel),
                &Ref::at_record(9)
            ),
            Err(Error::Stopped(_))
        ));
        assert_eq!(budget.used(Kind::Work), 3);
        let mut budget = Budget::default();
        assert_eq!(
            copy_value(
                &value,
                &mut context(&mut budget, &mut diagnostics, &cancel),
                &Ref::at_record(9)
            )
            .unwrap(),
            value
        );
    }

    #[test]
    fn checks_document_hash_with_work_budget() {
        struct TestFile(Vec<u8>);
        fn write(file: &TestFile, out: &mut Vec<u8>, _: &mut Cx<'_>) -> Result<(), Error> {
            out.extend_from_slice(&file.0);
            Ok(())
        }
        let bytes = vec![42; 4096];
        let changes = Changes {
            edits: vec![],
            document: Some(Hash::document(&bytes)),
        };
        let mut diagnostics = Diagnostics::new();
        let cancel = CancelToken::new();
        let mut budget = Budget::new(Limits::default().with(Kind::Work, Limit::Finite(2)));
        assert!(matches!(
            document(
                &TestFile(bytes),
                &changes,
                write,
                &mut context(&mut budget, &mut diagnostics, &cancel)
            ),
            Err(Error::Stopped(_))
        ));
        assert_eq!(
            diagnostics.iter().next().unwrap().code(),
            codes::BUDGET_WORK_EXCEEDED
        );
    }
}
