//! 要素を複写する（`Edit::Copy`、#889）。元のレコードの値を写し、[`moved`] で写しを変換して、元と同じ所有者の
//! 新しいレコードとして足す。レイヤ・色・線種・線種の倍率・線幅・透過性・XDATA などの値は、元のレコードのまま
//! 写す。図形とともに動く XDATA と記号の属性（ATTRIB）は、[`Edit::Transform`] と同じ書き方で直す。
//!
//! 写しが元と同じ相手を指すと壊れる参照は写さず、変更の一覧全体を拒む。リアクターと拡張辞書（102 の組と 360。
//! DXF Reference「Common Group Codes for Entities」）、XDATA のハンドル（1005）、ほかの図形を指すハンドル
//! （関連付けたハッチングの境界・注記を指す引出線など）が、それにあたる。表の項目（レイヤ・線種・文字スタイル・
//! 記号の定義のブロックのレコードなど）を指すハンドルは、同じ文書の中なので、そのまま写す

use super::*;

/// 写せる図形と、続くレコードの種類。寸法・表は名前のない記号の定義を描き、写しが元と定義を分け合うので写さない
const COPYABLE: &[&[u8]] = &[
    b"LINE",
    b"POINT",
    b"CIRCLE",
    b"ARC",
    b"ELLIPSE",
    b"LWPOLYLINE",
    b"POLYLINE",
    b"SPLINE",
    b"TEXT",
    b"MTEXT",
    b"INSERT",
    b"SOLID",
    b"TRACE",
    b"HATCH",
    b"RAY",
    b"XLINE",
    b"LEADER",
];

/// 続くレコードの種類
const FOLLOWERS: &[&[u8]] = &[b"VERTEX", b"ATTRIB", b"SEQEND"];

/// 写せない理由。診断の `field` と `value`
type Refusal = (&'static str, &'static str);

/// 種類 `kind` を、足すレコードの種類として引く
fn kind_in(list: &[&'static [u8]], kind: &[u8]) -> Option<&'static [u8]> {
    list.iter().copied().find(|known| *known == kind)
}

/// 要素を複写する（`Edit::Copy`）。写せなければ、理由を `change.copy.unsupported` で返して拒む
pub(super) fn copy_element<'e>(
    current: &Current<'_>,
    plan: &mut Plan<'e>,
    target_of: &'e Seen,
    transform: &Affine,
    edit_index: usize,
    cx: &mut Cx,
) -> Result<(), Error> {
    let Current {
        db,
        index,
        positions,
        target,
        ..
    } = *current;
    let Some(found) = plan.seen(&index.elements, target_of, index, cx)? else {
        return Ok(());
    };
    let (at, kind) = match found {
        Found::Model(element) => (&element.source, kind_of(&element.shape)),
        Found::Paper(element) => (&element.source, kind_of(&element.shape)),
    };
    let refuse = |plan: &mut Plan<'e>, (field, value): Refusal, cx: &mut Cx| {
        plan.reject(
            Diagnostic::new(codes::CHANGE_COPY_UNSUPPORTED)
                .with("edit", edit_index as u64)
                .with("kind", kind)
                .with("field", field)
                .with("value", value)
                .with("format", if target == Target::Dxf { "DXF" } else { "DWG" })
                .at(at.clone()),
            cx,
        );
    };
    // 先の変更が変えた要素は、変える前の値で写すことになるので写さない
    if plan.targets.contains(target_of.id.as_str()) {
        refuse(plan, ("source", "changed"), cx);
        return Ok(());
    }
    let object = (!index.shared.contains(at))
        .then(|| positions.get(at).copied())
        .flatten()
        .and_then(|position| Some((position, db.objects().get(position)?)));
    let Some((position, object)) = object else {
        refuse(plan, ("source", "shared"), cx);
        return Ok(());
    };
    if plan.rebuilt_removed.contains(&position) {
        refuse(plan, ("source", "changed"), cx);
        return Ok(());
    }
    match copied(current, plan, found, position, object, transform, cx)? {
        Ok(record) => {
            plan.added_records += record.count();
            plan.added.push(record);
        }
        Err(refusal) => refuse(plan, refusal, cx),
    }
    Ok(())
}

/// 写しのレコード。写せなければ、その理由
fn copied(
    current: &Current<'_>,
    plan: &mut Plan<'_>,
    found: Found<'_>,
    position: usize,
    object: &Object,
    transform: &Affine,
    cx: &mut Cx,
) -> Result<Result<add::Record, Refusal>, Error> {
    let Current {
        db,
        born,
        positions,
        frames,
        encoder,
        ..
    } = *current;
    let at = match found {
        Found::Model(element) => &element.source,
        Found::Paper(element) => &element.source,
    };
    let Some(kind) = kind_in(COPYABLE, &object.kind) else {
        return Ok(Err(("kind", "unsupported")));
    };
    let Some(next) = born.next_handle() else {
        return Ok(Err(("handles", "missing")));
    };
    // 続く ATTRIB の数が図面モデルの属性の数と合わなければ、写しが属性を落とす
    let followers = trailing(db, position, object, cx, at)?;
    if let Some(lifted) = attributes_of(found) {
        let filed = followers
            .iter()
            .filter(|(_, follower)| follower.kind == b"ATTRIB")
            .count();
        if filed != lifted {
            return Ok(Err(("attributes", "unsupported")));
        }
    }
    // 変換した値。動かす前に、値の走査と複製を予算に含める
    let moved = match found {
        Found::Model(was) => {
            transform_work(db, positions, was, object, cx)?;
            anchor(&was.shape).and_then(|anchor| {
                let frame = InSpace(anchor.space);
                moved(db, positions, was, object, transform, encoder, &frame, true)
            })
        }
        Found::Paper(was) => {
            transform_work(db, positions, was, object, cx)?;
            anchor(&was.shape).and_then(|anchor| {
                let frame = OnSheet(anchor.sheet, *frames.get(&anchor.sheet)?);
                moved(
                    db, positions, was, object, transform, encoder, &frame, false,
                )
            })
        }
    };
    let Some(moved) = moved else {
        return Ok(Err(("transform", "unsupported")));
    };
    let values = match own_values(db, &moved.object, cx, at)? {
        Ok(values) => values,
        Err(refusal) => return Ok(Err(refusal)),
    };
    let mut written = Vec::with_capacity(followers.len());
    for (at_follower, follower) in followers {
        let Some(kind) = kind_in(FOLLOWERS, &follower.kind) else {
            return Ok(Err(("followers", "unsupported")));
        };
        // VERTEX と ATTRIB は、動かした値がなければ写せない。SEQEND は点を持たないので、そのまま写す
        let source = match moved.followers.iter().find(|(p, _)| *p == at_follower) {
            Some((_, values)) => values,
            None if kind == b"SEQEND" => &follower.values,
            None => return Ok(Err(("followers", "unsupported"))),
        };
        match own_values(db, source, cx, at)? {
            Ok(values) => written.push((kind, values)),
            Err(refusal) => return Ok(Err(refusal)),
        }
    }
    // 元と同じ所有者に置く。BLOCKS の所の図形は、所有者のブロックの ENDBLK の前に置くので、所有者が要る
    let blocks = object.section == Section::Blocks;
    let owner = object.owner();
    if blocks && owner.is_none() {
        return Ok(Err(("owner", "missing")));
    }
    let record = add::Record {
        kind,
        values,
        home: Home {
            owner,
            blocks,
            paper: false,
        },
        followers: written,
        annotation: None,
    };
    // 足した後の HANDSEED まで付けられることを確かめる
    if next
        .checked_add(plan.added_records)
        .and_then(|first| first.checked_add(record.count()))
        .and_then(|last| last.checked_add(1))
        .is_none()
    {
        return Ok(Err(("handles", "exhausted")));
    }
    Ok(Ok(record))
}

/// 記号なら、図面モデルの属性の数
fn attributes_of(found: Found<'_>) -> Option<usize> {
    match found {
        Found::Model(element) => match &element.shape {
            Shape::Symbol(symbol) => Some(symbol.attributes.len()),
            _ => None,
        },
        Found::Paper(element) => match &element.shape {
            Shape::Symbol(symbol) => Some(symbol.attributes.len()),
            _ => None,
        },
    }
}

/// 図形（表の `position`）のすぐ後に並び、図形が所有する VERTEX・ATTRIB と、それを閉じる SEQEND。所有者（330）を
/// 持つものは、それが図形か、図形と同じ所有者（ezdxf 1.4.5 の書き方）を指す間だけ取る。続くレコードを持たない
/// 図形なら空
fn trailing<'a>(
    db: &'a AcadDb,
    position: usize,
    object: &Object,
    cx: &mut Cx,
    at: &Ref,
) -> Result<Vec<(usize, &'a Object)>, Error> {
    let mut found = Vec::new();
    if !followed(object) {
        return Ok(found);
    }
    let mine = [object.handle(), object.owner()];
    for (index, follower) in db.objects().iter().enumerate().skip(position + 1) {
        work(cx, 1, at)?;
        let ends = match follower.kind.as_slice() {
            b"VERTEX" | b"ATTRIB" => false,
            b"SEQEND" => true,
            _ => break,
        };
        if follower
            .owner()
            .is_some_and(|owner| !mine.contains(&Some(owner)))
        {
            break;
        }
        found.push((index, follower));
        if ends {
            break;
        }
    }
    Ok(found)
}

/// 足すレコードに書く、ハンドル（5）と所有者（330）の後の値。写すと元と同じ相手を指して壊れる参照があれば、
/// その理由
fn own_values(
    db: &AcadDb,
    values: &[Value],
    cx: &mut Cx,
    at: &Ref,
) -> Result<Result<Vec<Value>, Refusal>, Error> {
    let owner = values.iter().position(|value| value.code == 330);
    let handle = values.iter().position(|value| value.code == 5);
    let mut own = Vec::with_capacity(values.len());
    let mut xdata = false;
    for (index, value) in values.iter().enumerate() {
        work(cx, 1, at)?;
        if Some(index) == handle || Some(index) == owner {
            continue;
        }
        xdata |= value.code == 1001;
        match (value.code, &value.data) {
            (102, _) => return Ok(Err(("reference", "group"))),
            (360, _) => return Ok(Err(("reference", "owned"))),
            (1005, _) => return Ok(Err(("reference", "xdata_handle"))),
            (_, Data::Handle(0)) => {}
            (code, Data::Handle(target)) if !xdata => {
                if code == 330 {
                    return Ok(Err(("reference", "entity")));
                }
                let entity = db.get(*target).is_some_and(|object| {
                    matches!(object.section, Section::Entities | Section::Blocks)
                        && !matches!(object.kind.as_slice(), b"BLOCK" | b"ENDBLK")
                });
                if entity {
                    return Ok(Err(("reference", "entity")));
                }
            }
            _ => {}
        }
        own.push(value.clone());
    }
    Ok(Ok(own))
}
