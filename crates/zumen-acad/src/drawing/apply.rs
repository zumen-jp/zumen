//! 図面モデルの変更を、共有の表を通して DXF・DWG に書き戻す（決定 0139）。

use std::collections::BTreeMap;

use super::{lift, mtext, source, text};
use crate::db::{AcadDb, Data, Object, Section, Value};
use crate::{DwgFile, DxfFile};
use zumen_base::{Cx, Diagnostic, Diagnostics, Ref, codes};
use zumen_drawing::{
    Changes, Edge, Edit, Element, FormattedText, Horizontal, Length, ModelLength, ModelPoint,
    Placement, Polyline, Rotation, Shape, SpaceId, Symbol, Text, Vertical,
};
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
    // 2007 からの版は文字を UTF-8 で持つ。それより前は、ASCII の文字だけを書く（読む側と同じ）
    let unicode = db
        .variable(b"$ACADVER")
        .and_then(|values| match values.first().map(|v| &v.data) {
            Some(Data::Text(text)) => Some(text.trim_ascii() >= super::UTF8),
            _ => None,
        })
        .unwrap_or(false);
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
        if let Edit::Replace(element) = change
            && element.shape != original.shape
            && let Some(code) = moving_xdata(object)
        {
            cx.report(
                Diagnostic::new(codes::DXF_XDATA_UNSUPPORTED)
                    .with("kind", text(&object.kind))
                    .with("code", i64::from(code))
                    .at(at.clone()),
            );
            rejected = true;
            continue;
        }
        let planned = match change {
            Edit::Replace(element) => replacement(original, element, object, unicode)
                .map(|values| replaced.push((index, values))),
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
fn replacement(
    original: &Element<ModelPoint>,
    element: &Element<ModelPoint>,
    object: &Object,
    unicode: bool,
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
        (Shape::Text(text), Shape::Text(was), b"TEXT") => {
            lettering(&mut values, text, was, space, unicode)?;
        }
        (Shape::Formatted(text), Shape::Formatted(was), b"MTEXT") => {
            formatting(&mut values, text, was, space, unicode)?;
        }
        (Shape::Symbol(symbol), Shape::Symbol(was), b"INSERT") => {
            placing(&mut values, symbol, was, space)?;
        }
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

/// TEXT（DXF Reference「TEXT」）の欄を直す。内容（1）・高さ（40）・回転（50）・幅の倍率（41）・斜体の角度（51）・
/// 反転（71 の 2 と 4）と、置く点を書く。位置合わせ（72・73）・2 点の間への合わせ方・文字スタイル・高さの意味は
/// 変えられない。左の基準線に置く文字は 10 を、ほかの位置合わせの文字は 11 を基点とし、10 は 11 と同じだけ動かす。
/// 10 は位置合わせから決まる点で、読み直すときに使わないが、ほかのソフトが読むので食い違わせない
fn lettering(
    values: &mut Vec<Value>,
    text: &Text<ModelPoint>,
    was: &Text<ModelPoint>,
    space: SpaceId,
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
            within(space, &[point])?;
            let aligned = [72, 73].iter().any(|code| {
                values
                    .iter()
                    .any(|v| v.code == *code && v.data != Data::Int(0))
            });
            if point != was_point && aligned {
                let first = [first_real(values, 10)?, first_real(values, 20)?];
                put(values, 10, first[0] + (point.x - was_point.x), &[])?;
                put(values, 20, first[1] + (point.y - was_point.y), &[])?;
                put(values, 11, point.x, &[])?;
                put(values, 21, point.y, &[])?;
            } else if point != was_point {
                put(values, 10, point.x, &[])?;
                put(values, 20, point.y, &[])?;
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
            within(space, &[first, second])?;
            if first != was_first {
                put(values, 10, first.x, &[])?;
                put(values, 20, first.y, &[])?;
            }
            if second != was_second {
                put(values, 11, second.x, &[])?;
                put(values, 21, second.y, &[])?;
            }
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
        put(values, 40, positive(&text.height.value, space)?, &[])?;
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
fn formatting(
    values: &mut Vec<Value>,
    text: &FormattedText<ModelPoint>,
    was: &FormattedText<ModelPoint>,
    space: SpaceId,
    unicode: bool,
) -> Option<()> {
    if text.style != was.style
        || text.flow != was.flow
        || text.background != was.background
        || text.height.meaning != was.height.meaning
    {
        return None;
    }
    let length = |value: f64| Length::Model(ModelLength { space, value });
    if text.spans != mtext::spans(&text.content, &length) {
        return None;
    }
    within(space, &[&text.point])?;
    if text.point != was.point {
        put(values, 10, text.point.x, &[])?;
        put(values, 20, text.point.y, &[])?;
    }
    if text.content != was.content {
        contents(values, &text.content, unicode)?;
    }
    if text.height.value != was.height.value {
        put(values, 40, positive(&text.height.value, space)?, &[])?;
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
    for (code, frame, was_frame) in [
        (41, &text.width, &was.width),
        (46, &text.frame_height, &was.frame_height),
    ] {
        if frame != was_frame {
            let size = match frame {
                Some(size) => positive(size, space)?,
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
fn placing(
    values: &mut Vec<Value>,
    symbol: &Symbol<ModelPoint>,
    was: &Symbol<ModelPoint>,
    space: SpaceId,
) -> Option<()> {
    if symbol.key != was.key
        || symbol.grid != was.grid
        || symbol.attributes != was.attributes
        || symbol.clip != was.clip
    {
        return None;
    }
    within(space, &[&symbol.position])?;
    if symbol.position != was.position {
        put(values, 10, symbol.position.x, &[])?;
        put(values, 20, symbol.position.y, &[])?;
    }
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
        Shape::Text(text) => match &text.placement {
            Placement::Anchor { point, .. } => Some(point.space),
            Placement::Between { first, .. } => Some(first.space),
        },
        Shape::Formatted(text) => Some(text.point.space),
        Shape::Symbol(symbol) => Some(symbol.position.space),
        _ => None,
    }
}
