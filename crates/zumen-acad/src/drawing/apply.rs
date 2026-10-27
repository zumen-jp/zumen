//! 図面モデルの変更を、共有の表を通して DXF・DWG に書き戻す（決定 0139）。

use std::collections::BTreeMap;

use super::{lift, source, text};
use crate::db::{AcadDb, Data, Object, Section};
use crate::{DwgFile, DxfFile};
use zumen_base::{Cx, Diagnostic, Diagnostics, Ref, codes};
use zumen_drawing::{Changes, Edit, Element, Length, ModelPoint, Shape, SpaceId};
use zumen_sdk::{Apply, Error};

impl Apply<Changes> for DxfFile {
    /// 今のファイルから表を作り、変更を当ててから書き戻す
    fn apply(&mut self, changes: &Changes, cx: &mut Cx) -> Result<(), Error> {
        let mut db = self.table(cx, false)?;
        edit(&mut db, changes, cx)?;
        self.write_back(&db, cx)
    }
}

impl Apply<Changes> for DwgFile {
    /// 今のファイルから表を作り、変更を当ててから書き戻す
    fn apply(&mut self, changes: &Changes, cx: &mut Cx) -> Result<(), Error> {
        let mut db = self.objects(cx)?;
        edit(&mut db, changes, cx)?;
        self.write_back(&db, cx)
    }
}

/// 変更を表に当てる。当てられない変更が 1 つでもあれば、表を変えずに失敗する
fn edit(db: &mut AcadDb, changes: &Changes, cx: &mut Cx) -> Result<(), Error> {
    // 今の図面モデル。変換の診断は、書き戻しの診断に混ぜない
    let mut quiet = Diagnostics::new();
    let drawing = lift(db, &mut Cx::new(cx.budget, &mut quiet, cx.cancel))?;
    let elements: BTreeMap<&Ref, &Element<ModelPoint>> = drawing
        .spaces
        .iter()
        .flat_map(|space| &space.elements)
        .map(|element| (&element.source, element))
        .collect();
    let mut replaced = Vec::new();
    let mut removed = Vec::new();
    let mut rejected = false;
    for change in &changes.edits {
        let at = match change {
            Edit::Replace(element) => &element.source,
            Edit::Remove(at) => at,
        };
        let (Some(original), Some(index)) = (elements.get(at), entity(db, at)) else {
            cx.report(Diagnostic::new(codes::DXF_ELEMENT_MISSING).at(at.clone()));
            rejected = true;
            continue;
        };
        let object = db.objects().get(index).ok_or(Error::Rejected)?;
        let planned = match change {
            Edit::Replace(element) => {
                replacement(original, element, object).map(|values| replaced.push((index, values)))
            }
            Edit::Remove(_) => (!followed(object)).then(|| removed.push(index)),
        };
        if planned.is_none() {
            cx.report(
                Diagnostic::new(codes::DXF_CHANGE_UNSUPPORTED)
                    .with("kind", text(&object.kind))
                    .at(at.clone()),
            );
            rejected = true;
        }
    }
    if rejected {
        return Err(Error::Rejected);
    }
    for (index, values) in replaced {
        let object = db.object_mut(index).ok_or(Error::Rejected)?;
        for (code, value) in values {
            let slot = object
                .values
                .iter_mut()
                .find(|v| v.code == code)
                .ok_or(Error::Rejected)?;
            slot.data = Data::Real(value);
        }
    }
    removed.sort_unstable();
    removed.dedup();
    for index in removed.into_iter().rev() {
        db.remove(index);
    }
    Ok(())
}

/// 出所が `at` の図形の、表の中の番号
fn entity(db: &AcadDb, at: &Ref) -> Option<usize> {
    db.objects()
        .iter()
        .position(|object| object.section == Section::Entities && source(object) == *at)
}

/// 続くレコード（POLYLINE の VERTEX、属性を持つ INSERT の ATTRIB、SEQEND）を持つ図形か
fn followed(object: &Object) -> bool {
    match object.kind.as_slice() {
        b"POLYLINE" => true,
        b"INSERT" => matches!(object.first(66), Some(Data::Int(n)) if *n != 0),
        _ => false,
    }
}

/// 置き換えで書く値（グループ・コードと実数）。レイヤ・スタイル・空間が同じで、形の種類が元の
/// レコードで表せるときだけ返す
fn replacement(
    original: &Element<ModelPoint>,
    element: &Element<ModelPoint>,
    object: &Object,
) -> Option<Vec<(i16, f64)>> {
    if element.layer != original.layer || element.style != original.style {
        return None;
    }
    let space = space(&original.shape)?;
    match (&element.shape, object.kind.as_slice()) {
        (Shape::Line { start, end }, b"LINE") if start.space == space && end.space == space => {
            Some(vec![(10, start.x), (20, start.y), (11, end.x), (21, end.y)])
        }
        (
            Shape::Circle {
                center,
                radius: Length::Model(radius),
            },
            b"CIRCLE",
        ) if center.space == space && radius.space == space => {
            Some(vec![(10, center.x), (20, center.y), (40, radius.value)])
        }
        _ => None,
    }
}

/// 置き換えられる形の空間
fn space(shape: &Shape<ModelPoint>) -> Option<SpaceId> {
    match shape {
        Shape::Line { start, .. } => Some(start.space),
        Shape::Circle { center, .. } => Some(center.space),
        _ => None,
    }
}
