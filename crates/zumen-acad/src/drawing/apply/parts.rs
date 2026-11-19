//! 寸法（DIMENSION・ARC_DIMENSION）と表（ACAD_TABLE）の置き換え・変換と、表示の匿名ブロック（`*D<n>`・`*T<n>`）の作り直し（#314）。

use std::collections::{BTreeMap, BTreeSet};

use super::add::{Born, Record, Target, int, text};
use super::{
    Charset, Current, Encoder, Frame, Home, InSpace, MODEL, Plan, carried, count_transform_values,
    entity_removal, moving_xdata, mtext, positive, real, split_text, work,
};
use crate::db::{AcadDb, Data, Object, Section, Value};
use zumen_base::{Cx, Ref};
use zumen_drawing::parts::{
    self, DimensionCurve, DimensionOffsets, DimensionParts, arrow_triangle, dimension_mtext,
    readable_angle, tick_line,
};
use zumen_drawing::transform as geom_transform;
use zumen_drawing::{
    Affine, Arrowhead, Axis, CellContent, Color, ColorCode, ColorSystem, Dimension, DimensionKind,
    Edge, Element, Flow, FormattedText, Height, HeightMeaning, Horizontal, Inherit, Label, Length,
    LineSpacing, ModelLength, ModelPoint, Rotation, Shape, Style, Symbol, SymbolKey, Table,
    Vertical,
};
use zumen_sdk::Error;

/// 匿名ブロック（`*D<n>`・`*T<n>`）の所有者と中身・参照・寸法スタイルを一度の走査で引く索引
pub(super) struct BlockIndex {
    /// 小文字のブロック名 → 一意な `BLOCK_RECORD` の `(位置, ハンドル)`。重複は `None`
    records: BTreeMap<String, Option<(usize, u64)>>,
    /// `BLOCK_RECORD` のハンドル → 原点 `(0, 0, 0)` の一意な `BLOCK` の位置。重複や外部参照は `None`
    blocks: BTreeMap<u64, Option<usize>>,
    /// `BLOCK_RECORD` のハンドル → 一意な `ENDBLK` の位置。重複は `None`
    endblks: BTreeMap<u64, Option<usize>>,
    /// `BLOCK_RECORD` のハンドル → `Section::Blocks` の中身の図形の位置
    members: BTreeMap<u64, Vec<usize>>,
    /// `Section::Blocks` に所有者を持たないオブジェクトがあるか
    unowned_blocks: bool,
    /// 小文字の匿名ブロック名（`*d...`・`*t...`）を `2` で指す図形・ブロックの位置
    name_refs: BTreeMap<String, Vec<usize>>,
    /// 匿名の `BLOCK_RECORD` のハンドルを指すオブジェクトの位置
    handle_refs: BTreeMap<u64, Vec<usize>>,
    /// ヘッダー変数が指す匿名の `BLOCK_RECORD` のハンドル
    header_handles: BTreeSet<u64>,
    /// 小文字の寸法スタイル名 → `DIMSTYLE` の位置
    dimstyles: BTreeMap<String, usize>,
    /// 小文字の表スタイル名かハンドルではなく、ハンドルから `TABLESTYLE` の位置は `db.get` で引ける
    /// 標準文字スタイルの高さ（`Standard` の `40`）
    standard_text_height: Option<f64>,
}

impl BlockIndex {
    pub(super) fn new(db: &AcadDb, charset: Charset, cx: &mut Cx) -> Result<Self, Error> {
        let place = Ref::default();
        let mut records: BTreeMap<String, Option<(usize, u64)>> = BTreeMap::new();
        let mut anon_handles = BTreeSet::new();
        let mut dimstyles = BTreeMap::new();
        let mut standard_text_height = None;

        for (pos, object) in db.objects().iter().enumerate() {
            work(cx, 1, &place)?;
            if object.section != Section::Tables {
                continue;
            }
            count_transform_values(object, &place, cx)?;
            match object.kind.as_slice() {
                b"BLOCK_RECORD" => {
                    let Some(Data::Text(raw)) = object.first(2) else {
                        continue;
                    };
                    let Some(name) = charset.decode(raw) else {
                        continue;
                    };
                    let lower = name.to_lowercase();
                    let Some(handle) = object.handle().filter(|&h| h != 0) else {
                        records.insert(lower, None);
                        continue;
                    };
                    if is_anon_parts_name(&lower) {
                        anon_handles.insert(handle);
                    }
                    records
                        .entry(lower)
                        .and_modify(|slot| *slot = None)
                        .or_insert(Some((pos, handle)));
                }
                b"DIMSTYLE" => {
                    if let Some(Data::Text(raw)) = object.first(2)
                        && let Some(name) = charset.decode(raw)
                    {
                        dimstyles.entry(name.to_lowercase()).or_insert(pos);
                    }
                }
                b"STYLE" => {
                    if let Some(Data::Text(raw)) = object.first(2)
                        && let Some(name) = charset.decode(raw)
                        && name.eq_ignore_ascii_case("Standard")
                        && let Some(Data::Real(h)) = object.first(40)
                        && h.is_finite()
                        && *h > 0.0
                    {
                        standard_text_height.get_or_insert(*h);
                    }
                }
                _ => {}
            }
        }

        let mut blocks: BTreeMap<u64, Option<usize>> = BTreeMap::new();
        let mut endblks: BTreeMap<u64, Option<usize>> = BTreeMap::new();
        let mut members: BTreeMap<u64, Vec<usize>> = BTreeMap::new();
        let mut unowned_blocks = false;
        let mut name_refs: BTreeMap<String, Vec<usize>> = BTreeMap::new();
        let mut handle_refs: BTreeMap<u64, Vec<usize>> = BTreeMap::new();

        for (pos, object) in db.objects().iter().enumerate() {
            work(cx, 1, &place)?;
            count_transform_values(object, &place, cx)?;
            if object.section == Section::Blocks {
                match object.owner() {
                    Some(owner) => match object.kind.as_slice() {
                        b"BLOCK" => {
                            let valid = block_is_origin_plain(object);
                            blocks
                                .entry(owner)
                                .and_modify(|slot| *slot = None)
                                .or_insert(valid.then_some(pos));
                        }
                        b"ENDBLK" => {
                            endblks
                                .entry(owner)
                                .and_modify(|slot| *slot = None)
                                .or_insert(Some(pos));
                        }
                        _ => members.entry(owner).or_default().push(pos),
                    },
                    None => unowned_blocks = true,
                }
            }
            let check_names = matches!(object.section, Section::Entities | Section::Blocks);
            for value in &object.values {
                match &value.data {
                    Data::Handle(h) if anon_handles.contains(h) => {
                        let list = handle_refs.entry(*h).or_default();
                        if list.last() != Some(&pos) {
                            list.push(pos);
                        }
                    }
                    Data::Text(raw) if check_names && value.code == 2 => {
                        if let Some(name) = charset.decode(raw) {
                            let lower = name.to_lowercase();
                            if is_anon_parts_name(&lower) {
                                let list = name_refs.entry(lower).or_default();
                                if list.last() != Some(&pos) {
                                    list.push(pos);
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }
        }

        let mut header_handles = BTreeSet::new();
        for (_, values) in db.variables() {
            work(cx, 1 + values.len() as u64, &place)?;
            for value in values {
                if let Data::Handle(h) = value.data
                    && anon_handles.contains(&h)
                {
                    header_handles.insert(h);
                }
            }
        }

        Ok(Self {
            records,
            blocks,
            endblks,
            members,
            unowned_blocks,
            name_refs,
            handle_refs,
            header_handles,
            dimstyles,
            standard_text_height,
        })
    }

    /// 図形 `entity_pos` だけが使う匿名ブロック（接頭辞 `prefix`）の `BLOCK_RECORD` ハンドルと中身の位置を返す
    fn locate(&self, entity_pos: usize, block_name: &str, prefix: &str) -> Option<(u64, &[usize])> {
        if self.unowned_blocks {
            return None;
        }
        let lower = block_name.to_lowercase();
        if !lower.starts_with(prefix) || lower.len() <= prefix.len() {
            return None;
        }
        let (record_pos, record_handle) = (*self.records.get(&lower)?)?;
        let block_pos = (*self.blocks.get(&record_handle)?)?;
        let endblk_pos = (*self.endblks.get(&record_handle)?)?;
        if self.header_handles.contains(&record_handle) {
            return None;
        }
        let member_slice = self
            .members
            .get(&record_handle)
            .map_or(&[][..], Vec::as_slice);
        let mut allowed = BTreeSet::from([entity_pos, record_pos, block_pos, endblk_pos]);
        allowed.extend(member_slice.iter().copied());
        if let Some(positions) = self.name_refs.get(&lower)
            && positions.iter().any(|pos| !allowed.contains(pos))
        {
            return None;
        }
        if let Some(positions) = self.handle_refs.get(&record_handle)
            && positions.iter().any(|pos| !allowed.contains(pos))
        {
            return None;
        }
        Some((record_handle, member_slice))
    }

    fn dimstyle<'a>(&self, db: &'a AcadDb, name: &str) -> Option<&'a Object> {
        let &pos = self.dimstyles.get(&name.to_lowercase())?;
        db.objects().get(pos)
    }
}

fn is_anon_parts_name(lower: &str) -> bool {
    lower.starts_with("*d") || lower.starts_with("*t")
}

/// `BLOCK` が基点 `(0, 0, 0)` で外部参照でないことを確かめる
fn block_is_origin_plain(block: &Object) -> bool {
    let zero = |code: i16| match block.first(code) {
        None => true,
        Some(Data::Real(v)) => *v == 0.0,
        _ => false,
    };
    let no_xref = match block.first(1) {
        None => true,
        Some(Data::Text(bytes)) => bytes.is_empty(),
        _ => false,
    };
    let flags_ok = match block.first(70) {
        None => true,
        Some(Data::Int(flags)) => *flags & (4 | 8 | 16 | 32) == 0,
        _ => false,
    };
    zero(10) && zero(20) && zero(30) && no_xref && flags_ok
}

/// 寸法または表の `Edit::Replace` を計画する
#[allow(clippy::too_many_arguments)]
pub(super) fn replace<P: Copy + PartialEq>(
    current: &Current<'_>,
    plan: &mut Plan<'_>,
    position: usize,
    object: &Object,
    was: &Element<P>,
    element: &Element<P>,
    frame: &impl Frame<P>,
    cx: &mut Cx,
) -> Result<Option<()>, Error> {
    if element.layer != was.layer || element.style != was.style {
        return Ok(None);
    }
    if element.shape == was.shape {
        plan.touched.insert(position);
        plan.replaced.push((position, object.values.clone()));
        return Ok(Some(()));
    }
    apply_parts(
        current,
        plan,
        position,
        object,
        was,
        &element.shape,
        frame,
        None,
        cx,
    )
}

/// 寸法または表の `Edit::Transform` を計画する
#[allow(clippy::too_many_arguments)]
pub(super) fn transform<P: Copy + PartialEq + geom_transform::Point>(
    current: &Current<'_>,
    plan: &mut Plan<'_>,
    position: usize,
    object: &Object,
    was: &Element<P>,
    affine: &Affine,
    frame: &impl Frame<P>,
    model: bool,
    cx: &mut Cx,
) -> Result<Option<()>, Error> {
    let Some(shape) = geom_transform::transform(&was.shape, affine) else {
        return Ok(None);
    };
    apply_parts(
        current,
        plan,
        position,
        object,
        was,
        &shape,
        frame,
        Some((affine, model)),
        cx,
    )
}

/// DXF の寸法または表を複写し、新しい匿名ブロック（`*D<n>`・`*T<n>`）のレコード一式を組み立てる（#889）。
/// 返り値は `(書き直した図形の値, [BLOCK_RECORD, BLOCK, ..部品, ENDBLK], 確保した匿名ブロック名（小文字）)`
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub(super) fn copy_parts<P: Copy + PartialEq + geom_transform::Point>(
    current: &Current<'_>,
    plan: &mut Plan<'_>,
    object: &Object,
    was: &Element<P>,
    affine: &Affine,
    frame: &impl Frame<P>,
    model: bool,
    cx: &mut Cx,
) -> Result<Option<(Vec<Value>, Vec<Record>, String)>, Error> {
    if current.target != Target::Dxf {
        return Ok(None);
    }
    let Some(new_shape) = geom_transform::transform(&was.shape, affine) else {
        return Ok(None);
    };
    if plan.blocks.is_none() {
        plan.blocks = Some(BlockIndex::new(current.db, current.charset, cx)?);
    }
    let Some(blocks) = plan.blocks.as_ref() else {
        return Ok(None);
    };
    let prefix = match (&was.shape, &new_shape) {
        (Shape::Dimension(_), Shape::Dimension(_)) => "*D",
        (Shape::Table(_), Shape::Table(_)) => "*T",
        _ => return Ok(None),
    };
    let (Some(next), Some(model_rec)) = (
        current.born.next_handle(),
        super::add::symbol::model_record(current.db),
    ) else {
        return Ok(None);
    };
    let Some(table_owner) = model_rec.owner() else {
        return Ok(None);
    };
    if super::layer::table_end(current.db, b"BLOCK_RECORD").is_none()
        || super::add::symbol::blocks_end(current.db).is_none()
    {
        return Ok(None);
    }
    let Some(zero) = current.born.named(8, b"LAYER", "0") else {
        return Ok(None);
    };

    let mut number = 1u64;
    let (anon_name, lower) = loop {
        let candidate = format!("{prefix}{number}");
        let lower = candidate.to_lowercase();
        if !blocks.records.contains_key(&lower) && !plan.anon_names.contains(&lower) {
            break (candidate, lower);
        }
        let Some(next_num) = number.checked_add(1) else {
            return Ok(None);
        };
        number = next_num;
    };
    let (Some(name2), Some(name3)) = (
        current.born.text_value(2, &anon_name),
        current.born.text_value(3, &anon_name),
    ) else {
        return Ok(None);
    };

    // 足す順番: [0] 写した寸法・表, [1] BLOCK_RECORD, [2] BLOCK, [3..] 部品, [末尾] ENDBLK
    let Some(anon_record_handle) = next
        .checked_add(plan.added_records)
        .and_then(|base| base.checked_add(1))
    else {
        return Ok(None);
    };
    let Some(parts_base) = plan.added_records.checked_add(3) else {
        return Ok(None);
    };

    let built = match (&was.shape, &new_shape) {
        (Shape::Dimension(old_dim), Shape::Dimension(new_dim)) => plan_dimension(
            current,
            blocks,
            object,
            old_dim,
            new_dim,
            anon_record_handle,
            frame,
            Some(affine),
            parts_base,
            true,
            cx,
            &was.source,
        )?,
        (Shape::Table(old_table), Shape::Table(new_table)) => plan_table(
            current,
            blocks,
            object,
            old_table,
            new_table,
            anon_record_handle,
            frame,
            parts_base,
            true,
            cx,
            &was.source,
        )?,
        _ => None,
    };
    let Some((mut values, part_records, part_count)) = built else {
        return Ok(None);
    };

    if moving_xdata(object).is_some()
        && (!model || carried(&mut values, &frame.affine(affine)).is_none())
    {
        return Ok(None);
    }

    // 写しの匿名ブロック参照を新しい名前と BLOCK_RECORD ハンドルへ付け替える
    if matches!(new_shape, Shape::Dimension(_)) {
        let xdata_at = values
            .iter()
            .position(|v| v.code == 1001)
            .unwrap_or(values.len());
        if let Some(slot) = values
            .get_mut(..xdata_at)
            .and_then(|s| s.iter_mut().find(|v| v.code == 2))
        {
            *slot = name2.clone();
        } else if insert_in_dim_common(&mut values, name2.clone(), &[100]).is_none() {
            return Ok(None);
        }
    } else if matches!(new_shape, Shape::Table(_)) {
        let Some(marker) = values.iter().position(|v| {
            v.code == 100 && matches!(&v.data, Data::Text(t) if t.as_slice() == b"AcDbTable")
        }) else {
            return Ok(None);
        };
        if let Some(slot) = values
            .get_mut(..marker)
            .and_then(|s| s.iter_mut().find(|v| v.code == 2))
        {
            *slot = name2.clone();
        } else {
            let at = values
                .get(..marker)
                .and_then(|s| s.iter().rposition(|v| v.code == 100))
                .map_or(marker, |i| i + 1);
            values.insert(at, name2.clone());
        }
        let marker = values
            .iter()
            .position(|v| {
                v.code == 100 && matches!(&v.data, Data::Text(t) if t.as_slice() == b"AcDbTable")
            })
            .unwrap_or(marker);
        let cell_at = values
            .get(marker..)
            .and_then(|s| s.iter().position(|v| v.code == 171))
            .map_or(values.len(), |rel| marker + rel);
        if let Some(slice) = values.get_mut(marker..cell_at) {
            for item in slice {
                if item.code == 343 && matches!(item.data, Data::Handle(_)) {
                    item.data = Data::Handle(anon_record_handle);
                }
            }
        }
    }

    let mut rec_values = Vec::new();
    for v in &model_rec.values {
        match v.code {
            2 => rec_values.push(name2.clone()),
            100 | 70 | 280 | 281 => rec_values.push(v.clone()),
            _ => {}
        }
    }
    if !rec_values.iter().any(|v| v.code == 2) {
        rec_values.push(name2.clone());
    }
    let block_record = Record {
        kind: b"BLOCK_RECORD",
        values: rec_values,
        home: Home {
            owner: Some(table_owner),
            blocks: false,
            paper: false,
        },
        followers: Vec::new(),
        annotation: None,
    };
    let block_home = Home {
        owner: Some(anon_record_handle),
        blocks: true,
        paper: false,
    };
    let markers = current.born.has_markers();
    let mut block_begin = Vec::new();
    if markers {
        block_begin.extend([
            text(100, "AcDbEntity"),
            zero.clone(),
            text(100, "AcDbBlockBegin"),
        ]);
    } else {
        block_begin.push(zero.clone());
    }
    block_begin.push(name2);
    block_begin.push(int(70, 1));
    let (Some(x0), Some(y0), Some(z0)) = (real(10, 0.0), real(20, 0.0), real(30, 0.0)) else {
        return Ok(None);
    };
    block_begin.extend([x0, y0, z0, name3, text(1, "")]);
    let mut end_values = Vec::new();
    if markers {
        end_values.extend([text(100, "AcDbEntity"), zero, text(100, "AcDbBlockEnd")]);
    } else {
        end_values.push(zero);
    }

    let Some(total_extra) = part_count.checked_add(3) else {
        return Ok(None);
    };
    if next
        .checked_add(plan.added_records)
        .and_then(|first| first.checked_add(1))
        .and_then(|after_ent| after_ent.checked_add(total_extra))
        .and_then(|last| last.checked_add(1))
        .is_none()
    {
        return Ok(None);
    }

    let mut extra = Vec::with_capacity(part_records.len() + 3);
    extra.push(block_record);
    extra.push(Record {
        kind: b"BLOCK",
        values: block_begin,
        home: block_home,
        followers: Vec::new(),
        annotation: None,
    });
    extra.extend(part_records);
    extra.push(Record {
        kind: b"ENDBLK",
        values: end_values,
        home: block_home,
        followers: Vec::new(),
        annotation: None,
    });
    Ok(Some((values, extra, lower)))
}

/// 寸法（DIMENSION・ARC_DIMENSION）と表（ACAD_TABLE）を新しく足す（`Edit::Add`）。
/// 主レコードと、新しい匿名ブロック（`*D<n>`・`*T<n>`）の `BLOCK_RECORD`・`BLOCK`・部品・`ENDBLK` を返す。
pub(super) fn add_parts<P: Copy>(
    current: &Current<'_>,
    plan: &mut Plan<'_>,
    element: &Element<P>,
    frame: &impl Frame<P>,
    home: Home,
    cx: &mut Cx,
) -> Result<Option<(Record, Vec<Record>, String)>, Error> {
    if home.owner.is_none() || !current.born.home_ready(home) {
        return Ok(None);
    }
    let Some(next) = current.born.next_handle() else {
        return Ok(None);
    };
    let dxf = current.target == Target::Dxf;
    if !dxf && !current.born.since(b"AC1015") {
        return Ok(None);
    }
    if plan.blocks.is_none() {
        plan.blocks = Some(BlockIndex::new(current.db, current.charset, cx)?);
    }
    let Some(blocks) = plan.blocks.as_ref() else {
        return Ok(None);
    };
    let Some(model_rec) = super::add::symbol::model_record(current.db) else {
        return Ok(None);
    };
    let Some(table_owner) = model_rec.owner() else {
        return Ok(None);
    };
    if dxf
        && (super::layer::table_end(current.db, b"BLOCK_RECORD").is_none()
            || super::add::symbol::blocks_end(current.db).is_none())
    {
        return Ok(None);
    }
    let Some(zero) = current.born.named(8, b"LAYER", "0") else {
        return Ok(None);
    };
    let paper = home.paper && !(home.blocks && !dxf);
    let Some(entity_head) = current.born.entity(element.layer, &element.style, paper) else {
        return Ok(None);
    };
    let prefix = match &element.shape {
        Shape::Dimension(_) => "*D",
        Shape::Table(_) => "*T",
        _ => return Ok(None),
    };
    let mut number = 1u64;
    let (anon_name, lower) = loop {
        let candidate = format!("{prefix}{number}");
        let lower = candidate.to_lowercase();
        if !blocks.records.contains_key(&lower) && !plan.anon_names.contains(&lower) {
            break (candidate, lower);
        }
        let Some(next_num) = number.checked_add(1) else {
            return Ok(None);
        };
        number = next_num;
    };
    let (Some(name2), Some(name3)) = (
        current.born.text_value(2, &anon_name),
        current.born.text_value(3, &anon_name),
    ) else {
        return Ok(None);
    };

    // 足す順番: [0] 寸法・表, [1] BLOCK_RECORD, [2] BLOCK, [3..] 部品, [末尾] ENDBLK
    let Some(anon_record_handle) = next
        .checked_add(plan.added_records)
        .and_then(|base| base.checked_add(1))
    else {
        return Ok(None);
    };
    let Some(parts_base) = plan.added_records.checked_add(3) else {
        return Ok(None);
    };
    let place_ref = Ref::default();
    let markers = dxf && current.born.has_markers();

    let (main_kind, mut main_values, part_records, part_count): (
        &'static [u8],
        Vec<Value>,
        Vec<Record>,
        u64,
    ) = match &element.shape {
        Shape::Dimension(dimension) => {
            let Some(built) = add_dimension_parts(
                current,
                blocks,
                dimension,
                &entity_head,
                &name2,
                anon_record_handle,
                frame,
                parts_base,
                dxf,
                cx,
                &place_ref,
            )?
            else {
                return Ok(None);
            };
            built
        }
        Shape::Table(table) => {
            let Some(built) = add_table_parts(
                current,
                blocks,
                table,
                &entity_head,
                &name2,
                anon_record_handle,
                frame,
                parts_base,
                dxf,
                cx,
                &place_ref,
            )?
            else {
                return Ok(None);
            };
            built
        }
        _ => return Ok(None),
    };
    if !markers {
        main_values.retain(|v| v.code != 100);
    }

    let mut rec_values = Vec::new();
    if dxf {
        for v in &model_rec.values {
            match v.code {
                2 => rec_values.push(name2.clone()),
                100 | 70 | 280 | 281 => rec_values.push(v.clone()),
                _ => {}
            }
        }
        if !rec_values.iter().any(|v| v.code == 2) {
            rec_values.push(name2.clone());
        }
    } else {
        rec_values.push(name2.clone());
    }
    let block_record = Record {
        kind: b"BLOCK_RECORD",
        values: rec_values,
        home: Home {
            owner: Some(table_owner),
            blocks: false,
            paper: false,
        },
        followers: Vec::new(),
        annotation: None,
    };
    let block_home = Home {
        owner: Some(anon_record_handle),
        blocks: true,
        paper: false,
    };
    let (Some(x0), Some(y0), Some(z0)) = (real(10, 0.0), real(20, 0.0), real(30, 0.0)) else {
        return Ok(None);
    };
    let mut block_begin = Vec::new();
    if markers {
        block_begin.extend([
            text(100, "AcDbEntity"),
            zero.clone(),
            text(100, "AcDbBlockBegin"),
        ]);
    } else {
        block_begin.push(zero.clone());
    }
    block_begin.push(name2);
    if dxf {
        block_begin.push(int(70, 1));
    }
    block_begin.extend([x0, y0, z0]);
    if dxf {
        block_begin.extend([name3, text(1, "")]);
    }
    let mut end_values = Vec::new();
    if markers {
        end_values.extend([text(100, "AcDbEntity"), zero, text(100, "AcDbBlockEnd")]);
    } else {
        end_values.push(zero);
    }

    let Some(total_extra) = part_count.checked_add(3) else {
        return Ok(None);
    };
    if next
        .checked_add(plan.added_records)
        .and_then(|first| first.checked_add(1))
        .and_then(|after_ent| after_ent.checked_add(total_extra))
        .and_then(|last| last.checked_add(1))
        .is_none()
    {
        return Ok(None);
    }

    let main_record = Record {
        kind: main_kind,
        values: main_values,
        home,
        followers: Vec::new(),
        annotation: None,
    };
    let mut extra = Vec::with_capacity(part_records.len() + 3);
    extra.push(block_record);
    extra.push(Record {
        kind: b"BLOCK",
        values: block_begin,
        home: block_home,
        followers: Vec::new(),
        annotation: None,
    });
    extra.extend(part_records);
    extra.push(Record {
        kind: b"ENDBLK",
        values: end_values,
        home: block_home,
        followers: Vec::new(),
        annotation: None,
    });
    Ok(Some((main_record, extra, lower)))
}

type BornPart = (&'static [u8], Vec<Value>, Vec<Record>, u64);

#[allow(clippy::too_many_arguments)]
fn add_dimension_parts<P: Copy>(
    current: &Current<'_>,
    blocks: &BlockIndex,
    dimension: &Dimension<P>,
    entity_head: &[Value],
    name2: &Value,
    anon_record_handle: u64,
    frame: &impl Frame<P>,
    parts_base: u64,
    dxf: bool,
    cx: &mut Cx,
    place: &Ref,
) -> Result<Option<BornPart>, Error> {
    if dimension.block.is_some() || dimension.measurement.is_some() {
        return Ok(None);
    }
    let is_arc = matches!(dimension.kind, DimensionKind::ArcLength { .. });
    if is_arc {
        if !current.born.since(b"AC1018") || !current.arc_dimension_class {
            return Ok(None);
        }
    } else if dxf && !current.born.since(b"AC1012") && !current.born.has_markers() {
        return Ok(None);
    }
    let Some(style3) = current.born.named(3, b"DIMSTYLE", &dimension.style) else {
        return Ok(None);
    };
    let dummy = Object {
        kind: b"DIMENSION".to_vec(),
        section: Section::Entities,
        values: Vec::new(),
        origin: crate::Origin::New,
    };
    let params = DimParams::resolve(current, blocks, &dummy, &dimension.style);
    let Some(drawn) = parts::dimension(
        dimension,
        |p| frame.point(p),
        |l| frame.length(l),
        params.offsets,
    ) else {
        return Ok(None);
    };
    let text_pos = if dimension.text.moved {
        let Some(pos) = &dimension.text.position else {
            return Ok(None);
        };
        let Some(pt) = frame.point(pos) else {
            return Ok(None);
        };
        pt
    } else if let Some(pos) = &dimension.text.position {
        let Some(pt) = frame.point(pos) else {
            return Ok(None);
        };
        pt
    } else {
        drawn.middle
    };
    let Some(label_val) = encode_dimension_label(&dimension.label, current.encoder) else {
        return Ok(None);
    };
    let att = dimension
        .text
        .attachment
        .map_or(5, |(h, v)| attachment_code(h, v));
    let push_pt3 = |out: &mut Vec<Value>, code: i16, [x, y]: [f64; 2]| -> Option<()> {
        out.extend([real(code, x)?, real(code + 10, y)?, real(code + 20, 0.0)?]);
        Some(())
    };

    let mut values = entity_head.to_vec();
    values.push(text(100, "AcDbDimension"));
    values.push(name2.clone());

    let main_kind: &'static [u8] = match &dimension.kind {
        DimensionKind::ArcLength {
            center,
            first,
            second,
            arc,
        } => {
            let (Some(p_center), Some(p_first), Some(p_second), Some(p_arc)) = (
                frame.point(center),
                frame.point(first),
                frame.point(second),
                frame.point(arc),
            ) else {
                return Ok(None);
            };
            let Some(DimensionCurve::Arc { start, sweep, .. }) = drawn.curve else {
                return Ok(None);
            };
            let start_rad = start.rem_euclid(360.0).to_radians();
            let end_rad = if sweep >= 360.0 && start_rad == 0.0 {
                core::f64::consts::TAU
            } else {
                (start + sweep).rem_euclid(360.0).to_radians()
            };
            let flags = 32 | 5 | if dimension.text.moved { 128 } else { 0 };
            let (Some(m42), Some(r40), Some(r41)) = (
                real(42, drawn.raw_measurement),
                real(40, start_rad),
                real(41, end_rad),
            ) else {
                return Ok(None);
            };
            if push_pt3(&mut values, 10, p_arc).is_none()
                || push_pt3(&mut values, 11, text_pos).is_none()
            {
                return Ok(None);
            }
            values.extend([int(70, flags), int(71, att), m42]);
            if let Some(val) = label_val {
                values.push(val);
            }
            if let Some(rot) = dimension.text.rotation {
                let Some(r53) = real(53, rot) else {
                    return Ok(None);
                };
                if dxf || rot != 0.0 {
                    values.push(r53);
                }
            }
            values.push(style3);
            values.push(text(100, "AcDbArcDimension"));
            if push_pt3(&mut values, 13, p_first).is_none()
                || push_pt3(&mut values, 14, p_second).is_none()
                || push_pt3(&mut values, 15, p_center).is_none()
            {
                return Ok(None);
            }
            values.extend([int(70, 0), r40, r41, int(71, 0)]);
            if push_pt3(&mut values, 16, p_first).is_none()
                || push_pt3(&mut values, 17, p_second).is_none()
            {
                return Ok(None);
            }
            b"ARC_DIMENSION"
        }
        kind => {
            let (kind_code, p10) = match kind {
                DimensionKind::Linear { line, .. } => (0, frame.point(line)),
                DimensionKind::Aligned { line, .. } => (1, frame.point(line)),
                DimensionKind::AngularLines { second, .. } => (2, frame.point(&second[1])),
                DimensionKind::Diameter { opposite, .. } => (3, frame.point(opposite)),
                DimensionKind::Radius { center, .. } => (4, frame.point(center)),
                DimensionKind::Angular { arc, .. } => (5, frame.point(arc)),
                DimensionKind::Ordinate { origin, axis, .. } => (
                    6 | if *axis == Axis::X { 64 } else { 0 },
                    frame.point(origin),
                ),
                DimensionKind::ArcLength { .. } => return Ok(None),
            };
            let Some(p10) = p10 else {
                return Ok(None);
            };
            let Some(m42) = real(42, drawn.raw_measurement) else {
                return Ok(None);
            };
            let flags = 32 | kind_code | if dimension.text.moved { 128 } else { 0 };
            if push_pt3(&mut values, 10, p10).is_none()
                || push_pt3(&mut values, 11, text_pos).is_none()
            {
                return Ok(None);
            }
            values.extend([int(70, flags), int(71, att), m42]);
            if let Some(val) = label_val {
                values.push(val);
            }
            if let Some(rot) = dimension.text.rotation {
                let Some(r53) = real(53, rot) else {
                    return Ok(None);
                };
                if dxf || rot != 0.0 {
                    values.push(r53);
                }
            }
            values.push(style3);
            match kind {
                DimensionKind::Linear {
                    first,
                    second,
                    rotation,
                    oblique,
                    ..
                } => {
                    let (Some(p13), Some(p14)) = (frame.point(first), frame.point(second)) else {
                        return Ok(None);
                    };
                    if !rotation.is_finite() || !oblique.is_finite() {
                        return Ok(None);
                    }
                    values.push(text(100, "AcDbAlignedDimension"));
                    if push_pt3(&mut values, 13, p13).is_none()
                        || push_pt3(&mut values, 14, p14).is_none()
                    {
                        return Ok(None);
                    }
                    if *rotation != 0.0 {
                        let Some(r50) = real(50, *rotation) else {
                            return Ok(None);
                        };
                        values.push(r50);
                    }
                    if *oblique != 0.0 {
                        let Some(r52) = real(52, *oblique) else {
                            return Ok(None);
                        };
                        values.push(r52);
                    }
                    values.push(text(100, "AcDbRotatedDimension"));
                }
                DimensionKind::Aligned {
                    first,
                    second,
                    oblique,
                    ..
                } => {
                    let (Some(p13), Some(p14)) = (frame.point(first), frame.point(second)) else {
                        return Ok(None);
                    };
                    if !oblique.is_finite() {
                        return Ok(None);
                    }
                    values.push(text(100, "AcDbAlignedDimension"));
                    if push_pt3(&mut values, 13, p13).is_none()
                        || push_pt3(&mut values, 14, p14).is_none()
                    {
                        return Ok(None);
                    }
                    if *oblique != 0.0 {
                        let Some(r52) = real(52, *oblique) else {
                            return Ok(None);
                        };
                        values.push(r52);
                    }
                }
                DimensionKind::Radius { chord, leader, .. } => {
                    let (Some(p15), Some(lead)) =
                        (frame.point(chord), super::nonnegative(frame, leader))
                    else {
                        return Ok(None);
                    };
                    let Some(r40) = real(40, lead) else {
                        return Ok(None);
                    };
                    values.push(text(100, "AcDbRadialDimension"));
                    if push_pt3(&mut values, 15, p15).is_none() {
                        return Ok(None);
                    }
                    values.push(r40);
                }
                DimensionKind::Diameter { chord, leader, .. } => {
                    let (Some(p15), Some(lead)) =
                        (frame.point(chord), super::nonnegative(frame, leader))
                    else {
                        return Ok(None);
                    };
                    let Some(r40) = real(40, lead) else {
                        return Ok(None);
                    };
                    values.push(text(100, "AcDbDiametricDimension"));
                    if push_pt3(&mut values, 15, p15).is_none() {
                        return Ok(None);
                    }
                    values.push(r40);
                }
                DimensionKind::Angular {
                    vertex,
                    first,
                    second,
                    ..
                } => {
                    let (Some(p13), Some(p14), Some(p15)) =
                        (frame.point(first), frame.point(second), frame.point(vertex))
                    else {
                        return Ok(None);
                    };
                    values.push(text(100, "AcDb3PointAngularDimension"));
                    if push_pt3(&mut values, 13, p13).is_none()
                        || push_pt3(&mut values, 14, p14).is_none()
                        || push_pt3(&mut values, 15, p15).is_none()
                    {
                        return Ok(None);
                    }
                }
                DimensionKind::AngularLines { first, second, arc } => {
                    let (Some(p13), Some(p14), Some(p15), Some(p16)) = (
                        frame.point(&first[0]),
                        frame.point(&first[1]),
                        frame.point(&second[0]),
                        frame.point(arc),
                    ) else {
                        return Ok(None);
                    };
                    values.push(text(100, "AcDb2LineAngularDimension"));
                    if push_pt3(&mut values, 13, p13).is_none()
                        || push_pt3(&mut values, 14, p14).is_none()
                        || push_pt3(&mut values, 15, p15).is_none()
                        || push_pt3(&mut values, 16, p16).is_none()
                    {
                        return Ok(None);
                    }
                }
                DimensionKind::Ordinate {
                    feature,
                    leader_end,
                    ..
                } => {
                    let (Some(p13), Some(p14)) = (frame.point(feature), frame.point(leader_end))
                    else {
                        return Ok(None);
                    };
                    values.push(text(100, "AcDbOrdinateDimension"));
                    if push_pt3(&mut values, 13, p13).is_none()
                        || push_pt3(&mut values, 14, p14).is_none()
                    {
                        return Ok(None);
                    }
                }
                DimensionKind::ArcLength { .. } => return Ok(None),
            }
            b"DIMENSION"
        }
    };

    let Some((records, count)) = build_dimension_records(
        current.born,
        dimension,
        &drawn,
        text_pos,
        &params,
        anon_record_handle,
        parts_base,
        true,
        cx,
        place,
    )?
    else {
        return Ok(None);
    };
    Ok(Some((main_kind, values, records, count)))
}

fn find_standard_tablestyle<'a>(
    db: &'a AcadDb,
    charset: Charset,
    cx: &mut Cx,
    place: &Ref,
) -> Result<Option<(u64, &'a Object)>, Error> {
    let entry_of = |dict: &Object, key: &str| -> Option<u64> {
        dict.values.windows(2).find_map(|pair| match pair {
            [
                Value {
                    code: 3,
                    data: Data::Text(raw),
                },
                Value {
                    code: 350 | 360,
                    data: Data::Handle(h),
                },
            ] if *h != 0
                && charset
                    .decode(raw)
                    .is_some_and(|s| s.eq_ignore_ascii_case(key)) =>
            {
                Some(*h)
            }
            _ => None,
        })
    };
    let mut dict_style = None;
    let mut named_styles = Vec::new();
    for obj in db.objects() {
        work(cx, 1, place)?;
        if obj.section != Section::Objects {
            continue;
        }
        if obj.kind == b"DICTIONARY"
            && let Some(group_handle) = entry_of(obj, "ACAD_TABLESTYLE")
            && let Some(group_dict) = db.get(group_handle).filter(|d| d.kind == b"DICTIONARY")
            && let Some(style_handle) = entry_of(group_dict, "Standard")
            && let Some(style_obj) = db.get(style_handle).filter(|s| s.kind == b"TABLESTYLE")
        {
            dict_style = Some((style_handle, style_obj));
            break;
        }
        if obj.kind == b"TABLESTYLE"
            && let Some(handle) = obj.handle().filter(|&h| h != 0)
            && let Some(Data::Text(raw)) = obj.first(3)
            && charset
                .decode(raw)
                .is_some_and(|s| s.eq_ignore_ascii_case("Standard"))
        {
            named_styles.push((handle, obj));
        }
    }
    if let Some(found) = dict_style {
        return Ok(Some(found));
    }
    Ok((named_styles.len() == 1).then(|| named_styles.remove(0)))
}

#[allow(clippy::too_many_arguments)]
fn add_table_parts<P: Copy>(
    current: &Current<'_>,
    blocks: &BlockIndex,
    table: &Table<P>,
    entity_head: &[Value],
    name2: &Value,
    anon_record_handle: u64,
    frame: &impl Frame<P>,
    parts_base: u64,
    dxf: bool,
    cx: &mut Cx,
    place: &Ref,
) -> Result<Option<BornPart>, Error> {
    if !dxf
        || !current.born.since(b"AC1018")
        || table.style.is_some()
        || table.formats.is_some()
        || table.drawn.is_some()
    {
        return Ok(None);
    }
    let Some((style_handle, style_obj)) =
        find_standard_tablestyle(current.db, current.charset, cx, place)?
    else {
        return Ok(None);
    };
    let rows = table.row_heights.len();
    let cols = table.column_widths.len();
    if rows == 0 || cols == 0 || rows.checked_mul(cols) != Some(table.cells.len()) {
        return Ok(None);
    }
    let Some([px, py]) = frame.point(&table.position) else {
        return Ok(None);
    };
    let [dx, dy] = table.direction;
    let norm = libm::hypot(dx, dy);
    if !(norm > 0.0 && norm.is_finite()) {
        return Ok(None);
    }
    let (Some(rows_i), Some(cols_i)) = (i64::try_from(rows).ok(), i64::try_from(cols).ok()) else {
        return Ok(None);
    };

    let mut values = entity_head.to_vec();
    let (Some(p10), Some(p20), Some(p30), Some(d11), Some(d21), Some(d31)) = (
        real(10, px),
        real(20, py),
        real(30, 0.0),
        real(11, dx),
        real(21, dy),
        real(31, 0.0),
    ) else {
        return Ok(None);
    };
    values.extend([
        text(100, "AcDbBlockReference"),
        name2.clone(),
        p10,
        p20,
        p30,
        text(100, "AcDbTable"),
        int(280, 0),
        Value {
            code: 342,
            data: Data::Handle(style_handle),
        },
        Value {
            code: 343,
            data: Data::Handle(anon_record_handle),
        },
        d11,
        d21,
        d31,
        int(90, 22),
        int(91, rows_i),
        int(92, cols_i),
        int(93, 0),
        int(94, 0),
        int(95, 0),
        int(96, 0),
    ]);
    for h in &table.row_heights {
        let Some(raw_h) = positive(frame, h).and_then(|v| real(141, v)) else {
            return Ok(None);
        };
        values.push(raw_h);
    }
    for w in &table.column_widths {
        let Some(raw_w) = positive(frame, w).and_then(|v| real(142, v)) else {
            return Ok(None);
        };
        values.push(raw_w);
    }

    let r2007 = current.born.since(b"AC1021");
    let flag_code = if r2007 { 91 } else { 177 };
    for cell in &table.cells {
        work(cx, 1, place)?;
        if cell.value.is_some()
            || cell.format != Default::default()
            || !cell.rotation.is_finite()
            || cell.span[0] == 0
            || cell.span[1] == 0
        {
            return Ok(None);
        }
        let Some(rot_val) = real(145, cell.rotation) else {
            return Ok(None);
        };
        match &cell.content {
            CellContent::Empty | CellContent::Text { .. } => {
                let raw_str = match &cell.content {
                    CellContent::Empty => "",
                    CellContent::Text { content, .. } => {
                        if content.is_empty() {
                            return Ok(None);
                        }
                        content.as_str()
                    }
                    CellContent::Symbol(_) => "",
                };
                let Some(text_chunks) = split_text(raw_str, 2, 1, current.encoder) else {
                    return Ok(None);
                };
                values.extend([
                    int(171, 1),
                    int(172, 0),
                    int(173, i64::from(cell.covered)),
                    int(174, 0),
                    int(175, i64::from(cell.span[0])),
                    int(176, i64::from(cell.span[1])),
                    int(flag_code, 0),
                    int(178, 0),
                    rot_val,
                ]);
                values.extend(text_chunks.clone());
                if r2007 {
                    values.push(text(301, "CELL_VALUE"));
                    if raw_str.is_empty() {
                        values.extend([
                            int(90, 0),
                            int(93, 1),
                            text(300, ""),
                            text(302, ""),
                            text(304, "ACVALUE_END"),
                        ]);
                    } else {
                        let Some(fmt_chunks) = split_text(raw_str, 303, 302, current.encoder)
                        else {
                            return Ok(None);
                        };
                        values.extend([int(90, 4), int(93, 0)]);
                        values.extend(text_chunks);
                        values.push(int(94, 0));
                        values.push(text(300, ""));
                        values.extend(fmt_chunks);
                        values.push(text(304, "ACVALUE_END"));
                    }
                }
            }
            CellContent::Symbol(key) => {
                if key.namespace != super::BLOCK
                    || key.name.starts_with('*')
                    || !current.born.has_block(&key.name)
                {
                    return Ok(None);
                }
                let Some((_, Some((_, blk_handle)))) =
                    blocks.records.get_key_value(&key.name.to_lowercase())
                else {
                    return Ok(None);
                };
                let Some(s144) = real(144, 1.0) else {
                    return Ok(None);
                };
                values.extend([
                    int(171, 2),
                    int(172, 0),
                    int(173, i64::from(cell.covered)),
                    int(174, 0),
                    int(175, i64::from(cell.span[0])),
                    int(176, i64::from(cell.span[1])),
                    int(flag_code, 0),
                    int(178, 0),
                    rot_val,
                    s144,
                    Value {
                        code: 340,
                        data: Data::Handle(*blk_handle),
                    },
                ]);
            }
        }
    }

    let mut styled_table = table.clone();
    styled_table.formats = crate::drawing::acad_table::lift_table_style_formats(
        current.db,
        style_obj,
        current.charset,
        &|v| frame.lifted(v),
    )
    .map(Box::new);
    let default_height = blocks.standard_text_height.unwrap_or(2.5);
    let Some((records, count)) = build_table_records(
        current.born,
        &styled_table,
        frame,
        default_height,
        anon_record_handle,
        parts_base,
        true,
        cx,
        place,
    )?
    else {
        return Ok(None);
    };
    Ok(Some((b"ACAD_TABLE", values, records, count)))
}

#[allow(clippy::too_many_arguments)]
fn apply_parts<P: Copy + PartialEq>(
    current: &Current<'_>,
    plan: &mut Plan<'_>,
    position: usize,
    object: &Object,
    was: &Element<P>,
    new_shape: &Shape<P>,
    frame: &impl Frame<P>,
    transformed: Option<(&Affine, bool)>,
    cx: &mut Cx,
) -> Result<Option<()>, Error> {
    // DWG の表の移動は、書式を読めないセルも表示用ブロックもそのまま保つ。
    // セル・向き・大きさが変わるときは、未対応の表書式を持たないものだけ部品を作り直す。
    if matches!(current.target, Target::Dwg(_))
        && let (Shape::Table(old), Shape::Table(new)) = (&was.shape, new_shape)
    {
        if !current.born.since(b"AC1015") {
            return Ok(None);
        }
        count_transform_values(object, &was.source, cx)?;
        if let Some(mut values) = moved_table_values(object, old, new, frame) {
            if let Some((affine, model)) = transformed
                && moving_xdata(object).is_some()
                && (!model || carried(&mut values, &frame.affine(affine)).is_none())
            {
                return Ok(None);
            }
            plan.touched.insert(position);
            plan.replaced.push((position, values));
            return Ok(Some(()));
        }
        if crate::drawing::acad_table::has_unsupported_dwg_table_format(current.db, object) {
            return Ok(None);
        }
    }
    if plan.blocks.is_none() {
        plan.blocks = Some(BlockIndex::new(current.db, current.charset, cx)?);
    }
    let Some(blocks) = plan.blocks.as_ref() else {
        return Ok(None);
    };

    let (block_name, prefix) = match (&was.shape, new_shape) {
        (Shape::Dimension(old_dim), Shape::Dimension(_)) => {
            let Some(name) = old_dim.block.as_deref() else {
                return Ok(None);
            };
            (name, "*d")
        }
        (Shape::Table(old_table), Shape::Table(_)) => {
            let Some(key) = old_table.drawn.as_ref() else {
                return Ok(None);
            };
            if key.namespace != super::BLOCK {
                return Ok(None);
            }
            (key.name.as_str(), "*t")
        }
        _ => return Ok(None),
    };

    let Some((record_handle, members)) = blocks.locate(position, block_name, prefix) else {
        return Ok(None);
    };
    if plan.rebuilt_blocks.contains(&record_handle) {
        return Ok(None);
    }

    let mut removals = BTreeSet::new();
    for &member_pos in members {
        work(cx, 1, &was.source)?;
        if plan.touched.contains(&member_pos) {
            return Ok(None);
        }
        let Some(member_obj) = current.db.objects().get(member_pos) else {
            return Ok(None);
        };
        let Some(skip) = entity_removal(current.db, member_pos, member_obj, cx, &was.source)?
        else {
            return Ok(None);
        };
        if skip.iter().any(|pos| plan.touched.contains(pos)) {
            return Ok(None);
        }
        removals.extend(skip);
    }

    let built = match (&was.shape, new_shape) {
        (Shape::Dimension(old_dim), Shape::Dimension(new_dim)) => plan_dimension(
            current,
            blocks,
            object,
            old_dim,
            new_dim,
            record_handle,
            frame,
            transformed.map(|(affine, _)| affine),
            plan.added_records,
            false,
            cx,
            &was.source,
        )?,
        (Shape::Table(old_table), Shape::Table(new_table)) => plan_table(
            current,
            blocks,
            object,
            old_table,
            new_table,
            record_handle,
            frame,
            plan.added_records,
            false,
            cx,
            &was.source,
        )?,
        _ => None,
    };
    let Some((mut values, records, added_count)) = built else {
        return Ok(None);
    };

    if let Some((affine, model)) = transformed
        && moving_xdata(object).is_some()
    {
        if !model {
            return Ok(None);
        }
        if carried(&mut values, &frame.affine(affine)).is_none() {
            return Ok(None);
        }
    }

    plan.rebuilt_blocks.insert(record_handle);
    plan.touched.insert(position);
    plan.rebuilt_removed.extend(removals.iter().copied());
    plan.replaced.push((position, values));
    plan.removed.extend(removals);
    plan.added_records = plan.added_records.saturating_add(added_count);
    plan.rebuilt.extend(records);
    Ok(Some(()))
}

/// 書き直した図形の値・新しく足す匿名ブロックの部品・増えるレコード数
type Rebuilt = (Vec<Value>, Vec<Record>, u64);

/// DWG の ACAD_TABLE の挿入点だけを変える。セルの内容・書式・大きさ・向きは変えない。
fn moved_table_values<P: Copy + PartialEq>(
    object: &Object,
    was: &Table<P>,
    table: &Table<P>,
    frame: &impl Frame<P>,
) -> Option<Vec<Value>> {
    if object.kind != b"ACAD_TABLE"
        || table.direction != was.direction
        || table.row_heights != was.row_heights
        || table.column_widths != was.column_widths
        || table.cells != was.cells
        || table.style != was.style
        || table.formats != was.formats
        || table.drawn != was.drawn
    {
        return None;
    }
    let mut values = object.values.clone();
    let marker = values
        .iter()
        .position(|v| matches!(v.code, 342 | 11 | 90))?;
    let [old_x, old_y] = frame.point(&was.position)?;
    let [x, y] = frame.point(&table.position)?;
    for (code, value, old_value) in [(10, x, old_x), (20, y, old_y)] {
        // 用紙の mm を戻したときの丸めで、動かしていない座標を変えない。
        if value == old_value {
            continue;
        }
        let field = values
            .get_mut(..marker)?
            .iter_mut()
            .find(|v| v.code == code)?;
        field.data = real(code, value)?.data;
    }
    Some(values)
}

/// 寸法の部品を作るためのスタイル値
struct DimParams {
    arrow_size: f64,
    tick_size: f64,
    offsets: DimensionOffsets,
    suppress_ext: [bool; 2],
    text_height: f64,
    linear_factor: f64,
    linear_dec: usize,
    angular_dec: usize,
    arc_sym: i64,
    text_style: String,
    dim_color: Inherit<Color>,
    ext_color: Inherit<Color>,
    text_color: Inherit<Color>,
    ends: [Arrowhead; 2],
}

impl DimParams {
    fn resolve(
        current: &Current<'_>,
        blocks: &BlockIndex,
        object: &Object,
        style_name: &str,
    ) -> Self {
        let style_obj = blocks.dimstyle(current.db, style_name);
        let mut reals = BTreeMap::new();
        let mut ints = BTreeMap::new();
        let mut handles = BTreeMap::new();

        if let Some(style_obj) = style_obj {
            for v in &style_obj.values {
                match v.data {
                    Data::Real(r) if r.is_finite() => {
                        reals.entry(v.code).or_insert(r);
                    }
                    Data::Int(i) => {
                        ints.entry(v.code).or_insert(i);
                    }
                    Data::Bool(b) => {
                        ints.entry(v.code).or_insert(i64::from(b));
                    }
                    Data::Handle(h) => {
                        handles.entry(v.code).or_insert(h);
                    }
                    _ => {}
                }
            }
        }
        apply_dstyle_xdata(object, current.charset, &mut reals, &mut ints, &mut handles);

        let real_or = |code: i16, default: f64| reals.get(&code).copied().unwrap_or(default);
        let int_or = |code: i16, default: i64| ints.get(&code).copied().unwrap_or(default);

        let raw_scale = real_or(40, 1.0);
        let scale = if raw_scale > 0.0 { raw_scale } else { 1.0 };
        let arrow_size = real_or(41, 0.18).max(0.0) * scale;
        let tick_size = real_or(142, 0.0).max(0.0) * scale;
        let offsets = DimensionOffsets {
            offset: real_or(42, 0.0625).max(0.0) * scale,
            extension: real_or(44, 0.18).max(0.0) * scale,
        };
        let suppress_ext = [int_or(75, 0) != 0, int_or(76, 0) != 0];
        let raw_txt = real_or(140, 0.18);
        let text_height = (if raw_txt > 0.0 { raw_txt } else { 0.18 }) * scale;
        let raw_lfac = real_or(144, 1.0);
        let linear_factor = if raw_lfac != 0.0 { raw_lfac.abs() } else { 1.0 };
        let linear_dec = usize::try_from(int_or(271, 4).clamp(0, 8)).unwrap_or(4);
        let adec_raw = int_or(179, 0);
        let angular_dec = if adec_raw < 0 {
            linear_dec
        } else {
            usize::try_from(adec_raw.clamp(0, 8)).unwrap_or(0)
        };
        let arc_sym = int_or(90, 0);

        let text_style = handles
            .get(&340)
            .copied()
            .filter(|&h| h != 0)
            .and_then(|h| current.db.get(h))
            .filter(|rec| rec.kind == b"STYLE")
            .and_then(|rec| match rec.first(2) {
                Some(Data::Text(bytes)) => current.charset.decode(bytes),
                _ => None,
            })
            .filter(|name| current.born.has_text_style(name))
            .unwrap_or_else(|| "Standard".to_owned());

        let block_arrow = |code: i16| -> Option<Arrowhead> {
            let &h = handles.get(&code)?;
            if h == 0 {
                return Some(Arrowhead::Default);
            }
            let rec = current.db.get(h).filter(|r| r.kind == b"BLOCK_RECORD")?;
            let Data::Text(bytes) = rec.first(2)? else {
                return None;
            };
            let name = current.charset.decode(bytes)?;
            Some(Arrowhead::Symbol(SymbolKey {
                namespace: super::BLOCK.into(),
                name,
            }))
        };
        let base_ends = current
            .drawing
            .dimension_style(style_name)
            .map(|s| s.ends.clone())
            .unwrap_or([Arrowhead::Default, Arrowhead::Default]);
        let ends = if int_or(173, 0) != 0 {
            [
                block_arrow(343).unwrap_or_else(|| base_ends[0].clone()),
                block_arrow(344).unwrap_or_else(|| base_ends[1].clone()),
            ]
        } else if let Some(both) = block_arrow(342) {
            [both.clone(), both]
        } else {
            base_ends
        };

        Self {
            arrow_size,
            tick_size,
            offsets,
            suppress_ext,
            text_height,
            linear_factor,
            linear_dec,
            angular_dec,
            arc_sym,
            text_style,
            dim_color: dim_aci(int_or(176, 0)),
            ext_color: dim_aci(int_or(177, 0)),
            text_color: dim_aci(int_or(178, 0)),
            ends,
        }
    }
}

fn dim_aci(code: i64) -> Inherit<Color> {
    match code {
        256 => Inherit::Layer,
        1..=255 => match u32::try_from(code) {
            Ok(value) => Inherit::Own(Color {
                code: Some(ColorCode {
                    system: ColorSystem::Aci,
                    value,
                }),
                rgb: None,
            }),
            Err(_) => Inherit::Block,
        },
        _ => Inherit::Block,
    }
}

/// 図形の XDATA にある `ACAD` / `DSTYLE` の上書きを拾う
fn apply_dstyle_xdata(
    object: &Object,
    charset: Charset,
    reals: &mut BTreeMap<i16, f64>,
    ints: &mut BTreeMap<i16, i64>,
    handles: &mut BTreeMap<i16, u64>,
) {
    let mut in_acad = false;
    let mut awaiting_brace = false;
    let mut in_dstyle = false;
    let mut key: Option<i16> = None;
    for value in &object.values {
        match (value.code, &value.data) {
            (1001, Data::Text(app)) => {
                in_acad = app.eq_ignore_ascii_case(b"ACAD");
                awaiting_brace = false;
                in_dstyle = false;
                key = None;
            }
            (1000, Data::Text(tag)) if in_acad && !in_dstyle => {
                if tag.as_slice() == b"DSTYLE" {
                    awaiting_brace = true;
                }
            }
            (1002, Data::Text(brace)) if awaiting_brace && brace.as_slice() == b"{" => {
                awaiting_brace = false;
                in_dstyle = true;
                key = None;
            }
            (1002, Data::Text(brace)) if in_dstyle && brace.as_slice() == b"}" => {
                in_dstyle = false;
                key = None;
            }
            (1070, Data::Int(code)) if in_dstyle && key.is_none() => {
                key = i16::try_from(*code).ok();
            }
            (_, data) if in_dstyle && key.is_some() => {
                if let Some(code) = key.take() {
                    match (value.code, data) {
                        (1040, Data::Real(r)) if r.is_finite() => {
                            reals.insert(code, *r);
                        }
                        (1070 | 1071, Data::Int(i)) => {
                            ints.insert(code, *i);
                        }
                        (1005, Data::Handle(h)) => {
                            handles.insert(code, *h);
                        }
                        (1005 | 1000, Data::Text(raw)) => {
                            if let Some(s) = charset.decode(raw)
                                && let Ok(h) = u64::from_str_radix(s.trim(), 16)
                            {
                                handles.insert(code, h);
                            }
                        }
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn plan_dimension<P: Copy + PartialEq>(
    current: &Current<'_>,
    blocks: &BlockIndex,
    object: &Object,
    was: &Dimension<P>,
    dimension: &Dimension<P>,
    record_handle: u64,
    frame: &impl Frame<P>,
    transformed: Option<&Affine>,
    base_index: u64,
    new_block: bool,
    cx: &mut Cx,
    place: &Ref,
) -> Result<Option<Rebuilt>, Error> {
    if dimension.style != was.style
        || dimension.block != was.block
        || dimension.text.attachment != was.text.attachment
    {
        return Ok(None);
    }
    let params = DimParams::resolve(current, blocks, object, &dimension.style);
    let Some(drawn) = parts::dimension(
        dimension,
        |p| frame.point(p),
        |l| frame.length(l),
        params.offsets,
    ) else {
        return Ok(None);
    };

    let text_pos = if transformed.is_some() {
        match &dimension.text.position {
            Some(pos) => {
                let Some(pt) = frame.point(pos) else {
                    return Ok(None);
                };
                pt
            }
            None => drawn.middle,
        }
    } else if dimension.text.moved {
        match &dimension.text.position {
            Some(pos) => {
                let Some(pt) = frame.point(pos) else {
                    return Ok(None);
                };
                pt
            }
            None => return Ok(None),
        }
    } else if dimension.kind == was.kind
        && let Some(pos) = &dimension.text.position
    {
        let Some(pt) = frame.point(pos) else {
            return Ok(None);
        };
        pt
    } else {
        drawn.middle
    };

    let values = match object.kind.as_slice() {
        b"DIMENSION" => update_dimension_values(
            object,
            was,
            dimension,
            &drawn,
            text_pos,
            frame,
            current.encoder,
            current.born,
        ),
        b"ARC_DIMENSION" => update_arc_dimension_values(
            object,
            was,
            dimension,
            &drawn,
            text_pos,
            frame,
            current.encoder,
            transformed,
        ),
        _ => None,
    };
    let Some(values) = values else {
        return Ok(None);
    };

    let Some((records, count)) = build_dimension_records(
        current.born,
        dimension,
        &drawn,
        text_pos,
        &params,
        record_handle,
        base_index,
        new_block,
        cx,
        place,
    )?
    else {
        return Ok(None);
    };
    Ok(Some((values, records, count)))
}

#[allow(clippy::too_many_arguments)]
fn build_dimension_records<P>(
    born: &Born,
    dimension: &Dimension<P>,
    drawn: &DimensionParts,
    text_pos: [f64; 2],
    params: &DimParams,
    record_handle: u64,
    base_index: u64,
    new_block: bool,
    cx: &mut Cx,
    place: &Ref,
) -> Result<Option<(Vec<Record>, u64)>, Error> {
    let Some(layer) = born.layer_zero() else {
        return Ok(None);
    };
    let home = Home {
        owner: Some(record_handle),
        blocks: true,
        paper: false,
    };
    let block_frame = InSpace(MODEL);
    let pt = |[x, y]: [f64; 2]| ModelPoint { space: MODEL, x, y };
    let len = |value: f64| {
        Length::Model(ModelLength {
            space: MODEL,
            value,
        })
    };
    let style_of = |color: Inherit<Color>| Style {
        color,
        line_type: Inherit::Block,
        line_type_scale: 1.0,
        line_weight: None,
        transparency: Inherit::Layer,
    };
    let dim_style = style_of(params.dim_color);
    let ext_style = style_of(params.ext_color);
    let txt_style = style_of(params.text_color);

    let mut records = Vec::new();
    let mut added = 0u64;
    let push_shape = |shape: Shape<ModelPoint>,
                      style: Style,
                      records: &mut Vec<Record>,
                      added: &mut u64|
     -> Option<()> {
        let element = Element {
            source: Ref::default(),
            layer,
            style,
            shape,
        };
        let idx = base_index.checked_add(*added)?;
        let rec = if new_block {
            born.record_in_new_block(&element, &block_frame, home, idx)?
        } else {
            born.record(&element, &block_frame, home, idx)?
        };
        *added = added.checked_add(rec.count())?;
        records.push(rec);
        Some(())
    };

    for (idx, &[a, b]) in drawn.extension_lines.iter().enumerate() {
        work(cx, 1, place)?;
        if params.suppress_ext.get(idx).copied().unwrap_or(false) {
            continue;
        }
        if libm::hypot(b[0] - a[0], b[1] - a[1]) > 1e-12
            && push_shape(
                Shape::Line {
                    start: pt(a),
                    end: pt(b),
                },
                ext_style,
                &mut records,
                &mut added,
            )
            .is_none()
        {
            return Ok(None);
        }
    }

    if let Some(curve) = drawn.curve {
        work(cx, 1, place)?;
        match curve {
            DimensionCurve::Line([a, b]) => {
                if libm::hypot(b[0] - a[0], b[1] - a[1]) > 1e-12
                    && push_shape(
                        Shape::Line {
                            start: pt(a),
                            end: pt(b),
                        },
                        dim_style,
                        &mut records,
                        &mut added,
                    )
                    .is_none()
                {
                    return Ok(None);
                }
            }
            DimensionCurve::Arc {
                center,
                radius,
                start,
                sweep,
            } => {
                if radius > 1e-12 && sweep > 1e-12 {
                    let shape = if sweep >= 360.0 - 1e-9 {
                        Shape::Circle {
                            center: pt(center),
                            radius: len(radius),
                        }
                    } else {
                        Shape::Curve(Edge::Arc {
                            center: pt(center),
                            radius: len(radius),
                            start: start.rem_euclid(360.0),
                            end: (start + sweep).rem_euclid(360.0),
                            counterclockwise: true,
                        })
                    };
                    if push_shape(shape, dim_style, &mut records, &mut added).is_none() {
                        return Ok(None);
                    }
                }
            }
        }
    }

    for end in &drawn.ends {
        work(cx, 1, place)?;
        if params.tick_size > 0.0 {
            if let Some([a, b]) = tick_line(end.tip, end.toward_tip, params.tick_size)
                && push_shape(
                    Shape::Line {
                        start: pt(a),
                        end: pt(b),
                    },
                    dim_style,
                    &mut records,
                    &mut added,
                )
                .is_none()
            {
                return Ok(None);
            }
            continue;
        }
        if params.arrow_size <= 0.0 || !params.arrow_size.is_finite() {
            continue;
        }
        let mut drew_symbol = false;
        if let Some(Arrowhead::Symbol(key)) = params.ends.get(end.side)
            && key.namespace == super::BLOCK
        {
            if key.name.eq_ignore_ascii_case("_NONE") {
                continue;
            }
            if born.has_block(&key.name) {
                let rot = libm::atan2(end.toward_tip[1], end.toward_tip[0]).to_degrees();
                if push_shape(
                    Shape::Symbol(Symbol {
                        key: key.clone(),
                        position: pt(end.tip),
                        scale: [params.arrow_size, params.arrow_size],
                        rotation: rot,
                        grid: None,
                        clip: None,
                        attributes: Vec::new(),
                    }),
                    dim_style,
                    &mut records,
                    &mut added,
                )
                .is_none()
                {
                    return Ok(None);
                }
                drew_symbol = true;
            }
        }
        if !drew_symbol
            && let Some([c0, c1, c2]) = arrow_triangle(end.tip, end.toward_tip, params.arrow_size)
        {
            let Some(idx) = base_index.checked_add(added) else {
                return Ok(None);
            };
            let Some(rec) = born.solid(layer, &dim_style, [c0, c1, c2, c2], home, idx, new_block)
            else {
                return Ok(None);
            };
            let Some(next_added) = added.checked_add(rec.count()) else {
                return Ok(None);
            };
            added = next_added;
            records.push(rec);
        }
    }

    let angular = matches!(
        dimension.kind,
        DimensionKind::Angular { .. } | DimensionKind::AngularLines { .. }
    );
    let precision = if angular {
        params.angular_dec
    } else {
        params.linear_dec
    };
    let display_val = if angular {
        drawn.display_value
    } else {
        drawn.display_value * params.linear_factor
    };
    if let Some(mut content) =
        dimension_mtext(&dimension.kind, &dimension.label, display_val, precision)
    {
        if matches!(dimension.kind, DimensionKind::ArcLength { .. })
            && matches!(dimension.label, Label::Measurement)
        {
            match params.arc_sym {
                0 => content.insert_str(0, "\\U+2312"),
                1 => content.insert_str(0, "\\U+2312\\P"),
                _ => {}
            }
        }
        work(cx, 1 + content.len() as u64, place)?;
        let deg = dimension
            .text
            .rotation
            .unwrap_or_else(|| readable_angle(drawn.along));
        let rad = deg.to_radians();
        let (horizontal, vertical) = dimension
            .text
            .attachment
            .unwrap_or((Horizontal::Center, Vertical::Middle));
        let spans = mtext::spans(&content, &|v| block_frame.lifted(v));
        let formatted = FormattedText {
            content,
            spans,
            point: pt(text_pos),
            height: Height {
                value: len(params.text_height),
                meaning: HeightMeaning::Declared("dxf.MTEXT.40".into()),
            },
            width: None,
            frame_height: None,
            horizontal,
            vertical,
            flow: Flow::LeftToRight,
            rotation: Rotation::Direction([libm::cos(rad), libm::sin(rad)]),
            line_spacing: LineSpacing {
                factor: 1.0,
                exact: false,
            },
            background: None,
            style: params.text_style.clone(),
        };
        if push_shape(
            Shape::Formatted(formatted),
            txt_style,
            &mut records,
            &mut added,
        )
        .is_none()
        {
            return Ok(None);
        }
    }

    Ok(Some((records, added)))
}

/// `DIMENSION`（DXF と DWG）のフィールドを書き直す
#[allow(clippy::too_many_arguments)]
fn update_dimension_values<P: PartialEq>(
    object: &Object,
    was: &Dimension<P>,
    dimension: &Dimension<P>,
    drawn: &DimensionParts,
    text_pos: [f64; 2],
    frame: &impl Frame<P>,
    encoder: &Encoder,
    born: &Born,
) -> Option<Vec<Value>> {
    // 複製の挿入点（12・22・32）・標高（31）・水平角（51）が 0 でないものは部品の座標系がずれるため扱わない
    for code in [12, 22, 31, 32, 51] {
        if let Some(Data::Real(v)) = object.first(code)
            && *v != 0.0
        {
            return None;
        }
    }
    let old_flags = match object.first(70)? {
        Data::Int(f) => *f,
        _ => return None,
    };
    let expected_kind: i64 = match (&dimension.kind, &was.kind) {
        (DimensionKind::Linear { .. }, DimensionKind::Linear { .. }) => 0,
        (DimensionKind::Aligned { .. }, DimensionKind::Aligned { .. }) => 1,
        (DimensionKind::AngularLines { .. }, DimensionKind::AngularLines { .. }) => 2,
        (DimensionKind::Diameter { .. }, DimensionKind::Diameter { .. }) => 3,
        (DimensionKind::Radius { .. }, DimensionKind::Radius { .. }) => 4,
        (DimensionKind::Angular { .. }, DimensionKind::Angular { .. }) => 5,
        (DimensionKind::Ordinate { .. }, DimensionKind::Ordinate { .. }) => 6,
        _ => return None,
    };
    if (old_flags & 0b1_1111) != expected_kind || (old_flags & !(0b1_1111 | 32 | 64 | 128)) != 0 {
        return None;
    }

    let xdata_at = object
        .values
        .iter()
        .position(|v| v.code == 1001)
        .unwrap_or(object.values.len());
    let (head, xdata) = object.values.split_at_checked(xdata_at)?;
    let mut main = head.to_vec();

    let mut new_flags = (old_flags & 32) | expected_kind;
    if let DimensionKind::Ordinate { axis: Axis::X, .. } = dimension.kind {
        new_flags |= 64;
    }
    if dimension.text.moved {
        new_flags |= 128;
    }
    set_int(&mut main, 70, new_flags)?;

    // 文字の中点（11・21・31）。DWG でも必須
    set_point3(&mut main, 11, text_pos, &[10, 30, 2])?;

    // 71（位置合わせ）は DWG で必須なので、なければ既定の 5 を補う
    if born.target() != Target::Dxf && !main.iter().any(|v| v.code == 71) {
        let code = dimension
            .text
            .attachment
            .map_or(5, |(h, v)| attachment_code(h, v));
        insert_in_dim_common(&mut main, int(71, code), &[70])?;
    }

    // 42（測定値）は、定義点が変わったときか DWG でないときに書く
    if dimension.kind != was.kind
        || (born.target() != Target::Dxf && !main.iter().any(|v| v.code == 42))
    {
        if main.iter().any(|v| v.code == 42) {
            set_real_existing(&mut main, 42, drawn.raw_measurement)?;
        } else {
            insert_in_dim_common(&mut main, real(42, drawn.raw_measurement)?, &[71, 70])?;
        }
    }

    // 1（利用者の文字）
    if dimension.label != was.label {
        let new_label = encode_dimension_label(&dimension.label, encoder)?;
        main.retain(|v| v.code != 1);
        if let Some(val) = new_label {
            insert_in_dim_common(&mut main, val, &[70, 31, 21])?;
        }
    }

    // 53（文字の回転、度）
    if dimension.text.rotation != was.text.rotation {
        main.retain(|v| v.code != 53);
        if let Some(rot) = dimension.text.rotation
            && (born.target() == Target::Dxf || rot != 0.0)
        {
            insert_in_dim_common(&mut main, real(53, rot)?, &[42, 1, 71, 70])?;
        }
    }

    // 種類ごとの定義点と角度・引出線長さ
    match &dimension.kind {
        DimensionKind::Linear {
            first,
            second,
            line,
            rotation,
            oblique,
        } => {
            set_point3(&mut main, 10, frame.point(line)?, &[2])?;
            set_point3(&mut main, 13, frame.point(first)?, &[30])?;
            set_point3(&mut main, 14, frame.point(second)?, &[33])?;
            set_linear_rotation(&mut main, *rotation, born.target() == Target::Dxf)?;
            set_optional_oblique(&mut main, *oblique)?;
        }
        DimensionKind::Aligned {
            first,
            second,
            line,
            oblique,
        } => {
            set_point3(&mut main, 10, frame.point(line)?, &[2])?;
            set_point3(&mut main, 13, frame.point(first)?, &[30])?;
            set_point3(&mut main, 14, frame.point(second)?, &[33])?;
            set_optional_oblique(&mut main, *oblique)?;
        }
        DimensionKind::Angular {
            vertex,
            first,
            second,
            arc,
        } => {
            set_point3(&mut main, 10, frame.point(arc)?, &[2])?;
            set_point3(&mut main, 13, frame.point(first)?, &[30])?;
            set_point3(&mut main, 14, frame.point(second)?, &[33])?;
            set_point3(&mut main, 15, frame.point(vertex)?, &[34])?;
        }
        DimensionKind::AngularLines { first, second, arc } => {
            set_point3(&mut main, 13, frame.point(&first[0])?, &[30])?;
            set_point3(&mut main, 14, frame.point(&first[1])?, &[33])?;
            set_point3(&mut main, 15, frame.point(&second[0])?, &[34])?;
            set_point3(&mut main, 10, frame.point(&second[1])?, &[2])?;
            set_point3(&mut main, 16, frame.point(arc)?, &[35, 34])?;
        }
        DimensionKind::Radius {
            center,
            chord,
            leader,
        } => {
            let lead = super::nonnegative(frame, leader)?;
            set_point3(&mut main, 10, frame.point(center)?, &[2])?;
            set_point3(&mut main, 15, frame.point(chord)?, &[30])?;
            set_subtype_real(&mut main, 40, lead, &[35, 25])?;
        }
        DimensionKind::Diameter {
            chord,
            opposite,
            leader,
        } => {
            let lead = super::nonnegative(frame, leader)?;
            set_point3(&mut main, 10, frame.point(opposite)?, &[2])?;
            set_point3(&mut main, 15, frame.point(chord)?, &[30])?;
            set_subtype_real(&mut main, 40, lead, &[35, 25])?;
        }
        DimensionKind::Ordinate {
            origin,
            feature,
            leader_end,
            ..
        } => {
            set_point3(&mut main, 10, frame.point(origin)?, &[2])?;
            set_point3(&mut main, 13, frame.point(feature)?, &[30])?;
            set_point3(&mut main, 14, frame.point(leader_end)?, &[33])?;
        }
        DimensionKind::ArcLength { .. } => return None,
    }

    main.extend_from_slice(xdata);
    Some(main)
}

/// `ARC_DIMENSION` の共通のフィールドと、弧長固有のフィールドを分けて書き直す。
#[allow(clippy::too_many_arguments)]
fn update_arc_dimension_values<P: PartialEq>(
    object: &Object,
    was: &Dimension<P>,
    dimension: &Dimension<P>,
    drawn: &DimensionParts,
    text_pos: [f64; 2],
    frame: &impl Frame<P>,
    encoder: &Encoder,
    transformed: Option<&Affine>,
) -> Option<Vec<Value>> {
    let (
        DimensionKind::ArcLength {
            center,
            first,
            second,
            arc,
        },
        DimensionKind::ArcLength {
            center: was_center,
            first: was_first,
            second: was_second,
            ..
        },
    ) = (&dimension.kind, &was.kind)
    else {
        return None;
    };

    let marker = object.values.iter().position(|v| {
        v.code == 100 && matches!(&v.data, Data::Text(t) if t.as_slice() == b"AcDbArcDimension")
    });
    // DWG の共有表はサブクラスを持たず、弧長固有の値を 13 から後ろに置く（objects::annotations）。
    let arc_marker = marker.or_else(|| object.values.iter().position(|v| v.code == 13))?;
    let arc_begin = arc_marker + usize::from(marker.is_some());
    let xdata_at = object
        .values
        .iter()
        .position(|v| v.code == 1001)
        .unwrap_or(object.values.len());
    if arc_marker >= xdata_at {
        return None;
    }
    let common_slice = object.values.get(..arc_marker)?;
    let arc_slice = object.values.get(arc_begin..xdata_at)?;
    let xdata_slice = object.values.get(xdata_at..)?;

    for code in [12, 22, 31, 51] {
        if common_slice
            .iter()
            .any(|v| v.code == code && !matches!(v.data, Data::Real(r) if r == 0.0))
        {
            return None;
        }
    }
    let partial = match arc_slice.iter().find(|v| v.code == 70).map(|v| &v.data) {
        None | Some(Data::Int(0)) => false,
        Some(Data::Int(1)) => true,
        _ => return None,
    };
    if partial {
        let Affine {
            m: [a, b, _, d, e, _],
        } = transformed?;
        if (*a, *b, *d, *e) != (1.0, 0.0, 0.0, 1.0) {
            return None;
        }
        // 公開サンプルの部分弧は、13・14 が部分の両端で、16・17 も同じ点を持つ。
        // この形の平行移動だけを受け付け、部分の角度と測定値を元のまま保つ。
        for (code, point) in [(16, was_first), (17, was_second)] {
            let [x, y] = frame.point(point)?;
            for (at, expected) in [(code, x), (code + 10, y), (code + 20, 0.0)] {
                match arc_slice.iter().find(|v| v.code == at).map(|v| &v.data) {
                    Some(Data::Real(value)) if *value == expected => {}
                    _ => return None,
                }
            }
        }
    }
    let has_leader = arc_slice
        .iter()
        .find_map(|v| match v.data {
            Data::Int(flag) if v.code == 71 => Some(flag != 0),
            _ => None,
        })
        .unwrap_or(false);
    let endpoints_changed = center != was_center || first != was_first || second != was_second;
    if has_leader && (partial || endpoints_changed) {
        return None;
    }

    let mut common = common_slice.to_vec();
    let old_flags = common.iter().find_map(|v| match v.data {
        Data::Int(f) if v.code == 70 => Some(f),
        _ => None,
    })?;
    let new_flags = (old_flags & !128) | if dimension.text.moved { 128 } else { 0 };
    set_int(&mut common, 70, new_flags)?;
    set_point3(&mut common, 10, frame.point(arc)?, &[2])?;
    set_point3(&mut common, 11, text_pos, &[30, 20, 10])?;
    if dimension.kind != was.kind && !partial {
        if common.iter().any(|v| v.code == 42) {
            set_real_existing(&mut common, 42, drawn.raw_measurement)?;
        } else {
            insert_after_codes(&mut common, real(42, drawn.raw_measurement)?, &[71, 70])?;
        }
    }
    if dimension.label != was.label {
        let new_label = encode_dimension_label(&dimension.label, encoder)?;
        common.retain(|v| v.code != 1);
        if let Some(val) = new_label {
            insert_after_codes(&mut common, val, &[70, 31, 21])?;
        }
    }
    if dimension.text.rotation != was.text.rotation {
        common.retain(|v| v.code != 53);
        if let Some(rot) = dimension.text.rotation {
            insert_after_codes(&mut common, real(53, rot)?, &[42, 1, 71, 70])?;
        }
    }

    let mut arc_part = arc_slice.to_vec();
    let p_first = frame.point(first)?;
    let p_second = frame.point(second)?;
    let p_center = frame.point(center)?;
    set_point3(&mut arc_part, 13, p_first, &[])?;
    set_point3(&mut arc_part, 14, p_second, &[33])?;
    set_point3(&mut arc_part, 15, p_center, &[34])?;
    if endpoints_changed && let Some(DimensionCurve::Arc { start, sweep, .. }) = drawn.curve {
        if !partial {
            let start_rad = start.rem_euclid(360.0).to_radians();
            let end_rad = if sweep >= 360.0 && start_rad == 0.0 {
                core::f64::consts::TAU
            } else {
                (start + sweep).rem_euclid(360.0).to_radians()
            };
            set_subtype_real(&mut arc_part, 40, start_rad, &[70, 35])?;
            set_subtype_real(&mut arc_part, 41, end_rad, &[40])?;
        }
        if !has_leader {
            if arc_part.iter().any(|v| v.code == 16) {
                set_point3(&mut arc_part, 16, p_first, &[71])?;
            }
            if arc_part.iter().any(|v| v.code == 17) {
                set_point3(&mut arc_part, 17, p_second, &[36])?;
            }
        }
    }

    let mut out = Vec::with_capacity(common.len() + 1 + arc_part.len() + xdata_slice.len());
    out.extend(common);
    if marker.is_some() {
        out.push(text(100, "AcDbArcDimension"));
    }
    out.extend(arc_part);
    out.extend_from_slice(xdata_slice);
    Some(out)
}

fn encode_dimension_label(label: &Label, encoder: &Encoder) -> Option<Option<Value>> {
    match label {
        Label::Measurement => Some(None),
        Label::Hidden => Some(Some(text(1, " "))),
        Label::Override { content, .. } => {
            if content.contains(['\n', '\r']) {
                return None;
            }
            let bytes = encoder.encode(content)?;
            if bytes.len() > 250 {
                return None;
            }
            Some(Some(Value {
                code: 1,
                data: Data::Text(bytes),
            }))
        }
    }
}

fn attachment_code(h: Horizontal, v: Vertical) -> i64 {
    let col = match h {
        Horizontal::Left => 1,
        Horizontal::Center | Horizontal::Middle => 2,
        Horizontal::Right => 3,
    };
    let row = match v {
        Vertical::Top => 0,
        Vertical::Middle => 1,
        Vertical::Bottom | Vertical::Baseline => 2,
    };
    row * 3 + col
}

fn set_int(values: &mut [Value], code: i16, new_val: i64) -> Option<()> {
    let slot = values.iter_mut().find(|v| v.code == code)?;
    slot.data = Data::Int(new_val);
    Some(())
}

fn set_real_existing(values: &mut [Value], code: i16, new_val: f64) -> Option<()> {
    if !new_val.is_finite() {
        return None;
    }
    let slot = values.iter_mut().find(|v| v.code == code)?;
    slot.data = Data::Real(new_val);
    Some(())
}

/// 3 次元の点（`code`・`code + 10`・`code + 20`、Z = 0）を書き直す。なければ `after` の後ろに入れる
fn set_point3(values: &mut Vec<Value>, code: i16, [x, y]: [f64; 2], after: &[i16]) -> Option<()> {
    if !(x.is_finite() && y.is_finite()) {
        return None;
    }
    let has_x = values.iter().any(|v| v.code == code);
    let has_y = values.iter().any(|v| v.code == code + 10);
    if has_x && has_y {
        set_real_existing(values, code, x)?;
        set_real_existing(values, code + 10, y)?;
        if values.iter().any(|v| v.code == code + 20) {
            set_real_existing(values, code + 20, 0.0)?;
        } else {
            insert_after_codes(values, real(code + 20, 0.0)?, &[code + 10])?;
        }
        return Some(());
    }
    if has_x || has_y {
        return None;
    }
    let at = find_insert_pos(values, after)?;
    values.splice(
        at..at,
        [real(code, x)?, real(code + 10, y)?, real(code + 20, 0.0)?],
    );
    Some(())
}

/// `AcDbDimension` の共通サブクラスの中（次の `100` より前）に値を挿入する
fn insert_in_dim_common(values: &mut Vec<Value>, value: Value, after: &[i16]) -> Option<()> {
    let dim_marker = values.iter().position(|v| {
        v.code == 100 && matches!(&v.data, Data::Text(t) if t.as_slice() == b"AcDbDimension")
    });
    let subtype_marker = match dim_marker {
        Some(start) => values
            .iter()
            .enumerate()
            .skip(start + 1)
            .find_map(|(idx, v)| (v.code == 100).then_some(idx)),
        None => None,
    };
    let limit = subtype_marker.unwrap_or(values.len());
    let slice = values.get(..limit)?;
    let pos = after
        .iter()
        .find_map(|&c| slice.iter().rposition(|v| v.code == c).map(|idx| idx + 1))
        .unwrap_or(limit);
    values.insert(pos, value);
    Some(())
}

fn set_subtype_real(values: &mut Vec<Value>, code: i16, val: f64, after: &[i16]) -> Option<()> {
    let v = real(code, val)?;
    if let Some(slot) = values.iter_mut().find(|item| item.code == code) {
        slot.data = v.data;
        return Some(());
    }
    insert_after_codes(values, v, after)
}

fn set_optional_oblique(values: &mut Vec<Value>, oblique: f64) -> Option<()> {
    if !oblique.is_finite() {
        return None;
    }
    if oblique == 0.0 {
        values.retain(|v| v.code != 52);
        return Some(());
    }
    let v = real(52, oblique)?;
    if let Some(slot) = values.iter_mut().find(|item| item.code == 52) {
        slot.data = v.data;
        return Some(());
    }
    // `50`・`52` は `AcDbAlignedDimension` の中に置き、`100 AcDbRotatedDimension` があればその前に入れる
    if let Some(rot_marker) = values.iter().position(|item| {
        item.code == 100
            && matches!(&item.data, Data::Text(t) if t.as_slice() == b"AcDbRotatedDimension")
    }) {
        values.insert(rot_marker, v);
        return Some(());
    }
    insert_after_codes(values, v, &[50, 34, 24])
}

fn set_linear_rotation(values: &mut Vec<Value>, rotation: f64, is_dxf: bool) -> Option<()> {
    if !rotation.is_finite() {
        return None;
    }
    if rotation == 0.0 {
        values.retain(|v| v.code != 50);
        return Some(());
    }
    let v = real(50, rotation)?;
    if let Some(slot) = values.iter_mut().find(|item| item.code == 50) {
        slot.data = v.data;
        return Some(());
    }
    // DXF では `50` も `AcDbAlignedDimension` の中に置き、`100 AcDbRotatedDimension` はその後ろに置く
    if let Some(before) = values.iter().position(|item| {
        item.code == 52
            || (item.code == 100
                && matches!(&item.data, Data::Text(t) if t.as_slice() == b"AcDbRotatedDimension"))
    }) {
        values.insert(before, v);
        return Some(());
    }
    if is_dxf
        && values.iter().any(|item| {
            item.code == 100
                && matches!(&item.data, Data::Text(t) if t.as_slice() == b"AcDbAlignedDimension")
        })
    {
        values.push(v);
        values.push(text(100, "AcDbRotatedDimension"));
        return Some(());
    }
    insert_after_codes(values, v, &[34, 24])
}

fn find_insert_pos(values: &[Value], after: &[i16]) -> Option<usize> {
    if after.is_empty() {
        return Some(0);
    }
    after
        .iter()
        .find_map(|&c| values.iter().rposition(|v| v.code == c).map(|idx| idx + 1))
        .or(Some(values.len()))
}

fn insert_after_codes(values: &mut Vec<Value>, value: Value, after: &[i16]) -> Option<()> {
    let pos = find_insert_pos(values, after)?;
    values.insert(pos, value);
    Some(())
}

/// `ACAD_TABLE` の書き直しと `*T<n>` の部品生成。DWG はセルの回転と未対応の書式を断る。
#[allow(clippy::too_many_arguments)]
fn plan_table<P: Copy + PartialEq>(
    current: &Current<'_>,
    blocks: &BlockIndex,
    object: &Object,
    was: &Table<P>,
    table: &Table<P>,
    record_handle: u64,
    frame: &impl Frame<P>,
    base_index: u64,
    new_block: bool,
    cx: &mut Cx,
    place: &Ref,
) -> Result<Option<Rebuilt>, Error> {
    let dwg = matches!(current.target, Target::Dwg(_));
    if dwg {
        let legacy = !current.born.since(b"AC1024");
        if current.born.since(b"AC1021") && legacy {
            return Ok(None);
        }
        if legacy && (was.style.is_some() || was.formats.is_some()) {
            return Ok(None);
        }
        if was.cells.iter().zip(&table.cells).any(|(old, new)| {
            old.rotation != 0.0
                || new.rotation != 0.0
                || (legacy && (old.value.is_some() || old.format != Default::default()))
        }) {
            return Ok(None);
        }
    }
    if object.kind != b"ACAD_TABLE"
        || table.style != was.style
        || table.drawn != was.drawn
        || table.formats != was.formats
        || table.row_heights.len() != was.row_heights.len()
        || table.column_widths.len() != was.column_widths.len()
        || table.cells.len() != was.cells.len()
    {
        return Ok(None);
    }
    let Some(values) = update_table_values(object, was, table, frame, current.encoder) else {
        return Ok(None);
    };
    let default_height = blocks.standard_text_height.unwrap_or(2.5);
    let Some((records, count)) = build_table_records(
        current.born,
        table,
        frame,
        default_height,
        record_handle,
        base_index,
        new_block,
        cx,
        place,
    )?
    else {
        return Ok(None);
    };
    Ok(Some((values, records, count)))
}

fn update_table_values<P: PartialEq>(
    object: &Object,
    was: &Table<P>,
    table: &Table<P>,
    frame: &impl Frame<P>,
    encoder: &Encoder,
) -> Option<Vec<Value>> {
    let dwg = matches!(object.origin, crate::Origin::Dwg { .. });
    let marker = object.values.iter().position(|v| {
        if dwg {
            matches!(v.code, 342 | 11 | 90)
        } else {
            v.code == 100 && matches!(&v.data, Data::Text(t) if t.as_slice() == b"AcDbTable")
        }
    })?;
    let xdata_at = object
        .values
        .iter()
        .position(|v| v.code == 1001)
        .unwrap_or(object.values.len());
    if marker >= xdata_at {
        return None;
    }
    let ref_slice = object.values.get(..marker)?;
    let table_slice = object.values.get(marker..xdata_at)?;
    let xdata_slice = object.values.get(xdata_at..)?;

    let first_cell = table_slice.iter().position(|v| v.code == 171)?;
    let (head_slice, mut body_slice) = table_slice.split_at_checked(first_cell)?;
    let block = if dwg {
        let (block, cells) = body_slice.split_last()?;
        if block.code != 2 || !matches!(block.data, Data::Text(_)) {
            return None;
        }
        body_slice = cells;
        Some(block)
    } else {
        None
    };

    // 1. 挿入点（10・20・30）
    let mut reference = ref_slice.to_vec();
    set_point3(&mut reference, 10, frame.point(&table.position)?, &[2])?;

    // 2. 横の向き（11・21・31）・行の高さ（141）・列の幅（142）
    let [dx, dy] = table.direction;
    if !(dx.is_finite() && dy.is_finite()) {
        return None;
    }
    if table.direction != was.direction {
        let norm = libm::hypot(dx, dy);
        if !(norm > 0.0 && norm.is_finite()) {
            return None;
        }
    }
    let mut heights = Vec::with_capacity(table.row_heights.len());
    for h in &table.row_heights {
        heights.push(positive(frame, h)?);
    }
    let mut widths = Vec::with_capacity(table.column_widths.len());
    for w in &table.column_widths {
        widths.push(positive(frame, w)?);
    }
    let mut head = head_slice.to_vec();
    if table.direction != was.direction {
        set_point3(&mut head, 11, [dx, dy], &[342, 90])?;
    }
    let mut h_iter = heights.into_iter();
    let mut w_iter = widths.into_iter();
    for item in &mut head {
        match item.code {
            141 => item.data = Data::Real(h_iter.next()?),
            142 => item.data = Data::Real(w_iter.next()?),
            _ => {}
        }
    }
    if h_iter.next().is_some() || w_iter.next().is_some() {
        return None;
    }

    // 3. セルごとの並び
    let raw_cells: Vec<&[Value]> = body_slice.chunk_by(|_, next| next.code != 171).collect();
    if raw_cells.len() != table.cells.len() {
        return None;
    }
    let mut out = Vec::with_capacity(object.values.len() + 32);
    out.extend(reference);
    out.extend(head);
    for ((raw, new_cell), old_cell) in raw_cells.into_iter().zip(&table.cells).zip(&was.cells) {
        if new_cell.span != old_cell.span
            || new_cell.covered != old_cell.covered
            || new_cell.format != old_cell.format
        {
            return None;
        }
        out.extend(update_cell_values(raw, old_cell, new_cell, encoder, dwg)?);
    }
    out.extend(block.cloned());
    out.extend_from_slice(xdata_slice);
    Some(out)
}

fn update_cell_values(
    raw: &[Value],
    old_cell: &zumen_drawing::Cell,
    new_cell: &zumen_drawing::Cell,
    encoder: &Encoder,
    dwg: bool,
) -> Option<Vec<Value>> {
    if !new_cell.rotation.is_finite() {
        return None;
    }
    let val_start = raw.iter().position(|v| v.code == 301).unwrap_or(raw.len());
    let (fields_slice, rest_slice) = raw.split_at_checked(val_start)?;
    let mut fields = fields_slice.to_vec();

    if new_cell.rotation != old_cell.rotation {
        let r = real(145, new_cell.rotation)?;
        if let Some(slot) = fields.iter_mut().find(|v| v.code == 145) {
            slot.data = r.data;
        } else if new_cell.rotation != 0.0 {
            insert_after_codes(&mut fields, r, &[174, 173, 172, 171])?;
        }
    }

    if new_cell.content == old_cell.content {
        if new_cell.value != old_cell.value {
            return None;
        }
        fields.extend_from_slice(rest_slice);
        return Some(fields);
    }

    // 内容を変えたセルは文字列か空の CELL_VALUE に書き直すため、別の型付き CellValue の指定は受けない
    if new_cell.value.is_some() && new_cell.value != old_cell.value {
        return None;
    }
    // フィールド（344 が 0 でないハンドル）を持つセルは式を壊さないよう拒む
    if fields
        .iter()
        .any(|v| v.code == 344 && !matches!(v.data, Data::Handle(0)))
    {
        return None;
    }

    let new_text = match (&old_cell.content, &new_cell.content) {
        (CellContent::Empty | CellContent::Text { .. }, CellContent::Empty) => "",
        (CellContent::Empty | CellContent::Text { .. }, CellContent::Text { content, .. }) => {
            if content.is_empty() {
                return None;
            }
            content.as_str()
        }
        _ => return None,
    };

    // R2010 からの DWG は 301..304 だけで文字を持ち、1・2 を 250 バイトで分割しない
    if dwg && !rest_slice.is_empty() {
        let end_rel = rest_slice.iter().position(|v| v.code == 304)?;
        let after_val = rest_slice.get(end_rel + 1..)?;
        fields.retain(|v| !matches!(v.code, 1 | 2));
        fields.push(text(301, "CELL_VALUE"));
        if new_text.is_empty() {
            fields.extend([
                int(93, 1),
                int(90, 0),
                int(94, 0),
                text(300, ""),
                text(302, ""),
                text(304, "ACVALUE_END"),
            ]);
        } else {
            let encoded = encoder.encode(new_text)?;
            fields.extend([
                int(93, 0),
                int(90, 4),
                Value {
                    code: 1,
                    data: Data::Text(encoded.clone()),
                },
                int(94, 0),
                text(300, ""),
                Value {
                    code: 302,
                    data: Data::Text(encoded),
                },
                text(304, "ACVALUE_END"),
            ]);
        }
        fields.extend_from_slice(after_val);
        return Some(fields);
    }

    let text_chunks = split_text(new_text, 2, 1, encoder)?;
    let insert_at = fields
        .iter()
        .position(|v| matches!(v.code, 1 | 2))
        .unwrap_or(fields.len());
    fields.retain(|v| !matches!(v.code, 1 | 2));
    fields.splice(insert_at..insert_at, text_chunks.clone());

    if rest_slice.is_empty() {
        return Some(fields);
    }

    // 2007 からの DXF の `301 CELL_VALUE .. 304 ACVALUE_END` を書き直す
    let end_rel = rest_slice.iter().position(|v| v.code == 304)?;
    let after_val = rest_slice.get(end_rel + 1..)?;
    fields.push(text(301, "CELL_VALUE"));
    if new_text.is_empty() {
        fields.extend([
            int(90, 0),
            int(93, 1),
            text(300, ""),
            text(302, ""),
            text(304, "ACVALUE_END"),
        ]);
    } else {
        let fmt_chunks = split_text(new_text, 303, 302, encoder)?;
        fields.extend([int(90, 4), int(93, 0)]);
        fields.extend(text_chunks);
        fields.push(int(94, 0));
        fields.push(text(300, ""));
        fields.extend(fmt_chunks);
        fields.push(text(304, "ACVALUE_END"));
    }
    fields.extend_from_slice(after_val);
    Some(fields)
}

#[allow(clippy::too_many_arguments)]
fn build_table_records<P>(
    born: &Born,
    table: &Table<P>,
    frame: &impl Frame<P>,
    default_height: f64,
    record_handle: u64,
    base_index: u64,
    new_block: bool,
    cx: &mut Cx,
    place: &Ref,
) -> Result<Option<(Vec<Record>, u64)>, Error> {
    let Some(layer) = born.layer_zero() else {
        return Ok(None);
    };
    let Some(drawn) = parts::table(table, [0.0, 0.0], |l| frame.length(l), Some(default_height))
    else {
        return Ok(None);
    };
    let home = Home {
        owner: Some(record_handle),
        blocks: true,
        paper: false,
    };
    let block_frame = InSpace(MODEL);
    let pt = |[x, y]: [f64; 2]| ModelPoint { space: MODEL, x, y };
    let len = |value: f64| {
        Length::Model(ModelLength {
            space: MODEL,
            value,
        })
    };
    let by_block = Style {
        color: Inherit::Block,
        line_type: Inherit::Block,
        line_type_scale: 1.0,
        line_weight: None,
        transparency: Inherit::Layer,
    };

    let mut records = Vec::new();
    let mut added = 0u64;
    let mut push_shape = |shape: Shape<ModelPoint>, style: Style| -> Option<()> {
        let element = Element {
            source: Ref::default(),
            layer,
            style,
            shape,
        };
        let idx = base_index.checked_add(added)?;
        let rec = if new_block {
            born.record_in_new_block(&element, &block_frame, home, idx)?
        } else {
            born.record(&element, &block_frame, home, idx)?
        };
        added = added.checked_add(rec.count())?;
        records.push(rec);
        Some(())
    };

    for [a, b] in drawn.borders {
        work(cx, 1, place)?;
        if libm::hypot(b[0] - a[0], b[1] - a[1]) > 1e-12
            && push_shape(
                Shape::Line {
                    start: pt(a),
                    end: pt(b),
                },
                by_block,
            )
            .is_none()
        {
            return Ok(None);
        }
    }

    for item in drawn.texts {
        work(cx, 1 + item.formatted.len() as u64, place)?;
        let style_name = item
            .text_style
            .filter(|name| born.has_text_style(name))
            .unwrap_or_else(|| "Standard".to_owned());
        let color = match item.text_color {
            Some(Inherit::Layer) => Inherit::Layer,
            Some(Inherit::Own(c)) if c.code.is_some() => Inherit::Own(Color {
                code: c.code,
                rgb: None,
            }),
            _ => Inherit::Block,
        };
        let rad = item.rotation.to_radians();
        let spans = mtext::spans(&item.formatted, &|v| block_frame.lifted(v));
        let formatted = FormattedText {
            content: item.formatted,
            spans,
            point: pt(item.position),
            height: Height {
                value: len(item.height),
                meaning: HeightMeaning::Declared("dxf.MTEXT.40".into()),
            },
            width: (item.width > 0.0).then_some(len(item.width)),
            frame_height: None,
            horizontal: item.alignment.0,
            vertical: item.alignment.1,
            flow: Flow::LeftToRight,
            rotation: Rotation::Direction([libm::cos(rad), libm::sin(rad)]),
            line_spacing: LineSpacing {
                factor: 1.0,
                exact: false,
            },
            background: None,
            style: style_name,
        };
        let text_style = Style { color, ..by_block };
        if push_shape(Shape::Formatted(formatted), text_style).is_none() {
            return Ok(None);
        }
    }

    for sym in drawn.symbols {
        work(cx, 1, place)?;
        if sym.key.namespace != super::BLOCK || !born.has_block(&sym.key.name) {
            return Ok(None);
        }
        if push_shape(
            Shape::Symbol(Symbol {
                key: sym.key,
                position: pt(sym.position),
                scale: [1.0, 1.0],
                rotation: sym.rotation,
                grid: None,
                clip: None,
                attributes: Vec::new(),
            }),
            by_block,
        )
        .is_none()
        {
            return Ok(None);
        }
    }

    Ok(Some((records, added)))
}

#[cfg(test)]
mod table_move_tests {
    use super::*;
    use crate::db::Origin;
    use crate::drawing::PaperFrame;
    use crate::drawing::apply::OnSheet;
    use zumen_drawing::{Cell, PaperLength, PaperPoint, SheetId};

    #[test]
    fn paper_move_preserves_the_unmoved_raw_coordinate() {
        let sheet = SheetId(0);
        let frame = OnSheet(
            sheet,
            PaperFrame {
                origin: [10.1, 2.7],
                mm: 25.4,
                home: Home {
                    owner: None,
                    blocks: false,
                    paper: true,
                },
            },
        );
        let was = Table {
            position: PaperPoint {
                sheet,
                x: (12.0 - 10.1) * 25.4,
                y: (0.1 - 2.7) * 25.4,
            },
            direction: [1.0, 0.0],
            row_heights: vec![Length::Paper(PaperLength(25.4))],
            column_widths: vec![Length::Paper(PaperLength(25.4))],
            cells: vec![Cell {
                content: CellContent::Empty,
                span: [1, 1],
                covered: false,
                value: None,
                rotation: 0.0,
                format: Default::default(),
            }],
            style: None,
            formats: None,
            drawn: None,
        };
        // インチと用紙の mm の往復で、元の Y と最後の桁が異なる入力。
        assert_ne!(frame.point(&was.position).unwrap()[1], 0.1);
        let object = Object {
            kind: b"ACAD_TABLE".to_vec(),
            section: Section::Entities,
            values: vec![
                real(10, 12.0).unwrap(),
                real(20, 0.1).unwrap(),
                real(30, 0.0).unwrap(),
                real(11, 1.0).unwrap(),
            ],
            origin: Origin::New,
        };
        let mut moved = was.clone();
        moved.position.x += 25.4;
        let values = moved_table_values(&object, &was, &moved, &frame).unwrap();
        assert_eq!(values[0], real(10, 13.0).unwrap());
        assert_eq!(values[1..], object.values[1..]);
        assert_eq!(
            moved_table_values(&object, &was, &was, &frame).unwrap(),
            object.values
        );
    }
}
