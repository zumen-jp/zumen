//! 要素を複写する（`Edit::Copy`、#889）。元のレコードの値を写し、[`moved`] で写しを変換して、配置先の所有者の
//! 新しいレコードとして足す。レイヤ・色・線種・線種の倍率・線幅・透過性・XDATA などの値は、元のレコードのまま
//! 写す。図形とともに動く XDATA と記号の属性（ATTRIB）は、[`Edit::Transform`] と同じ書き方で直す。
//! DXF の寸法（DIMENSION・ARC_DIMENSION）と表（ACAD_TABLE）は、写しごとに新しい匿名ブロック（`*D<n>`・`*T<n>`）を
//! 作り、元の表示ブロックと分け合わないようにする。
//!
//! 写しが元と同じ相手を指すと壊れる参照は写さず、変更の一覧全体を拒む。リアクターと拡張辞書（102 の組と 360。
//! DXF Reference「Common Group Codes for Entities」）、XDATA のハンドル（1005）、ほかの図形を指すハンドル
//! （関連付けたハッチングの境界・注記を指す引出線など）が、それにあたる。別の空間・用紙・文書へ写すときは、
//! 単位・画層・線種・文字スタイル・寸法スタイル・記号定義・APPID の対応と名前の衝突を確かめ、保てなければ全体を拒む。

use super::*;
use zumen_drawing::{Fill, Unit};

mod display;

/// DXF・DWG 共通で写せる図形
const COPYABLE: &[&[u8]] = &[
    b"LINE",
    b"POINT",
    b"CIRCLE",
    b"ARC",
    b"ELLIPSE",
    b"LWPOLYLINE",
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

/// DXF だけでそのまま写せる図形
const DXF_ONLY_COPYABLE: &[&[u8]] = &[b"POLYLINE", b"MULTILEADER"];

/// DXF で新しい匿名ブロック（`*D<n>`・`*T<n>`）を作って写す図形
const PARTS_COPYABLE: &[&[u8]] = &[b"DIMENSION", b"ARC_DIMENSION", b"ACAD_TABLE"];

/// 続くレコードの種類
const FOLLOWERS: &[&[u8]] = &[b"VERTEX", b"ATTRIB", b"SEQEND"];

type Followers<'a> = Vec<(usize, &'a Object)>;

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
    spec: CopySpec<'_>,
    edit_index: usize,
    cx: &mut Cx,
) -> Result<(), Error> {
    if let Some(source) = spec.from {
        return copy_from_other(
            current, plan, source, target_of, transform, spec.place, edit_index, cx,
        );
    }
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
    let dest_place = match spec.place {
        Some(place_id) => {
            let Some(&place) = index.places.get(place_id) else {
                missing(place_id, cx);
                plan.rejected = true;
                return Ok(());
            };
            Some(place)
        }
        None => None,
    };
    match copied(
        current, plan, found, position, object, transform, dest_place, cx,
    )? {
        Ok(Some((record, extra))) => {
            let mut total = record.count();
            for item in &extra {
                total = total.saturating_add(item.count());
            }
            plan.added_records = plan.added_records.saturating_add(total);
            plan.copy_extra_records.push(extra.len());
            plan.added.push(record);
            plan.added.extend(extra);
        }
        Ok(None) => {}
        Err(refusal) => refuse(plan, refusal, cx),
    }
    Ok(())
}

/// 別の文書 `source` の要素を `current` の配置先へ複写する
#[allow(clippy::too_many_arguments)]
fn copy_from_other<'e>(
    current: &Current<'_>,
    plan: &mut Plan<'e>,
    source: &SourceDb,
    target_of: &'e Seen,
    transform: &Affine,
    place_id: Option<&str>,
    edit_index: usize,
    cx: &mut Cx,
) -> Result<(), Error> {
    let src_index = Index::new(&source.drawing, cx)?;
    let Some(found) = plan.seen(&src_index.elements, target_of, &src_index, cx)? else {
        return Ok(());
    };
    let (at, kind) = match found {
        Found::Model(element) => (&element.source, kind_of(&element.shape)),
        Found::Paper(element) => (&element.source, kind_of(&element.shape)),
    };
    let format = if current.target == Target::Dxf {
        "DXF"
    } else {
        "DWG"
    };
    let refuse = |plan: &mut Plan<'e>, (field, value): Refusal, cx: &mut Cx| {
        plan.reject(
            Diagnostic::new(codes::CHANGE_COPY_UNSUPPORTED)
                .with("edit", edit_index as u64)
                .with("kind", kind)
                .with("field", field)
                .with("value", value)
                .with("format", format)
                .at(at.clone()),
            cx,
        );
    };
    let default_place = target_of.id.rsplit_once('/').map(|(p, _)| p);
    let Some(place_id) = place_id.or(default_place) else {
        refuse(plan, ("place", "missing"), cx);
        return Ok(());
    };
    let Some(&dest_place) = current.index.places.get(place_id) else {
        missing(place_id, cx);
        plan.rejected = true;
        return Ok(());
    };
    let object = (!src_index.shared.contains(at))
        .then(|| source.positions.get(at).copied())
        .flatten()
        .and_then(|position| Some((position, source.db.objects().get(position)?)));
    let Some((position, object)) = object else {
        refuse(plan, ("source", "shared"), cx);
        return Ok(());
    };
    match copied_from_source(
        current, plan, source, &src_index, found, position, object, transform, dest_place, cx,
    )? {
        Ok(Some(record)) => {
            plan.added_records = plan.added_records.saturating_add(record.count());
            plan.copy_extra_records.push(0);
            plan.added.push(record);
        }
        Ok(None) => {}
        Err(refusal) => refuse(plan, refusal, cx),
    }
    Ok(())
}

/// 同じ文書内の写しのレコードと、付随する匿名ブロックレコード。サイクル拒否のように別の診断を足したときは `Ok(None)`
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn copied(
    current: &Current<'_>,
    plan: &mut Plan<'_>,
    found: Found<'_>,
    position: usize,
    object: &Object,
    transform: &Affine,
    dest_place: Option<Place>,
    cx: &mut Cx,
) -> Result<Result<Option<(add::Record, Vec<add::Record>)>, Refusal>, Error> {
    let Current {
        db,
        drawing,
        index,
        born,
        positions,
        frames,
        encoder,
        target,
        ..
    } = *current;
    let at = match found {
        Found::Model(element) => &element.source,
        Found::Paper(element) => &element.source,
    };
    let src_place = match found {
        Found::Model(was) => anchor(&was.shape).map(|a| Place::Space(a.space)),
        Found::Paper(was) => anchor(&was.shape).map(|a| Place::Sheet(a.sheet)),
    };
    let Some(src_place) = src_place else {
        return Ok(Err(("transform", "unsupported")));
    };
    let dst_place = dest_place.unwrap_or(src_place);
    let cross_place = dst_place != src_place;

    let Some(next) = born.next_handle() else {
        return Ok(Err(("handles", "missing")));
    };

    // 配置先の Home と単位・入れ子・動く XDATA を検査する
    let (home, eff_transform, allow_moving_xdata) = if !cross_place {
        let blocks = object.section == Section::Blocks;
        let owner = object.owner();
        if blocks && owner.is_none() {
            return Ok(Err(("owner", "missing")));
        }
        if blocks
            && !owner.is_some_and(|o| {
                born.home_ready(Home {
                    owner: Some(o),
                    blocks: true,
                    paper: false,
                })
            })
        {
            return Ok(Err(("owner", "missing")));
        }
        let paper = object
            .values
            .iter()
            .any(|v| v.code == 67 && v.data == Data::Int(1));
        (
            Home {
                owner,
                blocks,
                paper,
            },
            *transform,
            src_place == Place::Space(MODEL),
        )
    } else {
        let src_unit = place_unit(drawing, index, src_place);
        let dst_unit = place_unit(drawing, index, dst_place);
        if src_unit.is_none() || src_unit != dst_unit {
            return Ok(Err(("unit", "mismatch")));
        }
        if moving_xdata(object).is_some() {
            return Ok(Err(("xdata", "world_space")));
        }
        let home = match resolve_dest_home(current, plan, found, dst_place, cx)? {
            Ok(Some(home)) => home,
            Ok(None) => return Ok(Ok(None)),
            Err(refusal) => return Ok(Err(refusal)),
        };
        let Some(eff) = cross_place_affine(frames, src_place, dst_place, transform) else {
            return Ok(Err(("place", "unsupported")));
        };
        (home, eff, false)
    };

    // DXF の寸法・表は、新しい匿名ブロックを作って写す
    if let Some(parts_kind) = kind_in(PARTS_COPYABLE, &object.kind) {
        if target != Target::Dxf {
            return Ok(Err(("kind", "unsupported")));
        }
        let destination_paper = matches!(dst_place, Place::Sheet(_));
        let built = match found {
            Found::Model(was) => {
                transform_work(db, positions, was, object, cx)?;
                let Place::Space(space) = src_place else {
                    return Ok(Err(("transform", "unsupported")));
                };
                parts::copy_parts(
                    current,
                    plan,
                    object,
                    was,
                    &eff_transform,
                    &InSpace(space),
                    allow_moving_xdata,
                    destination_paper,
                    cx,
                )?
            }
            Found::Paper(was) => {
                transform_work(db, positions, was, object, cx)?;
                let Place::Sheet(sheet) = src_place else {
                    return Ok(Err(("transform", "unsupported")));
                };
                let Some(&paper_frame) = frames.get(&sheet) else {
                    return Ok(Err(("transform", "unsupported")));
                };
                parts::copy_parts(
                    current,
                    plan,
                    object,
                    was,
                    &eff_transform,
                    &OnSheet(sheet, paper_frame),
                    false,
                    destination_paper,
                    cx,
                )?
            }
        };
        let Some((raw_values, extra, lower_name)) = built else {
            return Ok(Err(("transform", "unsupported")));
        };
        let mut values = match own_values(db, &raw_values, false, cx, at)? {
            Ok(values) => values,
            Err(refusal) => return Ok(Err(refusal)),
        };
        if cross_place {
            let want_paper = home.paper && !(home.blocks && target != Target::Dxf);
            set_paper_flag(&mut values, want_paper);
        }
        plan.anon_names.insert(lower_name);
        let record = add::Record {
            kind: parts_kind,
            values,
            home,
            followers: Vec::new(),
            annotation: None,
        };
        return Ok(Ok(Some((record, extra))));
    }

    let kind = kind_in(COPYABLE, &object.kind).or_else(|| {
        (target == Target::Dxf)
            .then(|| kind_in(DXF_ONLY_COPYABLE, &object.kind))
            .flatten()
    });
    let Some(kind) = kind else {
        return Ok(Err(("kind", "unsupported")));
    };

    let followers = if followed(object) {
        let attached = plan.attached_for(db, at, cx)?;
        match followers_from_attached(attached, db, position, cx, at)? {
            Ok(found) => found,
            Err(refusal) => return Ok(Err(refusal)),
        }
    } else {
        Vec::new()
    };
    if let Some(lifted) = attributes_of(found) {
        let filed = followers
            .iter()
            .filter(|(_, follower)| follower.kind == b"ATTRIB")
            .count();
        if filed != lifted {
            return Ok(Err(("attributes", "unsupported")));
        }
    }

    if let Err(refusal) = check_dwg_entity(db, born, target, object, &followers) {
        return Ok(Err(refusal));
    }

    let moved = match found {
        Found::Model(was) => {
            transform_work(db, positions, was, object, cx)?;
            let Place::Space(space) = src_place else {
                return Ok(Err(("transform", "unsupported")));
            };
            moved_for_copy(
                db,
                positions,
                was,
                object,
                &followers,
                &eff_transform,
                encoder,
                &InSpace(space),
                allow_moving_xdata,
            )
        }
        Found::Paper(was) => {
            transform_work(db, positions, was, object, cx)?;
            let Place::Sheet(sheet) = src_place else {
                return Ok(Err(("transform", "unsupported")));
            };
            let Some(&paper_frame) = frames.get(&sheet) else {
                return Ok(Err(("transform", "unsupported")));
            };
            moved_for_copy(
                db,
                positions,
                was,
                object,
                &followers,
                &eff_transform,
                encoder,
                &OnSheet(sheet, paper_frame),
                false,
            )
        }
    };
    let Some(moved) = moved else {
        return Ok(Err(("transform", "unsupported")));
    };
    let mut values = match own_values(db, &moved.object, false, cx, at)? {
        Ok(values) => values,
        Err(refusal) => return Ok(Err(refusal)),
    };
    let want_paper = home.paper && !(home.blocks && target != Target::Dxf);
    let want_follower_paper = home.paper && target == Target::Dxf;
    if cross_place {
        set_paper_flag(&mut values, want_paper);
    }
    let moved_followers: BTreeMap<_, _> = moved.followers.into_iter().collect();
    let mut written = Vec::with_capacity(followers.len());
    for (at_follower, follower) in followers {
        let Some(f_kind) = kind_in(FOLLOWERS, &follower.kind) else {
            return Ok(Err(("followers", "unsupported")));
        };
        let source = match moved_followers.get(&at_follower) {
            Some(values) => values,
            None if f_kind == b"SEQEND" => &follower.values,
            None => return Ok(Err(("followers", "unsupported"))),
        };
        match own_values(db, source, false, cx, at)? {
            Ok(mut f_values) => {
                if cross_place || target != Target::Dxf {
                    set_paper_flag(&mut f_values, want_follower_paper);
                }
                written.push((f_kind, f_values));
            }
            Err(refusal) => return Ok(Err(refusal)),
        }
    }

    let record = add::Record {
        kind,
        values,
        home,
        followers: written,
        annotation: None,
    };
    if next
        .checked_add(plan.added_records)
        .and_then(|first| first.checked_add(record.count()))
        .and_then(|last| last.checked_add(1))
        .is_none()
    {
        return Ok(Err(("handles", "exhausted")));
    }
    Ok(Ok(Some((record, Vec::new()))))
}

/// 別文書からの複写レコードを組み立てる
#[allow(clippy::too_many_arguments)]
fn copied_from_source(
    current: &Current<'_>,
    plan: &mut Plan<'_>,
    source: &SourceDb,
    src_index: &Index<'_>,
    found: Found<'_>,
    position: usize,
    object: &Object,
    transform: &Affine,
    dst_place: Place,
    cx: &mut Cx,
) -> Result<Result<Option<add::Record>, Refusal>, Error> {
    let Current {
        db: dst_db,
        drawing: dst_drawing,
        index: dst_index,
        born: dst_born,
        frames: dst_frames,
        encoder: dst_encoder,
        target,
        ..
    } = *current;
    let at = match found {
        Found::Model(element) => &element.source,
        Found::Paper(element) => &element.source,
    };
    let Some(next) = dst_born.next_handle() else {
        return Ok(Err(("handles", "missing")));
    };
    let src_place = match found {
        Found::Model(was) => anchor(&was.shape).map(|a| Place::Space(a.space)),
        Found::Paper(was) => anchor(&was.shape).map(|a| Place::Sheet(a.sheet)),
    };
    let Some(src_place) = src_place else {
        return Ok(Err(("transform", "unsupported")));
    };

    // 1. 単位の一致を検査する
    let src_unit = place_unit(&source.drawing, src_index, src_place);
    let dst_unit = place_unit(dst_drawing, dst_index, dst_place);
    if src_unit.is_none() || src_unit != dst_unit {
        return Ok(Err(("unit", "mismatch")));
    }

    // 2. 種類（別文書では匿名ブロックやオブジェクト辞書ハンドルを伴う種類は写さない）
    let kind = kind_in(COPYABLE, &object.kind).or_else(|| {
        (target == Target::Dxf && object.kind == b"POLYLINE").then_some(b"POLYLINE".as_slice())
    });
    let Some(kind) = kind else {
        return Ok(Err(("kind", "unsupported")));
    };

    let allow_moving_xdata = src_place == Place::Space(MODEL) && dst_place == Place::Space(MODEL);
    if moving_xdata(object).is_some() && !allow_moving_xdata {
        return Ok(Err(("xdata", "world_space")));
    }

    let followers = if followed(object) {
        let attached = source.attached_for(at, cx)?;
        match followers_from_attached(attached, &source.db, position, cx, at)? {
            Ok(found) => found,
            Err(refusal) => return Ok(Err(refusal)),
        }
    } else {
        Vec::new()
    };
    if let Some(lifted) = attributes_of(found) {
        let filed = followers
            .iter()
            .filter(|(_, follower)| follower.kind == b"ATTRIB")
            .count();
        if filed != lifted {
            return Ok(Err(("attributes", "unsupported")));
        }
    }

    // 3. 画層・線種・文字スタイル・寸法スタイル・記号定義・APPID の存在と衝突を検査する
    if let Err(refusal) = check_cross_doc_tables(current, source, found, object, &followers, cx)? {
        return Ok(Err(refusal));
    }

    // 4. 配置先の Home とサイクルを検査する
    let home = match resolve_dest_home(current, plan, found, dst_place, cx)? {
        Ok(Some(home)) => home,
        Ok(None) => return Ok(Ok(None)),
        Err(refusal) => return Ok(Err(refusal)),
    };
    let Some(eff_transform) =
        cross_doc_affine(&source.frames, dst_frames, src_place, dst_place, transform)
    else {
        return Ok(Err(("place", "unsupported")));
    };

    let src_encoder = Encoder::new(source.charset);
    let moved = match found {
        Found::Model(was) => {
            transform_work(&source.db, &source.positions, was, object, cx)?;
            let Place::Space(space) = src_place else {
                return Ok(Err(("transform", "unsupported")));
            };
            moved_for_copy(
                &source.db,
                &source.positions,
                was,
                object,
                &followers,
                &eff_transform,
                &src_encoder,
                &InSpace(space),
                allow_moving_xdata,
            )
        }
        Found::Paper(was) => {
            transform_work(&source.db, &source.positions, was, object, cx)?;
            let Place::Sheet(sheet) = src_place else {
                return Ok(Err(("transform", "unsupported")));
            };
            let Some(&paper_frame) = source.frames.get(&sheet) else {
                return Ok(Err(("transform", "unsupported")));
            };
            moved_for_copy(
                &source.db,
                &source.positions,
                was,
                object,
                &followers,
                &eff_transform,
                &src_encoder,
                &OnSheet(sheet, paper_frame),
                false,
            )
        }
    };
    let Some(moved) = moved else {
        return Ok(Err(("transform", "unsupported")));
    };

    let mut values = match own_values(&source.db, &moved.object, true, cx, at)? {
        Ok(values) => values,
        Err(refusal) => return Ok(Err(refusal)),
    };
    if let Err(refusal) = rewrite_cross_doc_values(
        &mut values,
        object.kind.as_slice(),
        source.charset,
        dst_born,
        dst_encoder,
    ) {
        return Ok(Err(refusal));
    }
    let want_paper = home.paper && !(home.blocks && target != Target::Dxf);
    let want_follower_paper = home.paper && target == Target::Dxf;
    set_paper_flag(&mut values, want_paper);

    let moved_followers: BTreeMap<_, _> = moved.followers.into_iter().collect();
    let mut written = Vec::with_capacity(followers.len());
    for (at_follower, follower) in &followers {
        let Some(f_kind) = kind_in(FOLLOWERS, &follower.kind) else {
            return Ok(Err(("followers", "unsupported")));
        };
        let raw_source = match moved_followers.get(at_follower) {
            Some(vals) => vals,
            None if f_kind == b"SEQEND" => &follower.values,
            None => return Ok(Err(("followers", "unsupported"))),
        };
        let mut f_values = match own_values(&source.db, raw_source, true, cx, at)? {
            Ok(vals) => vals,
            Err(refusal) => return Ok(Err(refusal)),
        };
        if let Err(refusal) =
            rewrite_cross_doc_values(&mut f_values, f_kind, source.charset, dst_born, dst_encoder)
        {
            return Ok(Err(refusal));
        }
        set_paper_flag(&mut f_values, want_follower_paper);
        written.push((f_kind, f_values));
    }

    // DWG の書き込み可否を書き換え後の名前で確かめる
    let temp_obj = Object {
        kind: kind.to_vec(),
        section: if home.blocks {
            Section::Blocks
        } else {
            Section::Entities
        },
        values: values.clone(),
        origin: crate::Origin::New,
    };
    let temp_followers: Vec<(usize, Object)> = written
        .iter()
        .enumerate()
        .map(|(i, (fk, fv))| {
            (
                i,
                Object {
                    kind: fk.to_vec(),
                    section: temp_obj.section.clone(),
                    values: fv.clone(),
                    origin: crate::Origin::New,
                },
            )
        })
        .collect();
    let temp_refs: Vec<(usize, &Object)> = temp_followers.iter().map(|(i, o)| (*i, o)).collect();
    if let Err(refusal) = check_dwg_entity(dst_db, dst_born, target, &temp_obj, &temp_refs) {
        return Ok(Err(refusal));
    }

    let record = add::Record {
        kind,
        values,
        home,
        followers: written,
        annotation: None,
    };
    if next
        .checked_add(plan.added_records)
        .and_then(|first| first.checked_add(record.count()))
        .and_then(|last| last.checked_add(1))
        .is_none()
    {
        return Ok(Err(("handles", "exhausted")));
    }
    Ok(Ok(Some(record)))
}

fn place_unit(drawing: &Drawing2D, index: &Index<'_>, place: Place) -> Option<Unit> {
    let unit = match place {
        Place::Space(space) if space == MODEL => {
            drawing.spaces.iter().find(|s| s.id == MODEL)?.unit
        }
        Place::Space(space) => index.definitions.get(&space)?.space.unit,
        Place::Sheet(_) => Unit::Millimeter,
    };
    (unit != Unit::Unknown).then_some(unit)
}

fn resolve_dest_home(
    current: &Current<'_>,
    plan: &mut Plan<'_>,
    found: Found<'_>,
    dst_place: Place,
    cx: &mut Cx,
) -> Result<Result<Option<Home>, Refusal>, Error> {
    match dst_place {
        Place::Space(space) if space == MODEL => {
            let home = current.born.model();
            if current.target != Target::Dxf && home.owner.is_none() {
                return Ok(Err(("place", "unsupported")));
            }
            Ok(Ok(Some(home)))
        }
        Place::Space(space) => {
            let Some(definition) = current.index.definitions.get(&space).copied() else {
                return Ok(Err(("place", "unsupported")));
            };
            let sym_name = match found {
                Found::Model(el) => match &el.shape {
                    Shape::Symbol(s) if s.key.namespace == BLOCK => Some(s.key.name.as_str()),
                    Shape::Symbol(_) => return Ok(Err(("place", "unsupported"))),
                    _ => None,
                },
                Found::Paper(el) => match &el.shape {
                    Shape::Symbol(s) if s.key.namespace == BLOCK => Some(s.key.name.as_str()),
                    Shape::Symbol(_) => return Ok(Err(("place", "unsupported"))),
                    _ => None,
                },
            };
            if let Some(inner_raw) = sym_name
                && definition.key.namespace == BLOCK
            {
                let outer = definition.key.name.to_lowercase();
                let inner = inner_raw.to_lowercase();
                if nests_into(&current.index.nests, &plan.nests, &inner, &outer, cx)? {
                    plan.reject(
                        Diagnostic::new(codes::CHANGE_SYMBOL_CYCLE_INVALID)
                            .with("id", id::space(space))
                            .with("name", inner_raw),
                        cx,
                    );
                    return Ok(Ok(None));
                }
                plan.nests.entry(outer).or_default().insert(inner);
            }
            let home = match found {
                Found::Model(el) => {
                    definition_home(current.db, current.positions, definition, &el.shape)
                }
                Found::Paper(el) => {
                    definition_home(current.db, current.positions, definition, &el.shape)
                }
            };
            let Some(home) = home.filter(|&h| current.born.home_ready(h)) else {
                return Ok(Err(("place", "unsupported")));
            };
            Ok(Ok(Some(home)))
        }
        Place::Sheet(sheet) => {
            let Some(&paper_frame) = current.frames.get(&sheet) else {
                return Ok(Err(("place", "unsupported")));
            };
            let has_pattern = match found {
                Found::Model(el) => {
                    matches!(&el.shape, Shape::Hatch(h) if matches!(h.fill, Fill::Pattern(_)))
                }
                Found::Paper(el) => {
                    matches!(&el.shape, Shape::Hatch(h) if matches!(h.fill, Fill::Pattern(_)))
                }
            };
            if has_pattern && !OnSheet(sheet, paper_frame).hatch_pattern_supported() {
                return Ok(Err(("place", "hatch_pattern")));
            }
            if !current.born.home_ready(paper_frame.home) {
                return Ok(Err(("place", "unsupported")));
            }
            Ok(Ok(Some(paper_frame.home)))
        }
    }
}

fn compose_affine(outer: Affine, inner: Affine) -> Option<Affine> {
    let [a1, b1, c1, d1, e1, f1] = outer.m;
    let [a2, b2, c2, d2, e2, f2] = inner.m;
    let m = [
        a1 * a2 + b1 * d2,
        a1 * b2 + b1 * e2,
        a1 * c2 + b1 * f2 + c1,
        d1 * a2 + e1 * d2,
        d1 * b2 + e1 * e2,
        d1 * c2 + e1 * f2 + f1,
    ];
    m.iter().all(|v| v.is_finite()).then_some(Affine { m })
}

fn cross_place_affine(
    frames: &BTreeMap<SheetId, PaperFrame>,
    src: Place,
    dst: Place,
    transform: &Affine,
) -> Option<Affine> {
    cross_doc_affine(frames, frames, src, dst, transform)
}

fn cross_doc_affine(
    src_frames: &BTreeMap<SheetId, PaperFrame>,
    dst_frames: &BTreeMap<SheetId, PaperFrame>,
    src: Place,
    dst: Place,
    transform: &Affine,
) -> Option<Affine> {
    let bridge = match (src, dst) {
        (Place::Space(_), Place::Space(_)) => Affine::IDENTITY,
        (Place::Space(_), Place::Sheet(sh2)) => {
            let f2 = *dst_frames.get(&sh2)?;
            if f2.mm == 0.0 || !f2.mm.is_finite() {
                return None;
            }
            let s = 1.0 / f2.mm;
            Affine {
                m: [s, 0.0, f2.origin[0], 0.0, s, f2.origin[1]],
            }
        }
        (Place::Sheet(sh1), Place::Space(_)) => {
            let f1 = *src_frames.get(&sh1)?;
            Affine {
                m: [
                    f1.mm,
                    0.0,
                    -f1.mm * f1.origin[0],
                    0.0,
                    f1.mm,
                    -f1.mm * f1.origin[1],
                ],
            }
        }
        (Place::Sheet(sh1), Place::Sheet(sh2)) => {
            let f1 = *src_frames.get(&sh1)?;
            let f2 = *dst_frames.get(&sh2)?;
            if f2.mm == 0.0 || !f2.mm.is_finite() {
                return None;
            }
            let ratio = f1.mm / f2.mm;
            Affine {
                m: [
                    ratio,
                    0.0,
                    f1.mm * (f2.origin[0] - f1.origin[0]),
                    0.0,
                    ratio,
                    f1.mm * (f2.origin[1] - f1.origin[1]),
                ],
            }
        }
    };
    compose_affine(bridge, *transform)
}

fn set_paper_flag(values: &mut Vec<Value>, paper: bool) {
    values.retain(|v| v.code != 67);
    if paper {
        let at = values
            .iter()
            .position(|v| v.code == 100 && matches!(&v.data, Data::Text(t) if t == b"AcDbEntity"))
            .map_or(0, |i| i + 1);
        values.insert(
            at,
            Value {
                code: 67,
                data: Data::Int(1),
            },
        );
    }
}

/// DWG で足す図形が表せないフィールド・参照を事前に検査し、`change.copy.unsupported` の理由を返す
fn check_dwg_entity(
    db: &AcadDb,
    born: &Born,
    target: Target,
    object: &Object,
    followers: &[(usize, &Object)],
) -> Result<(), Refusal> {
    if target == Target::Dxf {
        return Ok(());
    }
    let r2004 = born.since(b"AC1018");
    let has_record = |kind: &[u8], raw: &[u8]| {
        db.objects().iter().any(|rec| {
            rec.section == Section::Tables
                && rec.kind == kind
                && matches!(rec.first(2), Some(Data::Text(n)) if n.eq_ignore_ascii_case(raw))
        })
    };
    for current_obj in
        std::iter::once(object).chain(followers.iter().map(|(_, follower)| *follower))
    {
        if current_obj.kind == b"MTEXT" && current_obj.values.iter().any(|v| v.code == 101) {
            return Err(("kind", "columns"));
        }
        let mut xdata = false;
        for value in &current_obj.values {
            xdata |= value.code == 1001;
            match (value.code, &value.data) {
                (430, _) if !xdata => return Err(("style", "color_book")),
                (420, _) if !xdata && !r2004 => return Err(("style", "true_color")),
                (440, _) if !xdata && !r2004 => return Err(("style", "transparency")),
                (1001, Data::Text(app)) if !has_record(b"APPID", app) => {
                    return Err(("appid", "missing"));
                }
                (1003, Data::Text(layer)) if !has_record(b"LAYER", layer) => {
                    return Err(("layer", "missing"));
                }
                _ => {}
            }
        }
    }
    Ok(())
}

/// 別文書間で参照する画層・線種・文字スタイル・寸法スタイル・記号定義・APPID の存在と定義衝突を確かめる
fn check_cross_doc_tables(
    current: &Current<'_>,
    source: &SourceDb,
    found: Found<'_>,
    object: &Object,
    followers: &[(usize, &Object)],
    cx: &mut Cx,
) -> Result<Result<(), Refusal>, Error> {
    let dst_drawing = current.drawing;
    let dst_born = current.born;
    let dst_db = current.db;
    let at = match found {
        Found::Model(el) => &el.source,
        Found::Paper(el) => &el.source,
    };

    // 記号定義（INSERT）
    let sym_key = match found {
        Found::Model(el) => match &el.shape {
            Shape::Symbol(s) => Some(&s.key),
            _ => None,
        },
        Found::Paper(el) => match &el.shape {
            Shape::Symbol(s) => Some(&s.key),
            _ => None,
        },
    };
    if let Some(key) = sym_key {
        if key.namespace != BLOCK {
            return Ok(Err(("symbol", "unsupported")));
        }
        let Some(src_def) = source
            .drawing
            .symbols
            .iter()
            .find(|s| s.key.namespace == BLOCK && s.key.name.eq_ignore_ascii_case(&key.name))
        else {
            return Ok(Err(("symbol", "missing")));
        };
        let Some(dst_def) = dst_drawing
            .symbols
            .iter()
            .find(|s| s.key.namespace == BLOCK && s.key.name.eq_ignore_ascii_case(&key.name))
        else {
            return Ok(Err(("symbol", "missing")));
        };
        if dst_born
            .named(2, b"BLOCK_RECORD", &dst_def.key.name)
            .is_none()
        {
            return Ok(Err(("symbol", "missing")));
        }
        if !same_symbol_definition(src_def, dst_def, cx, at)?
            || !display::same_in_symbol(&source.drawing, dst_drawing, &key.name, cx)
                .map_err(|stop| display::stopped(cx, at, stop))?
        {
            return Ok(Err(("symbol", "conflict")));
        }
    }

    // グループ 3 が省略された引出線も、読んだときに解決した Standard を照合する。
    let leader = match found {
        Found::Model(el) => match &el.shape {
            Shape::Leader(v) => Some(&v.style),
            _ => None,
        },
        Found::Paper(el) => match &el.shape {
            Shape::Leader(v) => Some(&v.style),
            _ => None,
        },
    };
    if let Some(name) = leader {
        if dst_born.named(3, b"DIMSTYLE", name).is_none() {
            return Ok(Err(("dimension_style", "missing")));
        }
        if let Err(refusal) = display::check_style(&source.drawing, dst_drawing, name, cx)
            .map_err(|stop| display::stopped(cx, at, stop))?
        {
            return Ok(Err(refusal));
        }
    }

    let check_appids = current.target != Target::Dxf
        || dst_db
            .objects()
            .iter()
            .any(|o| o.section == Section::Tables && o.kind == b"APPID");

    for current_obj in
        std::iter::once(object).chain(followers.iter().map(|(_, follower)| *follower))
    {
        let mut xdata = false;
        for value in &current_obj.values {
            work(cx, 1, at)?;
            xdata |= value.code == 1001;
            match (value.code, &value.data) {
                (8, Data::Text(raw)) if !xdata => {
                    let Some(name) = source.charset.decode(raw) else {
                        return Ok(Err(("layer", "encoding")));
                    };
                    let exists = dst_born.named(8, b"LAYER", &name).is_some()
                        && (name.eq_ignore_ascii_case("0")
                            || dst_drawing
                                .layers
                                .iter()
                                .any(|l| l.name.eq_ignore_ascii_case(&name)));
                    if !exists {
                        return Ok(Err(("layer", "missing")));
                    }
                }
                (6, Data::Text(raw)) if !xdata => {
                    let Some(name) = source.charset.decode(raw) else {
                        return Ok(Err(("line_type", "encoding")));
                    };
                    if !name.eq_ignore_ascii_case("ByLayer")
                        && !name.eq_ignore_ascii_case("ByBlock")
                        && !name.eq_ignore_ascii_case("Continuous")
                    {
                        if dst_born.named(6, b"LTYPE", &name).is_none() {
                            return Ok(Err(("line_type", "missing")));
                        }
                        let src_lt = source
                            .drawing
                            .line_types
                            .iter()
                            .find(|l| l.name.eq_ignore_ascii_case(&name));
                        let dst_lt = dst_drawing
                            .line_types
                            .iter()
                            .find(|l| l.name.eq_ignore_ascii_case(&name));
                        match (src_lt, dst_lt) {
                            (Some(s), Some(d)) if s.definition == d.definition => {}
                            (Some(_), Some(_)) => return Ok(Err(("line_type", "conflict"))),
                            _ => return Ok(Err(("line_type", "missing"))),
                        }
                    }
                }
                (7, Data::Text(raw))
                    if !xdata
                        && matches!(
                            current_obj.kind.as_slice(),
                            b"TEXT" | b"MTEXT" | b"ATTRIB" | b"ATTDEF"
                        ) =>
                {
                    let Some(name) = source.charset.decode(raw) else {
                        return Ok(Err(("text_style", "encoding")));
                    };
                    if dst_born.named(7, b"STYLE", &name).is_none() {
                        return Ok(Err(("text_style", "missing")));
                    }
                    let src_ts = source
                        .drawing
                        .text_styles
                        .iter()
                        .find(|s| s.name.eq_ignore_ascii_case(&name));
                    let dst_ts = dst_drawing
                        .text_styles
                        .iter()
                        .find(|s| s.name.eq_ignore_ascii_case(&name));
                    match (src_ts, dst_ts) {
                        (Some(s), Some(d)) if s.definition == d.definition => {}
                        (Some(_), Some(_)) => return Ok(Err(("text_style", "conflict"))),
                        (None, None) if name.eq_ignore_ascii_case("Standard") => {}
                        _ => return Ok(Err(("text_style", "missing"))),
                    }
                }
                (1001, Data::Text(raw)) => {
                    let Some(name) = source.charset.decode(raw) else {
                        return Ok(Err(("appid", "encoding")));
                    };
                    if check_appids {
                        let found_app = dst_db.objects().iter().any(|rec| {
                            rec.section == Section::Tables
                                && rec.kind == b"APPID"
                                && matches!(rec.first(2), Some(Data::Text(n))
                                    if current.charset.decode(n).is_some_and(|dn| dn.eq_ignore_ascii_case(&name)))
                        });
                        if !found_app {
                            return Ok(Err(("appid", "missing")));
                        }
                    }
                }
                (1003, Data::Text(raw)) => {
                    let Some(name) = source.charset.decode(raw) else {
                        return Ok(Err(("layer", "encoding")));
                    };
                    if dst_born.named(8, b"LAYER", &name).is_none() {
                        return Ok(Err(("layer", "missing")));
                    }
                }
                _ => {}
            }
        }
    }
    Ok(Ok(()))
}

fn same_symbol_definition(
    a: &SymbolDefinition,
    b: &SymbolDefinition,
    cx: &mut Cx,
    at: &Ref,
) -> Result<bool, Error> {
    if a.anonymous
        || b.anonymous
        || a.external.is_some()
        || b.external.is_some()
        || a.space.unit != b.space.unit
        || a.attributes.len() != b.attributes.len()
        || a.space.elements.len() != b.space.elements.len()
    {
        return Ok(false);
    }
    let base_a = a.base.map(|p| [p.x, p.y]).unwrap_or([0.0, 0.0]);
    let base_b = b.base.map(|p| [p.x, p.y]).unwrap_or([0.0, 0.0]);
    if base_a != base_b {
        return Ok(false);
    }
    for (ea, eb) in a.space.elements.iter().zip(&b.space.elements) {
        let ha = Hash::element_checked(ea, cx).map_err(|stop| {
            cx.report(stop.to_diagnostic().at(at.clone()));
            Error::Stopped(stop)
        })?;
        let hb = Hash::element_checked(eb, cx).map_err(|stop| {
            cx.report(stop.to_diagnostic().at(at.clone()));
            Error::Stopped(stop)
        })?;
        if ha != hb {
            return Ok(false);
        }
    }
    Ok(true)
}

/// 別文書から写した値の表参照名と文字符号化を、貼り付け先の文書に合わせて書き直す
fn rewrite_cross_doc_values(
    values: &mut [Value],
    kind: &[u8],
    src_charset: Charset,
    dst_born: &Born,
    dst_encoder: &Encoder,
) -> Result<(), Refusal> {
    let mut xdata = false;
    for value in values.iter_mut() {
        xdata |= value.code == 1001;
        let Data::Text(raw) = &value.data else {
            continue;
        };
        let code = value.code;
        if !xdata {
            match code {
                8 => {
                    let s = src_charset.decode(raw).ok_or(("layer", "encoding"))?;
                    *value = dst_born
                        .named(8, b"LAYER", &s)
                        .ok_or(("layer", "missing"))?;
                    continue;
                }
                6 => {
                    let s = src_charset.decode(raw).ok_or(("line_type", "encoding"))?;
                    if s.eq_ignore_ascii_case("ByLayer") {
                        *value = dst_encoder
                            .encode("ByLayer")
                            .map(|b| Value {
                                code: 6,
                                data: Data::Text(b),
                            })
                            .ok_or(("line_type", "encoding"))?;
                    } else if s.eq_ignore_ascii_case("ByBlock") {
                        *value = dst_encoder
                            .encode("ByBlock")
                            .map(|b| Value {
                                code: 6,
                                data: Data::Text(b),
                            })
                            .ok_or(("line_type", "encoding"))?;
                    } else if s.eq_ignore_ascii_case("Continuous") {
                        *value = dst_encoder
                            .encode("Continuous")
                            .map(|b| Value {
                                code: 6,
                                data: Data::Text(b),
                            })
                            .ok_or(("line_type", "encoding"))?;
                    } else {
                        *value = dst_born
                            .named(6, b"LTYPE", &s)
                            .ok_or(("line_type", "missing"))?;
                    }
                    continue;
                }
                7 if matches!(kind, b"TEXT" | b"MTEXT" | b"ATTRIB" | b"ATTDEF") => {
                    let s = src_charset.decode(raw).ok_or(("text_style", "encoding"))?;
                    *value = dst_born
                        .named(7, b"STYLE", &s)
                        .ok_or(("text_style", "missing"))?;
                    continue;
                }
                3 if kind == b"LEADER" => {
                    let s = src_charset
                        .decode(raw)
                        .ok_or(("dimension_style", "encoding"))?;
                    *value = dst_born
                        .named(3, b"DIMSTYLE", &s)
                        .ok_or(("dimension_style", "missing"))?;
                    continue;
                }
                2 if kind == b"INSERT" => {
                    let s = src_charset.decode(raw).ok_or(("symbol", "encoding"))?;
                    *value = dst_born
                        .named(2, b"BLOCK_RECORD", &s)
                        .ok_or(("symbol", "missing"))?;
                    continue;
                }
                _ => {}
            }
        } else if code == 1003 {
            let s = src_charset.decode(raw).ok_or(("layer", "encoding"))?;
            let mut layer_val = dst_born
                .named(8, b"LAYER", &s)
                .ok_or(("layer", "missing"))?;
            layer_val.code = 1003;
            *value = layer_val;
            continue;
        }

        if src_charset != dst_encoder.charset && code != 100 && code != 102 {
            let s = src_charset.decode(raw).ok_or(("encoding", "unsupported"))?;
            let encoded = dst_encoder.encode(&s).ok_or(("encoding", "unsupported"))?;
            value.data = Data::Text(encoded);
        }
    }
    Ok(())
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

/// 旧式の 2D POLYLINE の平行移動では、親の基点を保ち、座標を持つ VERTEX を動かす。
#[allow(clippy::too_many_arguments)]
fn moved_for_copy<P: Copy + PartialEq + zumen_drawing::transform::Point>(
    db: &AcadDb,
    positions: &BTreeMap<Ref, usize>,
    was: &Element<P>,
    object: &Object,
    followers: &[(usize, &Object)],
    transform: &Affine,
    encoder: &Encoder,
    frame: &impl Frame<P>,
    model: bool,
) -> Option<Moved> {
    if object.kind != b"POLYLINE" {
        return moved(db, positions, was, object, transform, encoder, frame, model);
    }
    let Shape::Polyline(polyline) = &was.shape else {
        return None;
    };
    let [1.0, 0.0, _, 0.0, 1.0, _] = transform.m else {
        return None;
    };
    let copy_values = |record: &Object| {
        let mut values = record.values.clone();
        if moving_xdata(record).is_some() {
            if !model {
                return None;
            }
            carried(&mut values, transform)?;
        }
        Some(values)
    };
    let mut vertices = polyline.vertices.iter();
    let mut children = Vec::with_capacity(followers.len());
    for &(position, child) in followers {
        let mut values = copy_values(child)?;
        if child.kind == b"VERTEX" {
            let point = vertices.next()?.point;
            place(
                &mut values,
                frame,
                [10, 20],
                &transform.point(point),
                &point,
            )?;
        }
        children.push((position, values));
    }
    vertices.next().is_none().then_some(())?;
    Some(Moved {
        object: copy_values(object)?,
        followers: children,
    })
}

/// 図形（表の `position`）が所有する VERTEX・ATTRIB と、それを閉じる SEQEND を `VERTEX` / `ATTRIB` → `SEQEND` の順に返す（#914）
fn followers_from_attached<'a>(
    attached: &AttachedPositions,
    db: &'a AcadDb,
    position: usize,
    cx: &mut Cx,
    at: &Ref,
) -> Result<Result<Followers<'a>, Refusal>, Error> {
    let children = attached
        .children
        .get(&position)
        .map_or(&[][..], Vec::as_slice);
    let seqends = attached
        .seqends
        .get(&position)
        .map_or(&[][..], Vec::as_slice);
    if seqends.len() != 1 {
        return Ok(Err(("followers", "unsupported")));
    }
    let mut found = Vec::with_capacity(children.len() + seqends.len());
    for &pos in children.iter().chain(seqends.iter()) {
        work(cx, 1, at)?;
        let Some(follower) = db.objects().get(pos) else {
            return Ok(Err(("followers", "unsupported")));
        };
        if follower.kind == b"VERTEX" || follower.kind == b"SEQEND" {
            count_transform_values(follower, at, cx)?;
        }
        found.push((pos, follower));
    }
    Ok(Ok(found))
}

/// 足すレコードに書く、ハンドル（5）と所有者（330）の後の値。`cross_doc` なら非 0 のハンドル参照をすべて拒む
fn own_values(
    db: &AcadDb,
    values: &[Value],
    cross_doc: bool,
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
            (_, Data::Handle(_)) if cross_doc => return Ok(Err(("reference", "handle"))),
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
