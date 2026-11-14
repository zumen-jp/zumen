//! 図形のスタイルの変更・座標変換・記号の属性の書き換え・動く拡張データの更新。

use super::*;
use zumen_drawing::transform as geom_transform;

/// スタイルを変えた図形の値。変わったフィールド（レイヤ 8・線種 6・色 62 と 420 と 430・線幅 370・線種の倍率 48）
/// だけを、図形に共通の値の中で除いてから、足すときと同じ書き方（[`Born::entity`]）でレイヤの後に書く。
/// ほかの値は元のまま写す。DWG では、画層と線種をハンドルで、線の太さと真の色を共通のフィールドに書く（#723）
pub(crate) fn restyled(
    born: &Born,
    object: &Object,
    layer: LayerId,
    style: &Style,
    change: &Restyle,
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
    let mut written = Vec::new();
    if changed.contains(&8) {
        let Some(layer) = born.layer_value(new_layer) else {
            return Ok(None);
        };
        written.push(layer);
    }
    let Some(style) = born.style_values(&new, &changed, true) else {
        return Ok(None);
    };
    written.extend(style);
    // 図形に共通の値の範囲。サブクラスマーカー（100）を持てば AcDbEntity のサブクラスの中、持たなければ XDATA の前まで
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
pub(super) fn copy_value(value: &Value, cx: &mut Cx<'_>, at: &Ref) -> Result<Value, Error> {
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
pub(crate) struct Moved {
    pub(crate) object: Vec<Value>,
    pub(crate) followers: Followers,
}

/// 書き直す続くレコード（ATTRIB）の位置と値
pub(crate) type Followers = Vec<(usize, Vec<Value>)>;

/// 変換前に、値の走査・複製と属性のレコードの探索に必要な作業を数える。
pub(super) fn transform_work<P>(
    db: &AcadDb,
    positions: &BTreeMap<Ref, usize>,
    element: &Element<P>,
    object: &Object,
    cx: &mut Cx<'_>,
) -> Result<(), Error> {
    count_transform_values(object, &element.source, cx)?;
    if let Shape::Symbol(symbol) = &element.shape {
        for attribute in &symbol.attributes {
            work(cx, 1, &attribute.source)?;
            if let Some(candidate) = positions
                .get(&attribute.source)
                .and_then(|position| db.objects().get(*position))
            {
                count_transform_values(candidate, &attribute.source, cx)?;
            }
        }
    }
    Ok(())
}

pub(super) fn count_transform_values(
    object: &Object,
    at: &Ref,
    cx: &mut Cx<'_>,
) -> Result<(), Error> {
    for value in &object.values {
        work(cx, 1, at)?;
        let bytes = match &value.data {
            Data::Text(bytes) | Data::Binary(bytes) => bytes.len(),
            _ => 0,
        };
        for _ in (0..bytes).step_by(1024) {
            work(cx, 1, at)?;
        }
    }
    Ok(())
}

/// 呼び出し側が渡した線種の、列と文字列を検査・複製する作業を数える。
pub(super) fn count_line_type_change(line_type: &LineType, cx: &mut Cx<'_>) -> Result<(), Error> {
    let at = Ref::default();
    let Some(definition) = &line_type.definition else {
        return Ok(());
    };
    work(cx, 1 + definition.description.len() as u64 / 1024, &at)?;
    work(cx, definition.pattern.len() as u64, &at)?;
    for mark in &definition.marks {
        work(cx, 1, &at)?;
        match &mark.content {
            MarkContent::Text { content, style } => {
                work(cx, 1 + content.len() as u64 / 1024, &at)?;
                if let Some(style) = style {
                    work(cx, 1 + style.len() as u64 / 1024, &at)?;
                }
            }
            MarkContent::Shape { font, .. } => {
                if let Some(font) = font {
                    work(cx, 1 + font.name.len() as u64 / 1024, &at)?;
                }
            }
        }
    }
    Ok(())
}

/// 変換 `transform` で動かした図形の値。形は置き換えと同じ書き方で書き、図形とともに動く XDATA（[`carried`]）と、
/// 記号の属性（続く ATTRIB の置く点・高さ・回転）も同じ変換で直す。`model` でなければ（用紙の図形）、
/// XDATA の点は用紙の mm と図面の単位の違いを直せないので、動く XDATA を持つ図形は扱わない
#[allow(clippy::too_many_arguments)]
pub(super) fn moved<P: Copy + PartialEq + geom_transform::Point>(
    db: &AcadDb,
    positions: &BTreeMap<Ref, usize>,
    was: &Element<P>,
    object: &Object,
    transform: &Affine,
    encoder: &Encoder,
    frame: &impl Frame<P>,
    model: bool,
) -> Option<Moved> {
    let mut attribs = Vec::new();
    if let Shape::Symbol(symbol) = &was.shape {
        attribs.reserve(symbol.attributes.len());
        for original in &symbol.attributes {
            if original.formatted.is_some() {
                return None;
            }
            let position = *positions.get(&original.source)?;
            let follower = db.objects().get(position)?;
            attribs.push((position, follower));
        }
    }
    moved_with_attribs(was, object, &attribs, transform, encoder, frame, model)
}

/// 図形と、既に引いた続く ATTRIB の列 `attribs` を `transform` で動かす
#[allow(clippy::too_many_arguments)]
pub(crate) fn moved_with_attribs<P: Copy + PartialEq + geom_transform::Point>(
    was: &Element<P>,
    object: &Object,
    attribs: &[(usize, &Object)],
    transform: &Affine,
    encoder: &Encoder,
    frame: &impl Frame<P>,
    model: bool,
) -> Option<Moved> {
    let shape = geom_transform::transform(&was.shape, transform)?;
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
    // 引出線つきの注記は、注記の文脈の点と向きを、レコードの値のまま写す（決定 0191）
    let mut values = match &was.shape {
        Shape::Callout(_) if object.kind == b"MULTILEADER" => {
            swung(&object.values, &frame.affine(transform))?
        }
        Shape::Callout(_) => return None,
        _ => replacement(was, &written, object, encoder, frame)?,
    };
    if let (Shape::Text(before), Shape::Text(after)) = (&was.shape, &element.shape) {
        aligned_first(
            &mut values,
            &object.values,
            before,
            after,
            transform,
            frame,
            73,
        )?;
    }
    if let Shape::Leader(_) = &was.shape {
        directed(&mut values, transform)?;
    }
    if moving_xdata(object).is_some() {
        if !model {
            return None;
        }
        carried(&mut values, transform)?;
    }
    let mut followers = Vec::new();
    if let (Shape::Symbol(symbol), Shape::Symbol(was)) = (&element.shape, &was.shape) {
        if attribs.len() != was.attributes.len() || attribs.len() != symbol.attributes.len() {
            return None;
        }
        for (((position, follower), attribute), original) in
            attribs.iter().zip(&symbol.attributes).zip(&was.attributes)
        {
            if original.formatted.is_some() || follower.kind != b"ATTRIB" {
                return None;
            }
            let mut values = follower.values.clone();
            lettering(
                &mut values,
                &attribute.text,
                &original.text,
                frame,
                encoder,
                74,
            )?;
            aligned_first(
                &mut values,
                &follower.values,
                &original.text,
                &attribute.text,
                transform,
                frame,
                74,
            )?;
            if moving_xdata(follower).is_some() {
                if !model {
                    return None;
                }
                carried(&mut values, transform)?;
            }
            followers.push((*position, values));
        }
    }
    Some(Moved {
        object: values,
        followers,
    })
}

/// 動かした LEADER の X 軸の向き（211・221・231。DXF Reference「LEADER」）を、変換の向きで写す（決定 0185）。
/// 向きがなければ (1, 0, 0) として写し、写した向きが (1, 0, 0) なら書かない。鉤の線と注記へのずれ（212・213）は
/// 差なので、平行移動を含めずに写す（決定 0191）。なければ 0 なので書かない
fn directed(values: &mut Vec<Value>, transform: &Affine) -> Option<()> {
    for code in [212, 213] {
        let at = values.iter().position(|v| v.code == code);
        let next = |at: usize, code: i16| match values.get(at) {
            Some(Value {
                code: found,
                data: Data::Real(v),
            }) if *found == code => Some(*v),
            _ => None,
        };
        if let Some(at) = at {
            let [x, y] = transform.vector([next(at, code)?, next(at + 1, code + 10)?]);
            values.get_mut(at)?.data = Data::Real(x);
            values.get_mut(at + 1)?.data = Data::Real(y);
        }
    }
    let at = |values: &[Value], code: i16| values.iter().position(|v| v.code == code);
    let read = |values: &[Value], code: i16| match values.iter().find(|v| v.code == code) {
        Some(Value {
            data: Data::Real(v),
            ..
        }) => Some(Some(*v)),
        None => Some(None),
        Some(_) => None,
    };
    let was = match [read(values, 211)?, read(values, 221)?, read(values, 231)?] {
        [None, None, None] => [1.0, 0.0],
        [Some(x), Some(y), z] if z.unwrap_or(0.0) == 0.0 => [x, y],
        _ => return None,
    };
    let [x, y] = transform.direction(was);
    let length = (x * x + y * y).sqrt();
    if length == 0.0 {
        return None;
    }
    let [x, y] = [x / length, y / length];
    let present = at(values, 211).is_some();
    if !present && [x, y] == [1.0, 0.0] {
        return Some(());
    }
    if present {
        for (code, v) in [(211, x), (221, y), (231, 0.0)] {
            match at(values, code) {
                Some(i) => values.get_mut(i)?.data = Data::Real(v),
                None => insert(values, code, Data::Real(v), &[code - 10])?,
            }
        }
    } else {
        // DXF Reference の並びでは、X 軸の向きは押し出しの向き（210）の後、ずれ（212）の前に置く
        let place = values
            .iter()
            .position(|v| matches!(v.code, 212 | 213 | 1001))
            .unwrap_or(values.len());
        let place = values
            .iter()
            .rposition(|v| matches!(v.code, 210 | 220 | 230))
            .map_or(place, |i| i + 1);
        values.splice(
            place..place,
            [(211, x), (221, y), (231, 0.0)].map(|(code, v)| Value {
                code,
                data: Data::Real(v),
            }),
        );
    }
    Some(())
}

/// 記号の属性の値の変更を、続く ATTRIB の値（1）に書く（表題欄の項目の書き換えなど）。値のほかのフィールド・数・順は
/// 変えられない。書式つきの値（複数行の属性）を持つ属性の値も変えられない。INSERT は元の属性のまま書くので、
/// 元の属性に戻した要素と、書き直す ATTRIB を返す。属性の値が変わらなければ、要素をそのまま返す
pub(super) fn revalued<P: Clone + PartialEq>(
    db: &AcadDb,
    positions: &BTreeMap<Ref, usize>,
    was: &Element<P>,
    element: &Element<P>,
    encoder: &Encoder,
    frame: &impl Frame<P>,
) -> Option<(Element<P>, Followers)> {
    let (Shape::Symbol(symbol), Shape::Symbol(original)) = (&element.shape, &was.shape) else {
        return Some((element.clone(), Vec::new()));
    };
    if symbol.attributes == original.attributes {
        return Some((element.clone(), Vec::new()));
    }
    if symbol.attributes.len() != original.attributes.len() {
        return None;
    }
    let mut followers = Vec::new();
    for (attribute, before) in symbol.attributes.iter().zip(&original.attributes) {
        if attribute == before {
            continue;
        }
        let mut same = attribute.clone();
        same.text.content.clone_from(&before.text.content);
        if same != *before || before.formatted.is_some() {
            return None;
        }
        let position = *positions.get(&before.source)?;
        let follower = db.objects().get(position)?;
        if follower.kind != b"ATTRIB" {
            return None;
        }
        let mut values = follower.values.clone();
        lettering(
            &mut values,
            &attribute.text,
            &before.text,
            frame,
            encoder,
            74,
        )?;
        followers.push((position, values));
    }
    let written = Element {
        shape: Shape::Symbol(Symbol {
            attributes: original.attributes.clone(),
            ..symbol.clone()
        }),
        ..element.clone()
    };
    Some((written, followers))
}

/// 位置合わせされた文字の、基点からの相対位置を保つ第1点も変換する。
fn aligned_first<P>(
    values: &mut Vec<Value>,
    original: &[Value],
    before: &Text<P>,
    after: &Text<P>,
    transform: &Affine,
    frame: &impl Frame<P>,
    vertical_code: i16,
) -> Option<()> {
    let aligned = [72, vertical_code].iter().any(|code| {
        original
            .iter()
            .any(|v| v.code == *code && v.data != Data::Int(0))
    });
    let (Placement::Anchor { point: old, .. }, Placement::Anchor { point: new, .. }) =
        (&before.placement, &after.placement)
    else {
        return Some(());
    };
    if !aligned {
        return Some(());
    }
    let [old_x, old_y] = frame.point(old)?;
    let [new_x, new_y] = frame.point(new)?;
    let [x, y] = transform.vector([
        first_real(original, 10)? - old_x,
        first_real(original, 20)? - old_y,
    ]);
    put(values, 10, new_x + x, &[])?;
    put(values, 20, new_y + y, &[])?;
    Some(())
}

/// 図形とともに動く XDATA を、AutoCAD と同じ規則で直す（DXF Reference「Group Codes That Apply to Extended
/// Data」）。位置（1011・1021）は変換で写し、変位（1012・1022）は平行移動を除いて写し、向き（1013・1023）は
/// 写してから元の長さにそろえる。距離（1041）と倍率（1042）は尺度を掛け、相似でない変換では直さない
pub(super) fn carried(values: &mut [Value], transform: &Affine) -> Option<()> {
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

#[cfg(test)]
mod tests {
    use super::*;
    use zumen_base::{Budget, CancelToken, Kind, Limit, Limits};

    fn context<'a>(
        budget: &'a mut Budget,
        diagnostics: &'a mut Diagnostics,
        cancel: &'a CancelToken,
    ) -> Cx<'a> {
        Cx::new(budget, diagnostics, cancel)
    }

    #[test]
    fn transform_values_obey_work_and_cancellation() {
        use crate::db::Origin;
        let object = Object {
            kind: b"LINE".to_vec(),
            section: Section::Entities,
            values: vec![
                Value {
                    code: 1001,
                    data: Data::Text(b"app".to_vec()),
                },
                Value {
                    code: 1004,
                    data: Data::Binary(vec![0; 4096]),
                },
                Value {
                    code: 1011,
                    data: Data::Real(2.0),
                },
            ],
            origin: Origin::Dxf { record: 1 },
        };
        let at = Ref::at_record(1);
        let mut diagnostics = Diagnostics::new();
        let cancel = CancelToken::new();
        let mut budget = Budget::new(Limits::default().with(Kind::Work, Limit::Finite(4)));
        assert!(matches!(
            count_transform_values(
                &object,
                &at,
                &mut context(&mut budget, &mut diagnostics, &cancel)
            ),
            Err(Error::Stopped(_))
        ));
        let mut budget = Budget::default();
        cancel.cancel();
        assert!(matches!(
            count_transform_values(
                &object,
                &at,
                &mut context(&mut budget, &mut diagnostics, &cancel)
            ),
            Err(Error::Stopped(_))
        ));
        assert_eq!(budget.used(Kind::Work), 0);
    }

    #[test]
    fn transform_attribute_search_obeys_work_and_cancellation() {
        use crate::db::Origin;
        use zumen_drawing::{Attribute, Height, HeightMeaning, SymbolKey};
        let point = ModelPoint {
            space: SpaceId(0),
            x: 0.0,
            y: 0.0,
        };
        let attribute = Attribute {
            tag: "A".into(),
            text: Text {
                content: "x".into(),
                height: Height {
                    value: Length::Model(ModelLength {
                        space: SpaceId(0),
                        value: 1.0,
                    }),
                    meaning: HeightMeaning::Frame,
                },
                placement: Placement::Anchor {
                    point,
                    horizontal: Horizontal::Left,
                    vertical: Vertical::Baseline,
                },
                rotation: 0.0,
                width_factor: 1.0,
                oblique: 0.0,
                backward: false,
                upside_down: false,
                style: String::new(),
            },
            formatted: None,
            hidden: false,
            layer: LayerId(0),
            style: Style::BLOCK,
            source: Ref::at_record(99),
        };
        let element = Element {
            source: Ref::at_record(0),
            layer: LayerId(0),
            style: Style::BLOCK,
            shape: Shape::Symbol(Symbol {
                key: SymbolKey {
                    namespace: "dxf.block".into(),
                    name: "B".into(),
                },
                position: point,
                scale: [1.0, 1.0],
                rotation: 0.0,
                grid: None,
                attributes: vec![attribute],
                clip: None,
            }),
        };
        let object = Object {
            kind: b"INSERT".to_vec(),
            section: Section::Entities,
            values: vec![],
            origin: Origin::Dxf { record: 0 },
        };
        let mut db = AcadDb::default();
        for record in 1..=100 {
            db.push(Object {
                kind: b"ATTRIB".to_vec(),
                section: Section::Entities,
                values: vec![],
                origin: Origin::Dxf { record },
            });
        }
        let mut diagnostics = Diagnostics::new();
        let cancel = CancelToken::new();
        let mut budget = Budget::new(Limits::default().with(Kind::Work, Limit::Finite(10)));
        assert!(matches!(
            object_positions(&db, &mut context(&mut budget, &mut diagnostics, &cancel)),
            Err(Error::Stopped(_))
        ));
        assert_eq!(budget.used(Kind::Work), 10);
        let mut budget = Budget::default();
        let positions =
            object_positions(&db, &mut context(&mut budget, &mut diagnostics, &cancel)).unwrap();
        // 索引を作った後は、ほかの 99 件をなめずに属性を引ける。
        let mut budget = Budget::new(Limits::default().with(Kind::Work, Limit::Finite(1)));
        assert!(
            transform_work(
                &db,
                &positions,
                &element,
                &object,
                &mut context(&mut budget, &mut diagnostics, &cancel)
            )
            .is_ok()
        );
        assert_eq!(budget.used(Kind::Work), 1);
        let mut budget = Budget::new(Limits::default().with(Kind::Work, Limit::Finite(0)));
        assert!(matches!(
            transform_work(
                &db,
                &positions,
                &element,
                &object,
                &mut context(&mut budget, &mut diagnostics, &cancel)
            ),
            Err(Error::Stopped(_))
        ));
        assert_eq!(budget.used(Kind::Work), 0);
        let mut budget = Budget::default();
        cancel.cancel();
        assert!(matches!(
            transform_work(
                &db,
                &positions,
                &element,
                &object,
                &mut context(&mut budget, &mut diagnostics, &cancel)
            ),
            Err(Error::Stopped(_))
        ));
        assert_eq!(budget.used(Kind::Work), 0);
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
}
