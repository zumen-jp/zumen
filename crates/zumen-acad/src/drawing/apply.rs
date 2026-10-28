//! 図面モデルの変更を、共有の表を通して DXF・DWG に書き戻す（決定 0139）。

use std::collections::BTreeMap;

use super::{lift, source, text};
use crate::db::{AcadDb, Data, Object, Section, Value};
use crate::{DwgFile, DxfFile};
use zumen_base::{Cx, Diagnostic, Diagnostics, Ref, codes};
use zumen_drawing::{Changes, Edge, Edit, Element, Length, ModelPoint, Polyline, Shape, SpaceId};
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
        db.object_mut(index).ok_or(Error::Rejected)?.values = values;
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

/// 置き換えで書く値。元の値を写し、変わった形の欄だけを直す。レイヤ・スタイルが同じで、形の種類が
/// 元のレコードで表せ、点と長さが元の要素と同じ空間にあるときだけ返す
fn replacement(
    original: &Element<ModelPoint>,
    element: &Element<ModelPoint>,
    object: &Object,
) -> Option<Vec<Value>> {
    if element.layer != original.layer || element.style != original.style {
        return None;
    }
    let space = space(&original.shape)?;
    let mut values = object.values.clone();
    if element.shape == original.shape {
        return Some(values);
    }
    match (&element.shape, &original.shape, object.kind.as_slice()) {
        (Shape::Line { start, end }, _, b"LINE") => {
            within(space, &[start, end])?;
            put(&mut values, 10, start.x, &[])?;
            put(&mut values, 20, start.y, &[])?;
            put(&mut values, 11, end.x, &[])?;
            put(&mut values, 21, end.y, &[])?;
        }
        (Shape::Circle { center, radius }, _, b"CIRCLE") => {
            within(space, &[center])?;
            put(&mut values, 10, center.x, &[])?;
            put(&mut values, 20, center.y, &[])?;
            put(&mut values, 40, positive(radius, space)?, &[])?;
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
            _,
            b"ARC",
        ) => {
            within(space, &[center])?;
            let (start, end) = turned(*start, *end, *counterclockwise);
            put(&mut values, 10, center.x, &[])?;
            put(&mut values, 20, center.y, &[])?;
            put(&mut values, 40, positive(radius, space)?, &[])?;
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
            within(space, &[center, major])?;
            ellipse(
                &mut values,
                [center, major, was_center, was_major],
                *ratio,
                turned(*start, *end, *counterclockwise),
                (*was_start, *was_end),
            )?;
        }
        (Shape::Polyline(line), _, b"LWPOLYLINE") => values = lwpolyline(&values, line, space)?,
        _ => return None,
    }
    Some(values)
}

/// ELLIPSE（DXF Reference「ELLIPSE」）の欄を直す。長軸の端（11・21）は中心からの向き、始まりと終わりの
/// パラメーター（41・42）はラジアンで持つ。読むときに度へ直した値と比べ、変わった欄だけを書く。
/// 直し戻した値が元の値と最後の桁で違って、変えていない欄まで書き換えないようにするため
fn ellipse(
    values: &mut Vec<Value>,
    [center, major, was_center, was_major]: [&ModelPoint; 4],
    ratio: f64,
    (start, end): (f64, f64),
    (was_start, was_end): (f64, f64),
) -> Option<()> {
    if !(ratio > 0.0 && ratio <= 1.0) {
        return None;
    }
    if center != was_center {
        put(values, 10, center.x, &[])?;
        put(values, 20, center.y, &[])?;
    }
    if center != was_center || major != was_major {
        let axis = [major.x - center.x, major.y - center.y];
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
fn lwpolyline(values: &[Value], line: &Polyline<ModelPoint>, space: SpaceId) -> Option<Vec<Value>> {
    let vertex = |code: i16| matches!(code, 10 | 20 | 40 | 41 | 42 | 91);
    let first = values.iter().position(|v| v.code == 10)?;
    let last = values.iter().rposition(|v| vertex(v.code))?;
    let body = values.get(first..=last)?;
    if body.iter().any(|v| !vertex(v.code)) {
        return None;
    }
    let ids: Vec<&Value> = body.iter().filter(|v| v.code == 91).collect();
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
            let width = Data::Real(nonnegative(width, space)?);
            match out.iter_mut().find(|v| v.code == 43) {
                Some(slot) => slot.data = width,
                None => insert(&mut out, 43, width, &[70, 90])?,
            }
        }
        None => out.retain(|v| v.code != 43),
    }
    for (index, corner) in line.vertices.iter().enumerate() {
        within(space, &[&corner.point])?;
        out.push(real(10, corner.point.x)?);
        out.push(real(20, corner.point.y)?);
        if let Some(id) = ids.get(index) {
            out.push((*id).clone());
        }
        if let Some([start, end]) = line.vertex_widths.get(index) {
            out.push(real(40, nonnegative(start, space)?)?);
            out.push(real(41, nonnegative(end, space)?)?);
        }
        if corner.bulge != 0.0 {
            out.push(real(42, corner.bulge)?);
        }
    }
    out.extend_from_slice(values.get(last + 1..)?);
    Some(out)
}

/// 時計回りの弧を、同じ弧をたどる反時計回りの始まりと終わりに直す
fn turned(start: f64, end: f64, counterclockwise: bool) -> (f64, f64) {
    if counterclockwise {
        (start, end)
    } else {
        (end, start)
    }
}

/// 点がすべて `space` にあるか
fn within(space: SpaceId, points: &[&ModelPoint]) -> Option<()> {
    points.iter().all(|p| p.space == space).then_some(())
}

/// `space` の実寸の長さで、0 より大きいもの
fn positive(length: &Length, space: SpaceId) -> Option<f64> {
    nonnegative(length, space).filter(|value| *value > 0.0)
}

/// `space` の実寸の長さで、0 以上のもの
fn nonnegative(length: &Length, space: SpaceId) -> Option<f64> {
    match length {
        Length::Model(length) if length.space == space && length.value >= 0.0 => {
            Some(length.value).filter(|value| value.is_finite())
        }
        _ => None,
    }
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

/// 置き換えられる形の空間
fn space(shape: &Shape<ModelPoint>) -> Option<SpaceId> {
    match shape {
        Shape::Line { start, .. } => Some(start.space),
        Shape::Circle { center, .. } => Some(center.space),
        Shape::Curve(Edge::Arc { center, .. } | Edge::Elliptic { center, .. }) => {
            Some(center.space)
        }
        Shape::Polyline(line) => line.vertices.first().map(|v| v.point.space),
        _ => None,
    }
}
