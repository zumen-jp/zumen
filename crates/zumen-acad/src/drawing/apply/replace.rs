//! 図形の置き換えで変わる形のフィールド（線・円弧・楕円・ポリライン・スプライン・文字・挿入・引出線・点）の書き直し。

use super::*;

/// 置き換えで書く値。元の値を写し、変わった形のフィールドだけを直す。レイヤ・スタイルが同じで、形の種類が
/// 元のレコードで表せ、点と長さが元の要素と同じ空間にあるときだけ返す
pub(super) fn replacement<P: PartialEq>(
    original: &Element<P>,
    element: &Element<P>,
    object: &Object,
    encoder: &Encoder,
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
        (Shape::Curve(Edge::Spline(spline)), Shape::Curve(Edge::Spline(was)), b"SPLINE") => {
            splining(&mut values, spline, was, frame)?;
        }
        (Shape::Polyline(line), Shape::Polyline(was), b"LWPOLYLINE") => {
            values = lwpolyline(&values, line, was, frame)?;
        }
        (Shape::Text(text), Shape::Text(was), b"TEXT") => {
            lettering(&mut values, text, was, frame, encoder, 73)?;
        }
        (Shape::Formatted(text), Shape::Formatted(was), b"MTEXT") => {
            formatting(&mut values, text, was, frame, encoder)?;
        }
        (Shape::Symbol(symbol), Shape::Symbol(was), b"INSERT") => {
            placing(&mut values, symbol, was, frame)?;
        }
        (Shape::Leader(leader), Shape::Leader(was), b"LEADER") => {
            leading(&mut values, leader, was, frame)?;
        }
        (Shape::Point(dot), Shape::Point(was), b"POINT") => {
            dotting(&mut values, dot, was, frame)?;
        }
        (Shape::Callout(callout), Shape::Callout(was), b"MULTILEADER") => {
            calling(&mut values, callout, was, frame, encoder)?;
        }
        _ => return None,
    }
    Some(values)
}

/// POINT（DXF Reference「POINT」）の位置（10・20）と X 軸の角度（50）を直す。見え方は文書の点の表し方
/// （`$PDMODE`・`$PDSIZE`）が決めるので、既定義のマーカーの点と作図の補助の点は書かない（決定 0183）。角度は、省けば
/// 0 なので、フィールドの並び（10・20・30・39・210・220・230・50）に合わせて書く
fn dotting<P: PartialEq>(
    values: &mut Vec<Value>,
    dot: &zumen_drawing::Point<P>,
    was: &zumen_drawing::Point<P>,
    frame: &impl Frame<P>,
) -> Option<()> {
    if dot.look != zumen_drawing::PointLook::Document || dot.construction {
        return None;
    }
    place(values, frame, [10, 20], &dot.position, &was.position)?;
    if dot.rotation != was.rotation {
        optional(values, 50, dot.rotation, 0.0, &[230, 220, 210, 39, 30, 20])?;
    }
    Some(())
}

/// LEADER（DXF Reference「LEADER」）のフィールドを直す。寸法スタイル（3）と注記（340）は変えない。経路が変われば、
/// 頂点の数（76）の後の頂点（10・20・30）を書き直し、曲線か（72）と矢印（71）は変わったものだけを書く。矢印を
/// 寸法スタイルに従わせるときは 71 を除く。注記の大きさと鉤の線（40・41・74・75）と、ずれ（212・213）は元のまま
/// （変換では [`directed`] がずれを写す）
fn leading<P: PartialEq>(
    values: &mut Vec<Value>,
    leader: &Leader<P>,
    was: &Leader<P>,
    frame: &impl Frame<P>,
) -> Option<()> {
    if leader.style != was.style || leader.annotation != was.annotation {
        return None;
    }
    if leader.path != was.path {
        let mut vertices = Vec::with_capacity(leader.path.len().saturating_mul(3));
        for point in &leader.path {
            let [x, y] = frame.point(point)?;
            vertices.extend([real(10, x)?, real(20, y)?, real(30, 0.0)?]);
        }
        // LEADER の 10・20・30 は頂点だけが持つ
        values.retain(|v| !matches!(v.code, 10 | 20 | 30));
        let count = values.iter().position(|v| v.code == 76)?;
        values.get_mut(count)?.data = Data::Int(i64::try_from(leader.path.len()).ok()?);
        values.splice(count + 1..count + 1, vertices);
    }
    // 矢印（71）は寸法スタイル（3）の後に、曲線か（72）は矢印の後に書く。どちらもなければ、図形のサブクラスマーカー（100）の
    // 最後の後に書く（DXF Reference の並び）
    let after = |values: &[Value], codes: &[i16]| {
        codes
            .iter()
            .find_map(|code| values.iter().position(|v| v.code == *code))
            .or_else(|| values.iter().rposition(|v| v.code == 100))
    };
    if leader.arrow != was.arrow {
        values.retain(|v| v.code != 71);
        if let Some(arrow) = leader.arrow {
            let at = after(values, &[3])?;
            values.insert(
                at + 1,
                Value {
                    code: 71,
                    data: Data::Int(i64::from(arrow)),
                },
            );
        }
    }
    if leader.spline != was.spline {
        let flag = Data::Int(i64::from(leader.spline));
        match values.iter_mut().find(|v| v.code == 72) {
            Some(slot) => slot.data = flag,
            None => {
                let at = after(values, &[71, 3])?;
                values.insert(
                    at + 1,
                    Value {
                        code: 72,
                        data: flag,
                    },
                );
            }
        }
    }
    Some(())
}

/// ELLIPSE（DXF Reference「ELLIPSE」）のフィールドを直す。長軸の端（11・21）は中心からの向き、始まりと終わりの
/// パラメーター（41・42）はラジアンで持つ。読むときに度へ直した値と比べ、変わったフィールドだけを書く。
/// 直し戻した値が元の値と最後の桁で違って、変えていないフィールドまで書き換えないようにするため
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
/// いること（70 の 1）・線種の生成（70 の 128）・一定の幅（43）を直し、頂点ごとに 10・20、頂点の ID（91）、頂点ごとの幅（40・41）、
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
        }) => *flags = (*flags & !(1 | 128)) | polyline_flags(line),
        Some(_) => return None,
        None if polyline_flags(line) != 0 => {
            insert(&mut out, 70, Data::Int(polyline_flags(line)), &[90])?;
        }
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

/// LWPOLYLINE・POLYLINE のフラグ（70）の、閉じている（1）と線種の生成（128）
pub(super) fn polyline_flags<P>(line: &Polyline<P>) -> i64 {
    i64::from(line.closed) | i64::from(line.line_type_generation) << 7
}

/// TEXT（DXF Reference「TEXT」）のフィールドを直す。内容（1）・高さ（40）・回転（50）・幅の倍率（41）・斜体の角度（51）・
/// 反転（71 の 2 と 4）と、置く点を書く。位置合わせ（72・73）・2 点の間への合わせ方・文字スタイル・高さの意味は
/// 変えられない。左の基準線に置く文字は 10 を、ほかの位置合わせの文字は 11 を基点とし、10 は 11 と同じだけ動かす。
/// 10 は位置合わせから決まる点で、読み直すときに使わないが、ほかのソフトが読むので食い違わせない
pub(super) fn lettering<P: PartialEq>(
    values: &mut Vec<Value>,
    text: &Text<P>,
    was: &Text<P>,
    frame: &impl Frame<P>,
    encoder: &Encoder,
    vertical_code: i16,
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
            let aligned = [72, vertical_code].iter().any(|code| {
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
        if content.contains(['\n', '\r']) {
            return None;
        }
        let slot = values.iter_mut().find(|v| v.code == 1)?;
        slot.data = Data::Text(encoder.encode(content)?);
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

/// MTEXT（DXF Reference「MTEXT」）のフィールドを直す。内容（250 バイトずつの 3 と、最後の 1）・基点（10）・高さ（40）・
/// 向き（50 のラジアンか、11・21 の向き）・枠の幅（41）と高さ（46）・位置合わせ（71）・行送り（44・73）を書く。
/// 文字スタイル・書き進める向き・背景・高さの意味は変えられない。向きは、元と同じ形（角度か向き）のときだけ
/// 書く。省いたフィールドは、DXF Reference の並び（内容の後に 50・73・44）で足す。区間は内容から決まるので、内容を解釈し直した区間と合わなければ受け付けない
fn formatting<P: PartialEq>(
    values: &mut Vec<Value>,
    text: &FormattedText<P>,
    was: &FormattedText<P>,
    frame: &impl Frame<P>,
    encoder: &Encoder,
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
        contents(values, &text.content, encoder)?;
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

/// INSERT（DXF Reference「INSERT」）の置く点（10・20）・尺度（41・42）・回転（50、度）と、格子の間隔
/// （44・45）を直す。定義（2）・格子の列と行の数（70・71）・属性・切り抜きは変えられない。属性と切り抜きは、
/// 続くレコードと拡張辞書にあるため。格子の有無と数を変えると、INSERT と MINSERT の型が変わるため
fn placing<P: PartialEq>(
    values: &mut Vec<Value>,
    symbol: &Symbol<P>,
    was: &Symbol<P>,
    frame: &impl Frame<P>,
) -> Option<()> {
    let spacing = match (&symbol.grid, &was.grid) {
        (None, None) => None,
        (Some(grid), Some(original))
            if (grid.columns, grid.rows) == (original.columns, original.rows) =>
        {
            Some((grid, original))
        }
        _ => return None,
    };
    if symbol.key != was.key || symbol.attributes != was.attributes || symbol.clip != was.clip {
        return None;
    }
    place(values, frame, [10, 20], &symbol.position, &was.position)?;
    // 尺度と回転は、省けば既定の値になる。変わったときだけ、フィールドの並び（10・20・30・41・42・43・50）に合わせて書く
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
    // 間隔は、変換で負になることもある（鏡映した行）。省けば 0 なので、フィールドの並び（70・71・44・45）に合わせて書く
    if let Some((grid, original)) = spacing {
        if grid.column_spacing != original.column_spacing {
            let value = frame
                .length(&grid.column_spacing)
                .filter(|v| v.is_finite())?;
            optional(values, 44, value, 0.0, &[71, 70, 50, 43, 42, 41, 30, 20])?;
        }
        if grid.row_spacing != original.row_spacing {
            let value = frame.length(&grid.row_spacing).filter(|v| v.is_finite())?;
            optional(
                values,
                45,
                value,
                0.0,
                &[44, 71, 70, 50, 43, 42, 41, 30, 20],
            )?;
        }
    }
    Some(())
}

/// SPLINE（DXF Reference「SPLINE」）の定義を書き直す。制御点で決まるものは、フラグ（70）の閉じている・周期的・有理
/// （1・2・4）と、次数（71）・ノットと制御点の数（72・73）・ノット（40）・制御点（10・20・30）・重み（41）を書く。
/// フィット点で決まるものは、ノットと制御点を書かず、フィット点の数（74）・フィット点（11・21・31）・
/// 接線（12・13）を書き、制御点は読む側が求め直す。ほかのフラグと許容差（42〜44）は元のまま。制御点で決まる
/// ものとフィット点で決まるものは、DWG の場合（scenario）が変わるので入れ替えない。フィット許容差と
/// ノットの取り方は、DWG の書き戻しが元のビットを写すので、変えるものは書き戻さない（決定 0157）
fn splining<P: PartialEq>(
    values: &mut Vec<Value>,
    spline: &Spline<P>,
    was: &Spline<P>,
    frame: &impl Frame<P>,
) -> Option<()> {
    let fitting = fitted(spline) && fitted(was);
    if !(fitting || controlled(spline) && controlled(was))
        || spline.fit_tolerance != was.fit_tolerance
        || spline.knot_parameterization != was.knot_parameterization
    {
        return None;
    }
    let defined = if fitting {
        fit_values(frame, spline)?
    } else {
        spline_values(frame, spline)?
    };
    let flags = values.iter().find_map(|v| match v.data {
        Data::Int(flags) if v.code == 70 => Some(flags),
        _ => None,
    });
    // DXF Reference「SPLINE」: 16 は線形を表す。変更後も線形かは、制御点だけでは
    // 保証できないため、線形のフラグを持つものの置き換えは拒否する。
    if flags.unwrap_or(0) & 16 != 0 {
        return None;
    }
    let mut counts = vec![
        (71, i64::from(spline.degree)),
        (72, i64::try_from(spline.knots.len()).ok()?),
        (73, i64::try_from(spline.control.len()).ok()?),
    ];
    // DWG のフィット点のスプラインは、閉じている・周期的のフラグを書かない。読んだ値にフラグがないか、R2013 からの
    // フィット点で決まるフラグ（1024）だけがあれば、フラグは直さず、閉じている・周期的は変えない
    let kept = fitting && flags.is_none_or(|flags| flags & 1024 != 0);
    if kept && (spline.closed, spline.periodic) != (was.closed, was.periodic) {
        return None;
    }
    if !kept {
        counts.insert(0, (70, spline_flags(flags.unwrap_or(8), spline)));
    }
    if fitting {
        counts.push((74, i64::try_from(spline.fit.len()).ok()?));
    }
    for (code, count) in counts {
        match values.iter_mut().find(|v| v.code == code) {
            Some(slot) => slot.data = Data::Int(count),
            None => insert(values, code, Data::Int(count), &[code - 1, 100])?,
        }
    }
    if fitting && !values.iter().any(|v| v.code == 44) {
        insert(values, 44, Data::Real(1e-10), &[43, 74])?;
    }
    let replaced = |code: i16| {
        matches!(code, 10 | 20 | 30 | 40 | 41)
            || fitting && matches!(code, 11 | 21 | 31 | 12 | 22 | 32 | 13 | 23 | 33)
    };
    let at = values.iter().position(|v| replaced(v.code))?;
    values.retain(|v| !replaced(v.code));
    values.splice(at..at, defined);
    Some(())
}

/// 制御点だけで決まるスプライン。フィット点も接線も持たない
pub(super) fn controlled<P>(spline: &Spline<P>) -> bool {
    !spline.control.is_empty()
        && spline.fit.is_empty()
        && spline.start_tangent.is_none()
        && spline.end_tangent.is_none()
}

/// フィット点だけで決まるスプライン。ノット・制御点・重みを持たず、フィット点を 2 つ以上持つ
pub(super) fn fitted<P>(spline: &Spline<P>) -> bool {
    spline.control.is_empty()
        && spline.knots.is_empty()
        && spline.weights.is_empty()
        && spline.fit.len() >= 2
        && spline.start_tangent != Some([0.0, 0.0])
        && spline.end_tangent != Some([0.0, 0.0])
}

/// SPLINE の接線（12・22・32、13・23・33）と、フィット点（11・21・31）の値。読む側が読めないスプラインは `None`
pub(super) fn fit_values<P>(frame: &impl Frame<P>, spline: &Spline<P>) -> Option<Vec<Value>> {
    crate::drawing::check_spline(b"SPLINE", spline).ok()?;
    let mut values = Vec::with_capacity(6 + spline.fit.len() * 3);
    for (code, tangent) in [(12, spline.start_tangent), (13, spline.end_tangent)] {
        if let Some([x, y]) = tangent {
            values.extend([real(code, x)?, real(code + 10, y)?, real(code + 20, 0.0)?]);
        }
    }
    for fit in &spline.fit {
        let [x, y] = frame.point(fit)?;
        values.extend([real(11, x)?, real(21, y)?, real(31, 0.0)?]);
    }
    Some(values)
}

/// SPLINE のフラグ（70）の、フィット点で決まること（1024）とノットの取り方（32・64・128・256）。取り方がなければ 0
pub(super) fn knot_flags<P>(spline: &Spline<P>) -> i64 {
    match spline.knot_parameterization {
        None => 0,
        Some(KnotParameterization::Chord) => 1024 | 32,
        Some(KnotParameterization::SquareRoot) => 1024 | 64,
        Some(KnotParameterization::Uniform) => 1024 | 128,
        Some(KnotParameterization::Custom) => 1024 | 256,
    }
}

/// SPLINE のフラグ（70）。`flags` の閉じている・周期的・有理（1・2・4）を、スプラインに合わせる
pub(super) fn spline_flags<P>(flags: i64, spline: &Spline<P>) -> i64 {
    flags & !7
        | i64::from(spline.closed)
        | i64::from(spline.periodic) << 1
        | i64::from(!spline.weights.is_empty()) << 2
}

/// SPLINE のノット（40）と、制御点（10・20・30）とその重み（41）の値。読む側が読めないスプラインは `None`
pub(super) fn spline_values<P>(frame: &impl Frame<P>, spline: &Spline<P>) -> Option<Vec<Value>> {
    crate::drawing::check_spline(b"SPLINE", spline).ok()?;
    let mut values = Vec::with_capacity(spline.knots.len() + spline.control.len() * 4);
    for knot in &spline.knots {
        values.push(real(40, *knot)?);
    }
    for (at, control) in spline.control.iter().enumerate() {
        let [x, y] = frame.point(control)?;
        values.extend([real(10, x)?, real(20, y)?, real(30, 0.0)?]);
        if let Some(weight) = spline.weights.get(at) {
            values.push(real(41, *weight)?);
        }
    }
    Some(values)
}

/// 0 でない有限の値
fn nonzero(value: f64) -> Option<f64> {
    (value.is_finite() && value != 0.0).then_some(value)
}

/// MTEXT の内容を、250 バイトずつの区切り（3）と最後の値（1）にして、元の内容の値の場所に置く。区切りは
/// 文字の途中と、制御文字の `^` の表記の途中で切らない。DXF の値は 1 行なので、改行は書けない（段落の区切りは `\P` で書く）。
/// 2007 より前のバージョンは、コード・ページで符号化したバイトで数える（決定 0192）
pub(super) fn contents(values: &mut Vec<Value>, content: &str, encoder: &Encoder) -> Option<()> {
    let chunks = split_text(content, 3, 1, encoder)?;
    let at = values.iter().position(|v| matches!(v.code, 1 | 3))?;
    values.retain(|v| !matches!(v.code, 1 | 3));
    values.splice(at..at, chunks);
    Some(())
}

/// 文字列 `content` を、250 バイトずつの区切り（`head`）と最後の値（`tail`）に分ける。文字の途中と、制御文字の
/// `^` の表記の途中で切らない。生の改行を含めば `None`
pub(super) fn split_text(
    content: &str,
    head: i16,
    tail: i16,
    encoder: &Encoder,
) -> Option<Vec<Value>> {
    if content.contains(['\n', '\r']) {
        return None;
    }
    // 符号化したバイト列と、文字の境目の位置
    let chars: Vec<char> = content.chars().collect();
    let mut bytes = Vec::with_capacity(content.len());
    let mut boundaries = vec![0];
    let mut buffer = [0; 4];
    for c in &chars {
        bytes.extend(encoder.encode(c.encode_utf8(&mut buffer))?);
        boundaries.push(bytes.len());
    }
    let mut chunks = Vec::new();
    // 区切りの始まりの、文字の位置
    let mut first = 0;
    while bytes.len() - boundaries.get(first)? > 250 {
        let start = *boundaries.get(first)?;
        // 1 文字は 1 バイト以上なので、251 文字より先は見ない
        let mut cut = (first + 1..boundaries.len().min(first + 251))
            .rev()
            .find(|&at| boundaries.get(at).is_some_and(|&end| end - start <= 250))?;
        // バイナリの DXF と DWG は、区切りごとに `^ ` や `^J` を戻す。末尾の `^` が対を持たなければ
        // 次の区切りへ送る。`^^` は 1 組の表記なので、その間では切らない。Shift_JIS の 2 バイト目の `^` は
        // 文字の一部なので数えない
        let carets = chars
            .get(first..cut)?
            .iter()
            .rev()
            .take_while(|&&c| c == '^')
            .count();
        if !carets.is_multiple_of(2) {
            cut = cut.checked_sub(1).filter(|&cut| cut > first)?;
        }
        chunks.push(Value {
            code: head,
            data: Data::Text(bytes.get(start..*boundaries.get(cut)?)?.to_vec()),
        });
        first = cut;
    }
    let start = *boundaries.get(first)?;
    chunks.push(Value {
        code: tail,
        data: Data::Text(bytes.get(start..)?.to_vec()),
    });
    Some(chunks)
}

/// `code` の最初の有限の実数
pub(super) fn first_real(values: &[Value], code: i16) -> Option<f64> {
    values.iter().find_map(|v| match v.data {
        Data::Real(value) if v.code == code && value.is_finite() => Some(value),
        _ => None,
    })
}

/// 省けば `default` になるフィールドを直す。フィールドがなく、値が `default` なら足さない
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
pub(super) fn turned(start: f64, end: f64, counterclockwise: bool) -> (f64, f64) {
    if counterclockwise {
        (start, end)
    } else {
        (end, start)
    }
}

/// 元の要素と同じ空間（用紙）の長さのレコードの値で、0 より大きいもの
pub(super) fn positive<P>(frame: &impl Frame<P>, length: &Length) -> Option<f64> {
    nonnegative(frame, length).filter(|value| *value > 0.0)
}

/// 元の要素と同じ空間（用紙）の長さのレコードの値で、0 以上のもの
pub(super) fn nonnegative<P>(frame: &impl Frame<P>, length: &Length) -> Option<f64> {
    frame
        .length(length)
        .filter(|value| value.is_finite() && *value >= 0.0)
}

/// 有限の実数の値
pub(super) fn real(code: i16, value: f64) -> Option<Value> {
    value.is_finite().then_some(Value {
        code,
        data: Data::Real(value),
    })
}

/// `code` の最初の値を `value` にする。なければ、`after` のうち最初に見つかったコードの値の後に足す
pub(super) fn put(values: &mut Vec<Value>, code: i16, value: f64, after: &[i16]) -> Option<()> {
    let value = real(code, value)?;
    match values.iter_mut().find(|v| v.code == code) {
        Some(slot) => slot.data = value.data,
        None => insert(values, code, value.data, after)?,
    }
    Some(())
}

/// `after` のうち最初に見つかったコードの値の後に、値を足す
pub(super) fn insert(values: &mut Vec<Value>, code: i16, data: Data, after: &[i16]) -> Option<()> {
    let at = after
        .iter()
        .find_map(|after| values.iter().position(|v| v.code == *after))?;
    values.insert(at + 1, Value { code, data });
    Some(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn linear_spline_is_not_replaced_with_an_unverified_curve() {
        let point = |x, y| ModelPoint { space: MODEL, x, y };
        let spline = Spline {
            degree: 2,
            knots: vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
            control: vec![point(0.0, 0.0), point(1.0, 0.0), point(2.0, 0.0)],
            weights: vec![],
            fit: vec![],
            start_tangent: None,
            end_tangent: None,
            closed: false,
            periodic: false,
            fit_tolerance: None,
            knot_parameterization: None,
        };
        let mut changed = spline.clone();
        changed.control[1].y = 1.0;
        let int = |code, value| Value {
            code,
            data: Data::Int(value),
        };
        let mut values = vec![
            int(70, 24),
            int(71, 2),
            int(72, 6),
            int(73, 3),
            real(40, 0.0).unwrap(),
        ];
        let before = values.clone();
        assert!(splining(&mut values, &changed, &spline, &InSpace(MODEL)).is_none());
        assert_eq!(values, before);
    }
}
