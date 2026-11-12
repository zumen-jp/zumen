//! 引出線つきの注記（MULTILEADER）の置き換えと変換、および形の基準点。

use super::*;

/// MULTILEADER（DXF Reference「MLEADER」）の枝を直す（決定 0185）。枝の線の頂点・集まる点・折れ曲がりと、文字の
/// 注記の内容（決定 0191）だけを書き、ほかのフィールド（注記の位置とスタイル・線の引き方・端部記号・大きさ・色・スタイル）は
/// 元と同じでなければならない。枝と線の数と、集まる点と折れ曲がりの有無も元と同じとする。セクション（302 `LEADER{`・
/// 304 `LEADER_LINE{`）の中の、枝の 10・20（集まる点）・11・21（折れ曲がりの向き）・40（折れ曲がりの長さ）と、線の
/// 10・20・30（頂点）を書き直す。線の切れ目（線の 11・12、枝の 12・13）は頂点から決め直せないので、それを持つ線か
/// 枝を変えるときは書かない。文字の内容は文脈の 304 に 1 つの値で書くので、250 バイトまでとする（長い内容の
/// 分け方を確かめていない）
pub(super) fn calling<P: PartialEq>(
    values: &mut Vec<Value>,
    callout: &Callout<P>,
    was: &Callout<P>,
    frame: &impl Frame<P>,
    encoder: &Encoder,
) -> Option<()> {
    let content = match (&callout.content, &was.content) {
        (content, before) if content == before => None,
        (
            zumen_drawing::CalloutContent::Text(text),
            zumen_drawing::CalloutContent::Text(before),
        ) => {
            let laid = text.height == before.height
                && text.point == before.point
                && text.horizontal == before.horizontal
                && text.vertical == before.vertical
                && text.rotation == before.rotation
                && text.width == before.width
                && text.frame_height == before.frame_height
                && text.flow == before.flow
                && text.line_spacing == before.line_spacing
                && text.background == before.background
                && text.style == before.style;
            let spans = mtext::spans(&text.content, &|value| frame.lifted(value));
            let encoded = encoder.encode(&text.content)?;
            if !laid
                || text.spans != spans
                || text.content.contains(['\n', '\r'])
                || encoded.len() > 250
            {
                return None;
            }
            Some(encoded)
        }
        _ => return None,
    };
    let mut wrote = content.is_none();
    if callout.path != was.path
        || callout.arrowhead != was.arrowhead
        || callout.arrow_size != was.arrow_size
        || callout.landing != was.landing
        || callout.scale != was.scale
        || callout.line_style != was.line_style
        || callout.content_color != was.content_color
        || callout.style != was.style
        || callout.branches.len() != was.branches.len()
    {
        return None;
    }
    for (branch, old) in callout.branches.iter().zip(&was.branches) {
        if branch.lines.len() != old.lines.len()
            || branch.end.is_some() != old.end.is_some()
            || branch.dogleg.is_some() != old.dogleg.is_some()
            || branch.lines.iter().any(Vec::is_empty)
        {
            return None;
        }
    }
    let marker =
        |value: &Value, text: &[u8]| matches!(&value.data, Data::Text(t) if t.as_slice() == text);
    // 0 は図形、1 は文脈、2 は枝、3 は線の中（読む側の `split_callout` と同じセクション）
    let mut depth = 0;
    let mut seen = false;
    let mut branches = 0_usize;
    let mut lines = 0_usize;
    let mut written = false;
    let old = std::mem::take(values);
    for value in old {
        let branch = branches
            .checked_sub(1)
            .and_then(|at| Some((callout.branches.get(at)?, was.branches.get(at)?)));
        let line = branch.and_then(|(branch, old)| {
            let at = lines.checked_sub(1)?;
            Some((branch.lines.get(at)?, old.lines.get(at)?))
        });
        match (depth, value.code) {
            (0, 300) if !seen => {
                depth = 1;
                seen = true;
            }
            (1, 301) => depth = 0,
            (1, 304) if let Some(text) = &content => {
                values.push(Value {
                    code: 304,
                    data: Data::Text(text.clone()),
                });
                wrote = true;
                continue;
            }
            (1, 302) if marker(&value, b"LEADER{") => {
                branches += 1;
                lines = 0;
                depth = 2;
            }
            (2, 303) => depth = 1,
            (2, 304) if marker(&value, b"LEADER_LINE{") => {
                lines += 1;
                written = false;
                depth = 3;
            }
            (3, 305) => {
                if line.is_some_and(|(line, old)| line != old) && !written {
                    return None;
                }
                depth = 2;
            }
            (2, code) => {
                let (branch, old) = branch?;
                if branch.end != old.end || branch.dogleg != old.dogleg {
                    let replaced = match (code, &branch.end, &branch.dogleg) {
                        (12 | 13, _, _) => return None,
                        (10, Some(end), _) => Some(real(10, frame.point(end)?[0])?),
                        (20, Some(end), _) => Some(real(20, frame.point(end)?[1])?),
                        (11, _, Some(dogleg)) => Some(real(11, dogleg.direction[0])?),
                        (21, _, Some(dogleg)) => Some(real(21, dogleg.direction[1])?),
                        (40, _, Some(dogleg)) => Some(real(40, positive(frame, &dogleg.length)?)?),
                        _ => None,
                    };
                    if let Some(replaced) = replaced {
                        values.push(replaced);
                        continue;
                    }
                }
            }
            (3, code) => {
                let (line, old) = line?;
                if line != old {
                    match code {
                        11 | 12 => return None,
                        10 if !written => {
                            for point in line {
                                let [x, y] = frame.point(point)?;
                                values.extend([real(10, x)?, real(20, y)?, real(30, 0.0)?]);
                            }
                            written = true;
                            continue;
                        }
                        10 | 20 | 30 => continue,
                        _ => {}
                    }
                }
            }
            _ => {}
        }
        values.push(value);
    }
    (seen && wrote && depth == 0 && branches == callout.branches.len()).then_some(())
}

/// 変換 `transform`（レコードの座標を写す、尺度を変えず鏡映を含まない相似）で動かした MULTILEADER（DXF
/// Reference「MLEADER」）の値（決定 0191）。セクションは [`calling`] と同じ。文脈の注記の基点（10）・文字の位置（12）・
/// 平面の原点（110）と、枝の集まる点（10）・切れ目（12・13）と、線の頂点（10）・切れ目（11・12）を点として写し、
/// 文字の向き（13）・平面の X と Y の軸（111・112）・折れ曲がりの向き（枝の 11）を回し、文字の回転（42、ラジアン）
/// に回転の角度を足す。大きさは変えない。図形のフィールド（セクションの外）は、点を持たないので変えない
pub(super) fn swung(values: &[Value], transform: &Affine) -> Option<Vec<Value>> {
    enum Kind {
        Point,
        Vector,
        Direction,
    }
    let s = transform.similarity()?;
    let mut values = values.to_vec();
    let marker =
        |value: &Value, text: &[u8]| matches!(&value.data, Data::Text(t) if t.as_slice() == text);
    let pair = |values: &[Value], at: usize, code: i16| -> Option<[f64; 2]> {
        match (values.get(at)?, values.get(at + 1)?) {
            (
                Value {
                    data: Data::Real(x),
                    ..
                },
                Value {
                    code: next,
                    data: Data::Real(y),
                },
            ) if *next == code + 10 => Some([*x, *y]),
            _ => None,
        }
    };
    let mut depth = 0;
    let mut seen = false;
    let mut at = 0;
    while let Some(value) = values.get(at) {
        let code = value.code;
        let kind = match (depth, code) {
            (0, 300) if !seen => {
                depth = 1;
                seen = true;
                None
            }
            (1, 301) => {
                depth = 0;
                None
            }
            (1, 302) if marker(value, b"LEADER{") => {
                depth = 2;
                None
            }
            (2, 303) => {
                depth = 1;
                None
            }
            (2, 304) if marker(value, b"LEADER_LINE{") => {
                depth = 3;
                None
            }
            (3, 305) => {
                depth = 2;
                None
            }
            (1, 42) => {
                let Data::Real(angle) = value.data else {
                    return None;
                };
                let turned = (angle.to_degrees() + s.rotation).rem_euclid(360.0);
                values.get_mut(at)?.data = Data::Real(turned.to_radians());
                None
            }
            (1, 10 | 12 | 110) | (2, 10 | 12 | 13) | (3, 10..=12) => Some(Kind::Point),
            (1, 13 | 111 | 112) => Some(Kind::Vector),
            (2, 11) => Some(Kind::Direction),
            _ => None,
        };
        if let Some(kind) = kind {
            let [x, y] = pair(&values, at, code)?;
            let [x, y] = match kind {
                Kind::Point => {
                    let (x, y) = transform.apply(x, y);
                    [x, y]
                }
                Kind::Vector => transform.vector([x, y]),
                Kind::Direction => transform.direction([x, y]),
            };
            values.get_mut(at)?.data = Data::Real(x);
            values.get_mut(at + 1)?.data = Data::Real(y);
            at += 1;
        }
        at += 1;
    }
    (seen && depth == 0).then_some(values)
}

/// 置き換えられる形の初めの点。形の空間（用紙）を決める
pub(super) fn anchor<P: Copy>(shape: &Shape<P>) -> Option<P> {
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
        Shape::Point(dot) => Some(dot.position),
        Shape::Curve(Edge::Spline(spline)) => {
            spline.control.first().or(spline.fit.first()).copied()
        }
        Shape::Leader(leader) => leader.path.first().copied(),
        Shape::Callout(callout) => callout.branches.iter().find_map(|branch| {
            branch
                .lines
                .iter()
                .find_map(|line| line.first())
                .or(branch.end.as_ref())
                .copied()
        }),
        _ => None,
    }
}
