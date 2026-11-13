//! 変更・取り消し・やり直し。
//!
//! 動かした後のレコードの値は、今の実装の `apply`（`crates/zumen-acad/src/drawing/apply/replace.rs` と
//! `apply/transform.rs`）と同じ規則で作り、タグの差し替えは、書き戻し（`dxf/write_back.rs`）と同じ規則で決める。
//! 文書の全体は読み直さない。変えるレコードのタグを直し、その行の列・表示・箱・索引だけを作り直す。

use zumen_acad::{Data, Value};
use zumen_drawing::transform::transform as transformed;
use zumen_drawing::{Affine, Edge, Length, ModelPoint, Placement, Polyline, Shape, SpaceId, Text};

use crate::lift::{self, Geom, Kind, Lift, POLY_WIDTHS, Pools, RawTag, TextRow, class};
use crate::scan::Span;
use crate::store::{
    Charset, Document, Locked, OTag, Shapes, Variable, bounds, range_of, tag_value, tokenize,
};
use crate::value::{value, value_bytes};

/// 要素を動かす変更
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Edit {
    /// 要素のハンドル
    pub handle: u64,
    /// 変換の行列（x' = m0·x + m1·y + m2、y' = m3·x + m4·y + m5）
    pub matrix: [f64; 6],
}

/// 変更を受け付けなかった理由。受け付けなければ、文書は何も変わらない
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Decline {
    /// 文書が変更を受け付けない
    Locked(Locked),
    /// ハンドルの要素がない
    Unknown,
    /// 同じ要素を 2 度指した
    Repeated,
    /// 種類を保って動かせないか、レコードに書けない（今の実装も拒む）
    Unsupported,
    /// 書いたレコードを、要素として読み直せない
    Unreadable,
}

/// レコードのタグに行う操作。`at` は、レコードの中のタグの番号
#[derive(Clone, Debug, PartialEq)]
enum Planned {
    /// 値を置き換える
    Set { at: usize, new: Box<[u8]> },
    /// タグを除く
    Remove { at: usize },
    /// タグを足す
    Insert {
        at: usize,
        code: i16,
        value: Box<[u8]>,
    },
}

/// 行った操作。取り消しとやり直しに使う
#[derive(Clone, Debug, PartialEq)]
enum TagOp {
    /// 値を置き換えた。`old` は、前に変えていた値
    Set {
        at: usize,
        old: Option<Box<[u8]>>,
        new: Box<[u8]>,
    },
    /// タグを足した
    Insert {
        at: usize,
        code: i16,
        value: Box<[u8]>,
    },
    /// タグを除いた
    Remove { at: usize, tag: OTag },
}

/// 1 つのレコードへの変更
#[derive(Clone, Debug, PartialEq)]
struct Change {
    record: u32,
    /// 作り直す行。HEADER のレコードでは `None`
    row: Option<u32>,
    /// この変更で、レコードをタグの列に変えたか
    created: bool,
    ops: Vec<TagOp>,
}

/// 取り消せる 1 回の変更
#[derive(Clone, Debug, PartialEq)]
pub struct Step {
    changes: Vec<Change>,
}

/// 1 つの要素について決めた変更
struct Plan {
    row: u32,
    record: u32,
    ops: Vec<Planned>,
    /// 値が変わる線と円の、動かした後の範囲。図面の範囲（`$EXTMIN`・`$EXTMAX`）を広げるのに使う
    bounds: Option<[[f64; 3]; 2]>,
}

/// 値の行に書けないバイトがあるか。今の実装のトランザクションも、改行を含む値を拒む
fn has_newline(value: &[u8]) -> bool {
    value.iter().any(|&b| b == b'\r' || b == b'\n')
}

/// 足すタグ
fn fresh(code: i16, value: Box<[u8]>) -> OTag {
    OTag {
        code,
        span: Span {
            code,
            start: 0,
            value: 0,
            value_end: 0,
            end: 0,
        },
        fresh: true,
        edited: Some(value),
    }
}

/// 操作を行う。今の実装のトランザクションと同じく、レコードの最初のタグ・ないタグ・グループ・コード 0 は扱わない
fn perform(tags: &mut Vec<OTag>, op: Planned) -> Option<TagOp> {
    match op {
        Planned::Set { at, new } => {
            if at == 0 || has_newline(&new) {
                return None;
            }
            let tag = tags.get_mut(at).filter(|tag| tag.code != 0)?;
            let old = tag.edited.replace(new.clone());
            Some(TagOp::Set { at, old, new })
        }
        Planned::Remove { at } => {
            if at == 0 || at >= tags.len() {
                return None;
            }
            Some(TagOp::Remove {
                at,
                tag: tags.remove(at),
            })
        }
        Planned::Insert { at, code, value } => {
            if at == 0 || at > tags.len() || code == 0 || has_newline(&value) {
                return None;
            }
            tags.insert(at, fresh(code, value.clone()));
            Some(TagOp::Insert { at, code, value })
        }
    }
}

/// 行った操作を、もう一度行う
fn forward(tags: &mut Vec<OTag>, op: &TagOp) {
    match op {
        TagOp::Set { at, new, .. } => {
            if let Some(tag) = tags.get_mut(*at) {
                tag.edited = Some(new.clone());
            }
        }
        TagOp::Insert { at, code, value } => {
            if *at <= tags.len() {
                tags.insert(*at, fresh(*code, value.clone()));
            }
        }
        TagOp::Remove { at, .. } => {
            if *at < tags.len() {
                tags.remove(*at);
            }
        }
    }
}

/// 行った操作を、元に戻す
fn backward(tags: &mut Vec<OTag>, op: &TagOp) {
    match op {
        TagOp::Set { at, old, .. } => {
            if let Some(tag) = tags.get_mut(*at) {
                tag.edited = old.clone();
            }
        }
        TagOp::Insert { at, .. } => {
            if *at < tags.len() {
                tags.remove(*at);
            }
        }
        TagOp::Remove { at, tag } => {
            if *at <= tags.len() {
                tags.insert(*at, tag.clone());
            }
        }
    }
}

const MODEL: SpaceId = SpaceId(0);

/// モデル空間の点の座標。今の実装と同じく、ほかの空間の点は扱わない
fn at(point: &ModelPoint) -> Option<[f64; 2]> {
    (point.space == MODEL).then_some([point.x, point.y])
}

/// モデル空間の長さ
fn length_of(length: &Length) -> Option<f64> {
    match length {
        Length::Model(length) if length.space == MODEL => Some(length.value),
        _ => None,
    }
}

fn nonnegative(length: &Length) -> Option<f64> {
    length_of(length).filter(|value| value.is_finite() && *value >= 0.0)
}

fn positive(length: &Length) -> Option<f64> {
    nonnegative(length).filter(|value| *value > 0.0)
}

/// 有限の実数の値
fn real(code: i16, value: f64) -> Option<Value> {
    value.is_finite().then_some(Value {
        code,
        data: Data::Real(value),
    })
}

/// `after` のうち、最初に見つかったグループ・コードの値の後ろに足す
fn insert(values: &mut Vec<Value>, code: i16, data: Data, after: &[i16]) -> Option<()> {
    let at = after
        .iter()
        .find_map(|after| values.iter().position(|v| v.code == *after))?;
    values.insert(at + 1, Value { code, data });
    Some(())
}

/// 最初の値を置き換える。なければ足す
fn put(values: &mut Vec<Value>, code: i16, value: f64, after: &[i16]) -> Option<()> {
    let value = real(code, value)?;
    match values.iter_mut().find(|v| v.code == code) {
        Some(slot) => slot.data = value.data,
        None => insert(values, code, value.data, after)?,
    }
    Some(())
}

/// 省ける値。既定の値で、元にもなければ書かない
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

fn first_real(values: &[Value], code: i16) -> Option<f64> {
    values.iter().find_map(|v| match v.data {
        Data::Real(value) if v.code == code && value.is_finite() => Some(value),
        _ => None,
    })
}

/// ARC は反時計回りで持つ。時計回りの弧は、始まりと終わりを入れ替える
fn turned(start: f64, end: f64, counterclockwise: bool) -> (f64, f64) {
    if counterclockwise {
        (start, end)
    } else {
        (end, start)
    }
}

/// 点が変わっていれば書く
fn place(
    values: &mut Vec<Value>,
    [x, y]: [i16; 2],
    point: &ModelPoint,
    was: &ModelPoint,
) -> Option<()> {
    let [now_x, now_y] = at(point)?;
    if point != was {
        put(values, x, now_x, &[])?;
        put(values, y, now_y, &[])?;
    }
    Some(())
}

/// 動かした後のレコードの値。今の実装の `moved` と同じ順に直す
fn moved(
    was: &Shape<ModelPoint>,
    old: &[Value],
    kind: Kind,
    transform: &Affine,
) -> Option<Vec<Value>> {
    // 今の実装は、基準の点を取れない形（頂点のない折れ線）を拒む
    if let Shape::Polyline(line) = was
        && line.vertices.is_empty()
    {
        return None;
    }
    let shape = transformed(was, transform)?;
    let mut values = replacement(was, &shape, old, kind)?;
    if let (Shape::Text(before), Shape::Text(after)) = (was, &shape) {
        aligned_first(&mut values, old, before, after, transform)?;
    }
    if moving_xdata(old) {
        carried(&mut values, transform)?;
    }
    Some(values)
}

/// 形を置き換えた後のレコードの値。形が変わらなければ、元の値のまま
fn replacement(
    was: &Shape<ModelPoint>,
    now: &Shape<ModelPoint>,
    old: &[Value],
    kind: Kind,
) -> Option<Vec<Value>> {
    let mut values = old.to_vec();
    if now == was {
        return Some(values);
    }
    match (now, was, kind) {
        (
            Shape::Line { start, end },
            Shape::Line {
                start: was_start,
                end: was_end,
            },
            Kind::Line,
        ) => {
            place(&mut values, [10, 20], start, was_start)?;
            place(&mut values, [11, 21], end, was_end)?;
        }
        (
            Shape::Circle { center, radius },
            Shape::Circle {
                center: was_center,
                radius: was_radius,
            },
            Kind::Circle,
        ) => {
            place(&mut values, [10, 20], center, was_center)?;
            if radius != was_radius {
                put(&mut values, 40, positive(radius)?, &[])?;
            }
        }
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
            Kind::Arc,
        ) => {
            place(&mut values, [10, 20], center, was_center)?;
            let (start, end) = turned(*start, *end, *counterclockwise);
            if radius != was_radius {
                put(&mut values, 40, positive(radius)?, &[])?;
            }
            put(&mut values, 50, start, &[])?;
            put(&mut values, 51, end, &[])?;
        }
        (Shape::Polyline(line), Shape::Polyline(was), Kind::Polyline) => {
            values = lwpolyline(&values, line, was)?;
        }
        (Shape::Text(text), Shape::Text(was), Kind::Text) => {
            lettering(&mut values, text, was)?;
        }
        _ => return None,
    }
    Some(values)
}

/// LWPOLYLINE のフラグ（70）の、閉じている（1）と線種の生成（128）
fn polyline_flags(line: &Polyline<ModelPoint>) -> i64 {
    i64::from(line.closed) | i64::from(line.line_type_generation) << 7
}

/// LWPOLYLINE の値を組み直す。頂点の前の値は写して、頂点の数（90）・フラグ（70）・一定の幅（43）を直し、
/// 頂点ごとに 10・20、頂点の ID（91）、頂点ごとの幅（40・41）、0 でない膨らみ（42）を並べる。頂点の後の値は写す
fn lwpolyline(
    values: &[Value],
    line: &Polyline<ModelPoint>,
    was: &Polyline<ModelPoint>,
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
    let flags = polyline_flags(line);
    match out.iter_mut().find(|v| v.code == 70) {
        Some(Value {
            data: Data::Int(old),
            ..
        }) => *old = (*old & !(1 | 128)) | flags,
        Some(_) => return None,
        None if flags != 0 => insert(&mut out, 70, Data::Int(flags), &[90])?,
        None => {}
    }
    match &line.width {
        Some(width) => {
            let width = Data::Real(nonnegative(width)?);
            match out.iter_mut().find(|v| v.code == 43) {
                Some(slot) => slot.data = width,
                None => insert(&mut out, 43, width, &[70, 90])?,
            }
        }
        None => out.retain(|v| v.code != 43),
    }
    for (index, corner) in line.vertices.iter().enumerate() {
        let [x, y] = at(&corner.point)?;
        let kept = was.vertices.get(index).map(|v| &v.point) == Some(&corner.point);
        match (kept, xs.get(index), ys.get(index)) {
            (true, Some(x), Some(y)) => out.extend([(*x).clone(), (*y).clone()]),
            _ => out.extend([real(10, x)?, real(20, y)?]),
        }
        if let Some(id) = ids.get(index) {
            out.push((*id).clone());
        }
        if let Some([start, end]) = line.vertex_widths.get(index) {
            out.push(real(40, nonnegative(start)?)?);
            out.push(real(41, nonnegative(end)?)?);
        }
        if corner.bulge != 0.0 {
            out.push(real(42, corner.bulge)?);
        }
    }
    out.extend_from_slice(values.get(last + 1..)?);
    Some(out)
}

/// TEXT の値を直す。左の基準線に置く文字は 10 を、ほかの位置合わせの文字は 11 を基点とし、10 は 11 と同じだけ動かす。
/// 位置合わせ（72・73）・2 点の間への収め方・文字スタイル・高さの意味は変えられない
fn lettering(
    values: &mut Vec<Value>,
    text: &Text<ModelPoint>,
    was: &Text<ModelPoint>,
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
            let now = at(point)?;
            let before = at(was_point)?;
            let aligned = [72, 73].iter().any(|code| {
                values
                    .iter()
                    .any(|v| v.code == *code && v.data != Data::Int(0))
            });
            if point != was_point && aligned {
                let first = [first_real(values, 10)?, first_real(values, 20)?];
                put(values, 10, first[0] + (now[0] - before[0]), &[])?;
                put(values, 20, first[1] + (now[1] - before[1]), &[])?;
                put(values, 11, now[0], &[])?;
                put(values, 21, now[1], &[])?;
            } else if point != was_point {
                put(values, 10, now[0], &[])?;
                put(values, 20, now[1], &[])?;
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
            place(values, [10, 20], first, was_first)?;
            place(values, [11, 21], second, was_second)?;
        }
        _ => return None,
    }
    // 動かすだけなら、中身は変わらない。中身を書く変更は、この試作にない
    if text.content != was.content {
        return None;
    }
    if text.height.value != was.height.value {
        put(values, 40, positive(&text.height.value)?, &[])?;
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

/// 位置合わせされた文字の、基点からの相対の位置を保つ第 1 点も変換する
fn aligned_first(
    values: &mut Vec<Value>,
    original: &[Value],
    before: &Text<ModelPoint>,
    after: &Text<ModelPoint>,
    transform: &Affine,
) -> Option<()> {
    let aligned = [72, 73].iter().any(|code| {
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
    let [old_x, old_y] = at(old)?;
    let [new_x, new_y] = at(new)?;
    let [x, y] = transform.vector([
        first_real(original, 10)? - old_x,
        first_real(original, 20)? - old_y,
    ]);
    put(values, 10, new_x + x, &[])?;
    put(values, 20, new_y + y, &[])?;
    Some(())
}

/// 図形とともに動く XDATA（点と、距離・倍率）を持つか
fn moving_xdata(values: &[Value]) -> bool {
    let Some(start) = values.iter().position(|v| v.code == 1001) else {
        return false;
    };
    values
        .iter()
        .skip(start)
        .any(|v| matches!(v.code, 1011..=1013 | 1041 | 1042))
}

/// 図形とともに動く XDATA を直す。位置（1011・1021）は変換で写し、変位（1012・1022）は平行移動を除いて写し、
/// 向き（1013・1023）は写してから元の長さにそろえる。距離（1041）と倍率（1042）は尺度を掛け、相似でない変換では直さない
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

/// 値の並び `old` を `new` にする操作を `out` に足す。今の実装の書き戻しと同じく、前と後ろの同じ値を除き、
/// 残りのグループ・コードの並びが同じなら違う値だけを置き換え、違えば残りを除いて足し直す。
/// `offset` は、`old` の最初の値のタグの番号
fn diff(offset: usize, old: &[Value], new: &[Value], out: &mut Vec<Planned>) -> Option<()> {
    let prefix = old.iter().zip(new).take_while(|(a, b)| a == b).count();
    let (old, new) = (
        old.get(prefix..).unwrap_or_default(),
        new.get(prefix..).unwrap_or_default(),
    );
    let suffix = old
        .iter()
        .rev()
        .zip(new.iter().rev())
        .take_while(|(a, b)| a == b)
        .count();
    let old = old.get(..old.len() - suffix).unwrap_or_default();
    let new = new.get(..new.len() - suffix).unwrap_or_default();
    let at = offset + prefix;
    let same_codes = old.len() == new.len() && old.iter().zip(new).all(|(a, b)| a.code == b.code);
    if same_codes {
        for (index, (a, b)) in old.iter().zip(new).enumerate() {
            if a != b {
                out.push(Planned::Set {
                    at: at + index,
                    new: value_bytes(&b.data)?.into(),
                });
            }
        }
        return Some(());
    }
    for index in (0..old.len()).rev() {
        out.push(Planned::Remove { at: at + index });
    }
    for (index, value) in new.iter().enumerate() {
        out.push(Planned::Insert {
            at: at + index,
            code: value.code,
            value: value_bytes(&value.data)?.into(),
        });
    }
    Some(())
}

/// グループ・コード `code`・`code + 10`・`code + 20` の点。Z は、なければ 0
fn point(values: &[Value], code: i16) -> Option<[f64; 3]> {
    let real = |code: i16| {
        values
            .iter()
            .find(|v| v.code == code)
            .map(|v| match v.data {
                Data::Real(value) if value.is_finite() => Some(value),
                _ => None,
            })
    };
    Some([
        real(code)??,
        real(code + 10)??,
        real(code + 20).unwrap_or(Some(0.0))?,
    ])
}

fn min3(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0].min(b[0]), a[1].min(b[1]), a[2].min(b[2])]
}

fn max3(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0].max(b[0]), a[1].max(b[1]), a[2].max(b[2])]
}

/// 図面の範囲に足す、図形の範囲。今の実装と同じく、線と、押し出しの向きが既定の円だけが範囲を持つ。
/// 行は ENTITIES のセクションのモデル空間の図形なので、セクションと空間は見ない
fn model_bounds(kind: Kind, values: &[Value]) -> Option<[[f64; 3]; 2]> {
    let corners = |a: [f64; 3], b: [f64; 3]| Some([min3(a, b), max3(a, b)]);
    match kind {
        Kind::Line => corners(point(values, 10)?, point(values, 11)?),
        Kind::Circle => {
            let normal = if values.iter().any(|v| v.code == 210) {
                point(values, 210)?
            } else {
                [0.0, 0.0, 1.0]
            };
            if normal != [0.0, 0.0, 1.0] {
                return None;
            }
            let [x, y, z] = point(values, 10)?;
            let radius = match values.iter().find(|v| v.code == 40)?.data {
                Data::Real(radius) => radius.abs(),
                _ => return None,
            };
            corners([x - radius, y - radius, z], [x + radius, y + radius, z])
        }
        Kind::Arc | Kind::Polyline | Kind::Text => None,
    }
}

/// 読み直した頂点（と頂点ごとの幅）を、同じ場所に書く。場所がなければ、何も変えずに `false`
fn overwrite(
    pools: &mut Pools,
    first: u32,
    vertices: &[[f64; 3]],
    widths: Option<&[[f64; 2]]>,
) -> bool {
    let range = first as usize..first as usize + vertices.len();
    if pools.vertices.get(range.clone()).is_none() {
        return false;
    }
    if let Some(widths) = widths {
        let place = pools.widths_mut(first, vertices.len() as u32);
        if place.len() != widths.len() {
            return false;
        }
        place.copy_from_slice(widths);
    }
    if let Some(place) = pools.vertices.get_mut(range) {
        place.copy_from_slice(vertices);
    }
    true
}

/// 読み直した形を、行の形の場所に置く。置いた形（折れ線と文字は、文書の置き場を指すもの）を返す。
/// 動かすだけなら、頂点の数も文字の中身も変わらないので、置き場は増えない
fn store_geom(shapes: &mut Shapes, at: u32, geom: &Geom, scratch: &Pools) -> Option<Geom> {
    let at = at as usize;
    Some(match *geom {
        Geom::Line(line) => {
            *shapes.lines.get_mut(at)? = line;
            Geom::Line(line)
        }
        Geom::Circle(circle) => {
            *shapes.circles.get_mut(at)? = circle;
            Geom::Circle(circle)
        }
        Geom::Arc(arc) => {
            *shapes.arcs.get_mut(at)? = arc;
            Geom::Arc(arc)
        }
        Geom::Polyline {
            first,
            count,
            flags,
            width,
        } => {
            let vertices = scratch
                .vertices
                .get(first as usize..first as usize + count as usize)?;
            let wide = flags & POLY_WIDTHS != 0;
            let widths = scratch.widths_of(first, count);
            if wide && widths.len() != vertices.len() {
                return None;
            }
            let pools = &mut shapes.pools;
            let poly = shapes.polys.get_mut(at)?;
            let same = poly.count == count && (poly.flags & POLY_WIDTHS != 0) == wide;
            if !(same && overwrite(pools, poly.first, vertices, wide.then_some(widths))) {
                // 頂点の数か、幅を持つかが変わった。後ろに置き直す（前の場所は使わなくなる）
                let start = u32::try_from(pools.vertices.len()).ok()?;
                let width_start = u32::try_from(pools.widths.len()).ok()?;
                pools.vertices.extend_from_slice(vertices);
                if wide {
                    pools.width_first.push((start, width_start));
                    pools.widths.extend_from_slice(widths);
                }
                poly.first = start;
            }
            poly.count = count;
            poly.flags = flags;
            poly.width = width;
            Geom::Polyline {
                first: poly.first,
                count,
                flags,
                width,
            }
        }
        Geom::Text(text) => {
            let old = *shapes.texts.get(at)?;
            let pools = &mut shapes.pools;
            let content = scratch.text_of(text.content);
            let content = if pools.text_of(old.content) == content {
                old.content
            } else {
                pools.push_text(content)?
            };
            let style = match (text.style, old.style) {
                (None, _) => None,
                (Some(range), Some(kept)) if pools.text_of(kept) == scratch.text_of(range) => {
                    Some(kept)
                }
                (Some(range), _) => Some(pools.push_text(scratch.text_of(range))?),
            };
            let stored = TextRow {
                content,
                style,
                ..text
            };
            *shapes.texts.get_mut(at)? = stored;
            Geom::Text(stored)
        }
    })
}

impl Document {
    /// 要素を動かす。変更は、すべて受け付けるか、すべて受け付けないかのどちらか。
    /// 受け付けた 1 回の変更は、1 回の取り消しで戻る
    pub fn apply(&mut self, edits: &[Edit]) -> Result<(), Decline> {
        if let Some(locked) = self.locked {
            return Err(Decline::Locked(locked));
        }
        // 先に、すべての変更を決める。ここでは文書を変えない
        let mut plans = Vec::with_capacity(edits.len());
        for edit in edits {
            let row = self.row_of(edit.handle).ok_or(Decline::Unknown)?;
            let transform = Affine { m: edit.matrix };
            plans.push(self.plan(row, &transform).ok_or(Decline::Unsupported)?);
        }
        // ファイルの順に並べる。今の実装も、変えたレコードをファイルの順に見て図面の範囲を広げる
        plans.sort_by_key(|plan| plan.row);
        if plans
            .windows(2)
            .any(|pair| pair.first().map(|plan| plan.row) == pair.get(1).map(|plan| plan.row))
        {
            return Err(Decline::Repeated);
        }
        let header = self.extents(&plans)?;

        let mut step = Step {
            changes: Vec::with_capacity(plans.len() + 1),
        };
        let mut scratch = Pools::default();
        let mut failed = None;
        for plan in plans {
            if plan.ops.is_empty() {
                continue;
            }
            let Some(change) = self.rewrite(plan.record, Some(plan.row), plan.ops) else {
                failed = Some(Decline::Unsupported);
                break;
            };
            step.changes.push(change);
            if !self.refresh_row(plan.row, &mut scratch) {
                failed = Some(Decline::Unreadable);
                break;
            }
        }
        if failed.is_none()
            && let Some((record, ops)) = header
        {
            match self.rewrite(record, None, ops) {
                Some(change) => step.changes.push(change),
                None => failed = Some(Decline::Unsupported),
            }
        }
        if let Some(decline) = failed {
            self.revert(&step);
            return Err(decline);
        }
        self.undo.push(step);
        self.redo.clear();
        Ok(())
    }

    /// 直前の変更を取り消す。取り消す変更がなければ `false`
    pub fn undo(&mut self) -> bool {
        let Some(step) = self.undo.pop() else {
            return false;
        };
        self.revert(&step);
        self.redo.push(step);
        true
    }

    /// 取り消した変更をやり直す。やり直す変更がなければ `false`
    pub fn redo(&mut self) -> bool {
        let Some(step) = self.redo.pop() else {
            return false;
        };
        self.replay(&step);
        self.undo.push(step);
        true
    }

    /// 変えたレコードのタグと、取り消し・やり直しの列の、およそのバイト数
    pub fn edit_bytes(&self) -> usize {
        let held = |value: &Option<Box<[u8]>>| value.as_ref().map_or(0, |value| value.len());
        let overlay: usize = self
            .overlay
            .values()
            .flatten()
            .map(|tag| core::mem::size_of::<OTag>() + held(&tag.edited))
            .sum();
        let op = |op: &TagOp| {
            core::mem::size_of::<TagOp>()
                + match op {
                    TagOp::Set { old, new, .. } => held(old) + new.len(),
                    TagOp::Insert { value, .. } => value.len(),
                    TagOp::Remove { tag, .. } => held(&tag.edited),
                }
        };
        let steps = |steps: &[Step]| -> usize {
            steps
                .iter()
                .flat_map(|step| &step.changes)
                .map(|change| {
                    core::mem::size_of::<Change>() + change.ops.iter().map(op).sum::<usize>()
                })
                .sum()
        };
        overlay + steps(&self.undo) + steps(&self.redo)
    }

    /// レコードの、グループ・コード 0 の次からの値
    fn values(&self, record: u32) -> Vec<Value> {
        let read = |tags: &[OTag]| -> Vec<Value> {
            tags.iter()
                .skip(1)
                .map(|tag| value(tag.code, tag_value(&self.src, tag)))
                .collect()
        };
        match self.overlay.get(&record) {
            Some(tags) => read(tags),
            None => read(&tokenize(&self.src, self.record_range(record))),
        }
    }

    /// ヘッダーの変数の、今の値
    fn variable_values(&self, record: u32, variable: &Variable) -> Vec<Value> {
        match self.overlay.get(&record) {
            Some(tags) => tags
                .iter()
                .skip(variable.start)
                .take(variable.values.len())
                .map(|tag| value(tag.code, tag_value(&self.src, tag)))
                .collect(),
            None => variable.values.clone(),
        }
    }

    /// 行を `transform` で動かす変更を決める。動かせなければ `None`
    fn plan(&self, at: u32, transform: &Affine) -> Option<Plan> {
        let row = self.rows.get(at as usize)?;
        let old = self.values(row.record);
        let geom = self.shapes.geom(row.kind, row.geom)?;
        let was = lift::shape(&geom, &self.shapes.pools)?;
        let new = moved(&was, &old, row.kind, transform)?;
        let mut ops = Vec::new();
        diff(1, &old, &new, &mut ops)?;
        let bounds = if new == old {
            None
        } else {
            model_bounds(row.kind, &new)
        };
        Some(Plan {
            row: at,
            record: row.record,
            ops,
            bounds,
        })
    }

    /// 図面の範囲（`$EXTMIN`・`$EXTMAX`）を、値が変わった線と円を含むように広げる操作。狭めることはない
    fn extents(&self, plans: &[Plan]) -> Result<Option<(u32, Vec<Planned>)>, Decline> {
        let Some(header) = &self.header else {
            return Ok(None);
        };
        let found = plans
            .iter()
            .filter_map(|plan| plan.bounds)
            .reduce(|[a0, a1], [b0, b1]| [min3(a0, b0), max3(a1, b1)]);
        let Some([low, high]) = found else {
            return Ok(None);
        };
        type Pick = fn([f64; 3], [f64; 3]) -> [f64; 3];
        let corners: [(&Option<Variable>, [f64; 3], Pick); 2] =
            [(&header.extmin, low, min3), (&header.extmax, high, max3)];
        let mut ops = Vec::new();
        for (variable, corner, pick) in corners {
            let Some(variable) = variable else {
                continue;
            };
            let values = self.variable_values(header.record, variable);
            let Some(old) = point(&values, 10) else {
                continue;
            };
            let new = pick(old, corner);
            if new == old {
                continue;
            }
            let mut fixed = values.clone();
            for value in &mut fixed {
                if let Some(axis) = [10, 20, 30].iter().position(|&code| code == value.code)
                    && let Some(&coordinate) = new.get(axis)
                {
                    value.data = Data::Real(coordinate);
                }
            }
            diff(variable.start, &values, &fixed, &mut ops).ok_or(Decline::Unsupported)?;
        }
        Ok((!ops.is_empty()).then_some((header.record, ops)))
    }

    /// レコードのタグに操作を行う。1 つでも行えなければ、行った分を戻して `None`
    fn rewrite(&mut self, record: u32, row: Option<u32>, ops: Vec<Planned>) -> Option<Change> {
        let created = !self.overlay.contains_key(&record);
        let range = range_of(&self.starts, record);
        let src = &self.src;
        let tags = self
            .overlay
            .entry(record)
            .or_insert_with(|| tokenize(src, range));
        let mut done = Vec::with_capacity(ops.len());
        for op in ops {
            let Some(op) = perform(tags, op) else {
                for op in done.iter().rev() {
                    backward(tags, op);
                }
                if created {
                    self.overlay.remove(&record);
                }
                return None;
            };
            done.push(op);
        }
        Some(Change {
            record,
            row,
            created,
            ops: done,
        })
    }

    /// 変更を、後ろから元に戻す
    fn revert(&mut self, step: &Step) {
        let mut scratch = Pools::default();
        for change in step.changes.iter().rev() {
            if let Some(tags) = self.overlay.get_mut(&change.record) {
                for op in change.ops.iter().rev() {
                    backward(tags, op);
                }
            }
            if change.created {
                self.overlay.remove(&change.record);
            }
            if let Some(row) = change.row {
                self.refresh_row(row, &mut scratch);
            }
        }
    }

    /// 変更を、前からもう一度行う
    fn replay(&mut self, step: &Step) {
        let mut scratch = Pools::default();
        for change in &step.changes {
            let range = range_of(&self.starts, change.record);
            let src = &self.src;
            let tags = self
                .overlay
                .entry(change.record)
                .or_insert_with(|| tokenize(src, range));
            for op in &change.ops {
                forward(tags, op);
            }
            if let Some(row) = change.row {
                self.refresh_row(row, &mut scratch);
            }
        }
    }

    /// 行の列・表示・箱・索引を、レコードの今のタグから作り直す。開くときと同じ読み方（`lift`）を通すので、
    /// 開き直した文書と同じ値になる。要素として読めなければ、何も変えずに `false` を返す
    fn refresh_row(&mut self, at: u32, scratch: &mut Pools) -> bool {
        let Document {
            src,
            starts,
            rows,
            shapes,
            layers,
            charset,
            overlay,
            display,
            boxes,
            grid,
            ..
        } = self;
        let src: &[u8] = src;
        let Some(row) = rows.get_mut(at as usize) else {
            return false;
        };
        let pristine;
        let tags: &[OTag] = match overlay.get(&row.record) {
            Some(tags) => tags,
            None => {
                pristine = tokenize(src, range_of(starts, row.record));
                &pristine
            }
        };
        let raw: Vec<RawTag<'_>> = tags
            .iter()
            .skip(1)
            .map(|tag| RawTag {
                code: tag.code,
                value: tag_value(src, tag),
            })
            .collect();
        scratch.clear();
        let Lift::Element(lifted) = lift::lift(row.kind, &raw, scratch) else {
            return false;
        };
        let readable = lifted.class == class::ASCII
            || (lifted.class == class::UTF8 && *charset == Charset::Utf8);
        if !readable || lifted.handle.unwrap_or(0) != row.handle {
            return false;
        }
        let Some(geom) = store_geom(shapes, row.geom, &lifted.geom, scratch) else {
            return false;
        };
        let same_layer = layers
            .list
            .get(row.layer as usize)
            .is_some_and(|layer| layer.name.eq_ignore_ascii_case(lifted.layer));
        if !same_layer {
            row.layer = layers.intern(lifted.layer);
        }
        row.color = lifted.color;
        row.class = lifted.class;
        display.update(at, &geom, &shapes.pools, [row.layer, row.color]);
        if let Some(place) = boxes.get_mut(at as usize) {
            *place = bounds(&shapes.pools, &geom);
            grid.update(at, *place);
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// ハンドルを持つ最小の DXF。`entities` は、ENTITIES のセクションのレコード
    fn dxf(entities: &str) -> Vec<u8> {
        let head = "  0\nSECTION\n  2\nHEADER\n  9\n$ACADVER\n  1\nAC1015\n  9\n$HANDSEED\n  5\nFFFF\n\
                    \x20 9\n$EXTMIN\n 10\n0.0\n 20\n0.0\n 30\n0.0\n  9\n$EXTMAX\n 10\n10.0\n 20\n10.0\n 30\n0.0\n\
                    \x20 0\nENDSEC\n  0\nSECTION\n  2\nENTITIES\n";
        format!("{head}{entities}  0\nENDSEC\n  0\nEOF\n").into_bytes()
    }

    fn open(entities: &str) -> Document {
        Document::open(dxf(entities), 1).expect("開く")
    }

    fn text(bytes: &[u8]) -> String {
        String::from_utf8(bytes.to_vec()).expect("ASCII")
    }

    const LINE: &str =
        "  0\nLINE\n  5\n2A\n  8\n0\n 10\n1.0\n 20\n2.0\n 30\n0.0\n 11\n3.0\n 21\n4.0\n 31\n0.0\n";

    fn translation(handle: u64, dx: f64, dy: f64) -> Edit {
        Edit {
            handle,
            matrix: [1.0, 0.0, dx, 0.0, 1.0, dy],
        }
    }

    #[test]
    fn moving_a_line_rewrites_its_points_and_widens_the_extents() {
        let mut document = open(LINE);
        let before = document.save();
        assert_eq!(before, dxf(LINE));
        assert_eq!(document.apply(&[translation(0x2A, 10.0, 0.5)]), Ok(()));
        let after = text(&document.save());
        assert!(
            after.contains(" 10\n11.0\n 20\n2.5\n 30\n0.0\n 11\n13.0\n 21\n4.5\n"),
            "{after}"
        );
        // 範囲は、広がる向きの値だけが変わる
        assert!(after.contains("$EXTMIN\n 10\n0.0\n 20\n0.0\n"), "{after}");
        assert!(after.contains("$EXTMAX\n 10\n13.0\n 20\n10.0\n"), "{after}");
        assert_eq!(document.geom(0), Some(Geom::Line([11.0, 2.5, 13.0, 4.5])));
        assert_eq!(document.boxes, [[11.0, 2.5, 13.0, 4.5]]);
        // 取り消すと、元のバイト列と列に戻る
        assert!(document.undo());
        assert_eq!(document.save(), before);
        assert_eq!(document.geom(0), Some(Geom::Line([1.0, 2.0, 3.0, 4.0])));
        assert_eq!(document.edited_records(), 0);
        assert!(!document.undo());
        assert!(document.redo());
        assert_eq!(text(&document.save()), after);
        assert!(!document.redo());
    }

    #[test]
    fn the_edited_document_equals_a_reopened_one() {
        let entities = format!(
            "{LINE}  0\nCIRCLE\n  5\n2B\n  8\nA\n 10\n5.0\n 20\n5.0\n 40\n1.5\n\
             \x20 0\nARC\n  5\n2C\n  8\nA\n 10\n5.0\n 20\n5.0\n 40\n2.0\n 50\n0.0\n 51\n90.0\n\
             \x20 0\nLWPOLYLINE\n  5\n2D\n  8\n0\n 90\n3\n 70\n1\n 43\n0.0\n 10\n0.0\n 20\n0.0\n 10\n4.0\n 20\n0.0\n 42\n0.5\n 10\n4.0\n 20\n3.0\n\
             \x20 0\nTEXT\n  5\n2E\n  8\n0\n 10\n1.0\n 20\n1.0\n 40\n0.5\n  1\nabc\n 72\n1\n 11\n2.0\n 21\n1.0\n"
        );
        let mut document = open(&entities);
        assert_eq!(document.rows.len(), 5);
        let quarter = Affine::rotation(1.0, 1.0, 90.0).m;
        let edits: Vec<Edit> = [0x2A, 0x2B, 0x2C, 0x2D, 0x2E]
            .into_iter()
            .map(|handle| Edit {
                handle,
                matrix: quarter,
            })
            .collect();
        assert_eq!(document.apply(&edits), Ok(()));
        assert_eq!(document.apply(&[translation(0x2D, 0.25, 0.0)]), Ok(()));
        let saved = document.save();
        let reopened = Document::open(saved.clone(), 1).expect("開き直す");
        assert!(document.same_content(&reopened));
        assert_eq!(document.display.canonical(), reopened.display.canonical());
        assert_eq!(document.boxes, reopened.boxes);
        // 一定の幅が 0 の折れ線は、43 を除いて頂点を書き直す
        let saved = text(&saved);
        assert!(!saved.contains(" 43\n"), "{saved}");
        assert!(document.undo());
        assert!(document.undo());
        assert_eq!(document.save(), dxf(&entities));
    }

    #[test]
    fn declined_edits_change_nothing() {
        let entities =
            format!("{LINE}  0\nCIRCLE\n  5\n2B\n  8\n0\n 10\n5.0\n 20\n5.0\n 40\n1.5\n");
        let mut document = open(&entities);
        let before = document.save();
        assert_eq!(
            document.apply(&[translation(0x99, 1.0, 0.0)]),
            Err(Decline::Unknown)
        );
        assert_eq!(
            document.apply(&[translation(0x2A, 1.0, 0.0), translation(0x2A, 2.0, 0.0)]),
            Err(Decline::Repeated)
        );
        // 逆のない変換と、円を楕円にする変換は、種類を保てない。まとめて受け付けない
        let flat = Edit {
            handle: 0x2A,
            matrix: [1.0, 0.0, 0.0, 0.0, 0.0, 0.0],
        };
        assert_eq!(document.apply(&[flat]), Err(Decline::Unsupported));
        let stretch = Edit {
            handle: 0x2B,
            matrix: [2.0, 0.0, 0.0, 0.0, 1.0, 0.0],
        };
        assert_eq!(
            document.apply(&[translation(0x2A, 1.0, 0.0), stretch]),
            Err(Decline::Unsupported)
        );
        assert_eq!(document.save(), before);
        assert_eq!(document.undo_len(), 0);
        // 線は、相似でない変換でも動かせる
        let line = Edit {
            handle: 0x2A,
            ..stretch
        };
        assert_eq!(document.apply(&[line]), Ok(()));
        assert_eq!(document.geom(0), Some(Geom::Line([2.0, 2.0, 6.0, 4.0])));
    }

    #[test]
    fn a_seed_not_above_the_largest_handle_locks_the_document() {
        let entities = LINE.replace("2A", "FFFF");
        let mut document = open(&entities);
        assert_eq!(document.locked, Some(Locked::HandleSeed));
        assert_eq!(
            document.apply(&[translation(0xFFFF, 1.0, 0.0)]),
            Err(Decline::Locked(Locked::HandleSeed))
        );
    }
}
