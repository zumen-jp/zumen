//! 図面モデルの変更を、JWW の図形の一覧に書き戻す（決定 0168）。
//!
//! スタイルと形の変更は、図形の共通の欄と形の値のバイトを、同じ大きさのまま書き直す。除くか足すときは、図形の一覧と
//! ブロックの定義の一覧を書き直し、件数と、オブジェクトの前の印（クラスの番号）を数え直す。見出しと画像の一覧は
//! 写す。当てた後に読み直して図面モデルに変換し、変えた要素が求めた値になり、ほかの要素の値が変わらないことを
//! 確かめる。合わなければ、何も変えずに断る。

use std::collections::{BTreeMap, BTreeSet};

use zumen_base::{Diagnostic, Kind, Ref, codes};
use zumen_drawing::{
    Applied, Changes, Color, ColorCode, ColorSystem, Drawing2D, Edge, Edit, Element, Hash, Inherit,
    LayerId, Length, LineTypeId, LineWeight, ModelLength, ModelPoint, PaperLength, Placed, Seen,
    Shape, SpaceId, Style, id, transform,
};
use zumen_sdk::{Apply, Codec, Cx, Error, Lift, quietly};

use super::{Header, PEN_WIDTH_SINCE, SXF_SINCE, TURN};
use crate::parse::Reader;
use crate::{Jww, JwwFile, RecordKind};

/// 診断に書く形式の名前
const FORMAT: &str = "JWW";

/// 線
const LINE: &[u8] = b"CDataSen";

/// 円・円弧・楕円・楕円弧
const ARC: &[u8] = b"CDataEnko";

/// 形の値の比べ方。実寸の値は、用紙の値に縮尺の分母を掛けて読むので、最後の桁が違いうる
const NEAR: f64 = 1e-9;

impl Apply<Changes> for JwwFile {
    /// 変更ごとの結果と、ほかの要素の ID がずれたか。JWW の要素は鍵を持たず、ID はレコードの番号で作る
    /// （決定 0156）。前の図形を除くか足すと、後ろの要素の ID がずれる
    type Output = (Vec<Applied>, bool);

    fn apply(&mut self, changes: &Changes, cx: &mut Cx) -> Result<(Vec<Applied>, bool), Error> {
        // 変えていないファイルは元のバイトのまま書くので、元のバイト列のハッシュと比べる
        let current = match changes.document {
            None => true,
            Some(hash) => {
                let same = Hash::document_checked(&self.src, cx).map_err(|stop| {
                    cx.report(stop.to_diagnostic());
                    Error::Stopped(stop)
                })? == hash;
                if !same {
                    cx.report(Diagnostic::new(codes::CHANGE_DOCUMENT_STALE));
                }
                same
            }
        };
        let header = self
            .records()
            .first()
            .filter(|record| record.kind() == RecordKind::Header)
            .and_then(|record| Header::read(self, record.span().end).ok());
        let Some(header) = header else {
            cx.report(unsupported("apply", "header"));
            return Err(Error::Rejected);
        };
        let drawing = quietly(cx, |cx| self.lift(cx))?;
        let before = Index::new(&drawing, cx)?;
        let mut plan = Plan {
            file: self,
            header: &header,
            patches: Vec::new(),
            removed: BTreeSet::new(),
            added: Vec::new(),
        };
        let mut expected = Vec::new();
        let mut targets = BTreeSet::new();
        let mut rejected = false;
        for change in &changes.edits {
            work(cx, 1)?;
            let result = match change {
                Edit::Remove { target } => {
                    let Some(found) = target_of(&before, target, &mut targets, "remove", cx)?
                    else {
                        rejected = true;
                        continue;
                    };
                    plan.remove(found.record)
                        .map(|()| Expected::Removed)
                        .or_else(|| {
                            cx.report(
                                unsupported("remove", &plan.kind(found.record)).at(found.source()),
                            );
                            None
                        })
                }
                Edit::Restyle { target, style } => {
                    let Some(found) = target_of(&before, target, &mut targets, "restyle", cx)?
                    else {
                        rejected = true;
                        continue;
                    };
                    let (layer, new) = style.over(found.element.layer, &found.element.style);
                    plan.styled(&found, layer, &new)
                        .map(|(layer, style)| {
                            Expected::Changed(
                                found.record,
                                layer,
                                style,
                                found.element.shape.clone(),
                            )
                        })
                        .or_else(|| {
                            cx.report(
                                unsupported("restyle", &plan.kind(found.record)).at(found.source()),
                            );
                            None
                        })
                }
                Edit::Replace { target, element } => {
                    let Some(found) = target_of(&before, target, &mut targets, "replace", cx)?
                    else {
                        rejected = true;
                        continue;
                    };
                    let replaced = match element.as_ref() {
                        Placed::Model(element) => plan
                            .styled(&found, element.layer, &element.style)
                            .and_then(|(layer, style)| {
                                plan.shaped(&found, &element.shape)?;
                                Some(Expected::Changed(
                                    found.record,
                                    layer,
                                    style,
                                    element.shape.clone(),
                                ))
                            }),
                        Placed::Paper(_) => None,
                    };
                    replaced.or_else(|| {
                        cx.report(
                            unsupported("replace", &plan.kind(found.record)).at(found.source()),
                        );
                        None
                    })
                }
                Edit::Transform { target, transform } => {
                    let Some(found) = target_of(&before, target, &mut targets, "transform", cx)?
                    else {
                        rejected = true;
                        continue;
                    };
                    transform::transform(&found.element.shape, transform)
                        .and_then(|shape| {
                            plan.shaped(&found, &shape)?;
                            Some(Expected::Changed(
                                found.record,
                                found.element.layer,
                                found.element.style,
                                shape,
                            ))
                        })
                        .or_else(|| {
                            cx.report(
                                unsupported("transform", &plan.kind(found.record))
                                    .at(found.source()),
                            );
                            None
                        })
                }
                Edit::Add { place, element } => {
                    let added = match element.as_ref() {
                        Placed::Model(element) => before
                            .spaces
                            .get(place)
                            .and_then(|space| plan.add(*space, element)),
                        Placed::Paper(_) => None,
                    };
                    added
                        .map(|(layer, style, shape)| {
                            Expected::Added(plan.added.len() - 1, layer, style, shape)
                        })
                        .or_else(|| {
                            cx.report(unsupported("add", kind_of(element)));
                            None
                        })
                }
                Edit::ReplaceLayer { .. } | Edit::AddLayer { .. } | Edit::RemoveLayer { .. } => {
                    cx.report(unsupported(change.name(), "layer"));
                    None
                }
                Edit::ReplaceTextStyle { .. }
                | Edit::AddTextStyle { .. }
                | Edit::RemoveTextStyle { .. } => {
                    cx.report(unsupported(change.name(), "text_style"));
                    None
                }
                Edit::ReplaceLineType { .. }
                | Edit::AddLineType { .. }
                | Edit::RemoveLineType { .. } => {
                    cx.report(unsupported(change.name(), "line_type"));
                    None
                }
                Edit::ReplaceDimensionStyle { .. }
                | Edit::AddDimensionStyle { .. }
                | Edit::RemoveDimensionStyle { .. } => {
                    cx.report(unsupported(change.name(), "dimension_style"));
                    None
                }
            };
            match result {
                Some(result) => expected.push((change.name(), result)),
                None => rejected = true,
            }
        }
        if rejected || !current {
            return Err(Error::Rejected);
        }
        let Some(written) = plan.bytes(cx)? else {
            cx.report(unsupported("apply", "round_trip"));
            return Err(Error::Rejected);
        };
        // 読み直しは、書いたバイト列をもう一度写して保持する。
        work(cx, (written.src.len() as u64).div_ceil(1024))?;
        let updated = match quietly(cx, |cx| Jww::read(&written.src, cx)) {
            Ok(updated) => updated,
            Err(Error::Stopped(stop)) => return Err(Error::Stopped(stop)),
            Err(_) => {
                cx.report(unsupported("apply", "round_trip"));
                return Err(Error::Rejected);
            }
        };
        let drawing = quietly(cx, |cx| updated.lift(cx))?;
        let after = Index::new(&drawing, cx)?;
        let mut applied = Vec::new();
        let mut changed = BTreeSet::new();
        for (edit, expected) in &expected {
            work(cx, 1)?;
            let (record, layer, style, shape) = match expected {
                Expected::Removed => {
                    applied.push(Applied {
                        id: None,
                        hash: None,
                    });
                    continue;
                }
                Expected::Changed(record, layer, style, shape) => {
                    changed.insert(*record);
                    (written.moved.get(record).copied(), layer, style, shape)
                }
                Expected::Added(at, layer, style, shape) => {
                    (written.added.get(*at).copied(), layer, style, shape)
                }
            };
            let found = record
                .and_then(|record| after.found(record))
                .filter(|found| {
                    found.element.layer == *layer
                        && found.element.style == *style
                        && near(&found.element.shape, shape)
                });
            let Some(found) = found else {
                cx.report(unsupported(edit, "round_trip"));
                return Err(Error::Rejected);
            };
            applied.push(Applied {
                id: Some(found.id.clone()),
                hash: Some(hash(found.element, cx)?),
            });
        }
        // ほかの要素は、値が変わらない。除いた要素はなくなり、ほかに要素は増えない
        let mut shifted = false;
        let mut kept = BTreeSet::new();
        for (record, id) in &before.records {
            work(cx, 1)?;
            let Some(&now) = written.moved.get(record) else {
                if written.removed.contains(record) {
                    continue;
                }
                cx.report(unsupported("apply", "round_trip"));
                return Err(Error::Rejected);
            };
            kept.insert(now);
            if changed.contains(record) {
                continue;
            }
            let same = match (id, before.found(*record), after.records.get(&now)) {
                (Some(_), Some(was), Some(Some(_))) => after.found(now).is_some_and(|found| {
                    shifted |= found.id != was.id;
                    same(found.element, was.element)
                }),
                (None, _, Some(None)) => true,
                _ => false,
            };
            if !same {
                let at = before.found(*record).map(|was| was.source());
                cx.report(unsupported("apply", "round_trip").at(at.unwrap_or_default()));
                return Err(Error::Rejected);
            }
        }
        kept.extend(written.added.iter().copied());
        if after.records.keys().any(|record| !kept.contains(record)) {
            cx.report(unsupported("apply", "round_trip"));
            return Err(Error::Rejected);
        }
        *self = updated;
        Ok((applied, shifted))
    }
}
/// 予算の work を使う
fn work(cx: &mut Cx, amount: u64) -> Result<(), Error> {
    cx.check(Kind::Work, amount).map_err(|stop| {
        cx.report(stop.to_diagnostic());
        Error::Stopped(stop)
    })
}

fn hash(element: &Element<ModelPoint>, cx: &mut Cx) -> Result<Hash, Error> {
    Hash::element_checked(element, cx).map_err(|stop| {
        cx.report(stop.to_diagnostic().at(element.source.clone()));
        Error::Stopped(stop)
    })
}

/// 当てた後に確かめること
enum Expected {
    /// 要素がなくなる
    Removed,
    /// このレコードの要素が、このレイヤ・スタイル・形になる
    Changed(usize, LayerId, Style, Shape<ModelPoint>),
    /// 足した順の番号の要素が、このレイヤ・スタイル・形になる
    Added(usize, LayerId, Style, Shape<ModelPoint>),
}
/// 当てられない変更の診断
fn unsupported(edit: &str, kind: &str) -> Diagnostic {
    Diagnostic::new(codes::CHANGE_EDIT_UNSUPPORTED)
        .with("edit", edit)
        .with("kind", kind)
        .with("format", FORMAT)
}

/// 足す要素のクラスの名前
fn kind_of(element: &Placed) -> &'static str {
    let shape = match element {
        Placed::Model(element) => class_of(&element.shape),
        Placed::Paper(_) => None,
    };
    shape.map_or("element", |class| match class {
        LINE => "CDataSen",
        _ => "CDataEnko",
    })
}

/// 形を書くクラス
fn class_of(shape: &Shape<ModelPoint>) -> Option<&'static [u8]> {
    match shape {
        Shape::Line { .. } => Some(LINE),
        Shape::Circle { .. } | Shape::Curve(Edge::Arc { .. }) => Some(ARC),
        _ => None,
    }
}

/// 出所を除いて、同じ値か
fn same(a: &Element<ModelPoint>, b: &Element<ModelPoint>) -> bool {
    a.layer == b.layer && a.style == b.style && a.shape == b.shape
}

/// 形が、書いた値の桁の違いを除いて同じか。角度は 1 周の違いを除く
fn near(a: &Shape<ModelPoint>, b: &Shape<ModelPoint>) -> bool {
    let value = |a: f64, b: f64| (a - b).abs() <= NEAR * a.abs().max(b.abs()).max(1.0);
    let point =
        |a: &ModelPoint, b: &ModelPoint| a.space == b.space && value(a.x, b.x) && value(a.y, b.y);
    let length = |a: &Length, b: &Length| match (a, b) {
        (Length::Model(a), Length::Model(b)) => a.space == b.space && value(a.value, b.value),
        (Length::Paper(a), Length::Paper(b)) => value(a.0, b.0),
        _ => false,
    };
    let angle = |a: f64, b: f64| {
        let d = (a - b).rem_euclid(360.0);
        value(d, 0.0) || value(d, 360.0)
    };
    match (a, b) {
        (Shape::Line { start, end }, Shape::Line { start: s, end: e }) => {
            point(start, s) && point(end, e)
        }
        (
            Shape::Circle { center, radius },
            Shape::Circle {
                center: c,
                radius: r,
            },
        ) => point(center, c) && length(radius, r),
        (
            Shape::Curve(Edge::Arc {
                center,
                radius,
                start,
                end,
                counterclockwise,
            }),
            Shape::Curve(Edge::Arc {
                center: c,
                radius: r,
                start: s,
                end: e,
                counterclockwise: ccw,
            }),
        ) => {
            point(center, c)
                && length(radius, r)
                && counterclockwise == ccw
                && angle(*start, *s)
                && angle(*end, *e)
        }
        _ => a == b,
    }
}

/// 変更が指す要素を探し、前提を確かめる。見つからない・ハッシュが違う・書き戻せないときは診断を足して `None`
fn target_of<'a>(
    index: &Index<'a>,
    seen: &Seen,
    targets: &mut BTreeSet<usize>,
    edit: &str,
    cx: &mut Cx,
) -> Result<Option<Found<'a>>, Error> {
    let Some(&element) = index.elements.get(&seen.id) else {
        cx.report(Diagnostic::new(codes::CHANGE_ELEMENT_MISSING).with("id", seen.id.as_str()));
        return Ok(None);
    };
    if hash(element, cx)? != seen.hash {
        cx.report(Diagnostic::new(codes::CHANGE_ELEMENT_STALE).with("id", seen.id.as_str()));
        return Ok(None);
    }
    // 1 つのレコードから複数の要素ができたものは、1 つだけを書き戻せない
    let found = element
        .source
        .record()
        .and_then(|record| usize::try_from(record).ok())
        .and_then(|record| index.found(record))
        .filter(|found| found.id == seen.id && targets.insert(found.record));
    let Some(found) = found else {
        cx.report(unsupported(edit, "shared").at(element.source.clone()));
        return Ok(None);
    };
    Ok(Some(found))
}

/// 要素と、そのレコード
struct Found<'a> {
    id: String,
    element: &'a Element<ModelPoint>,
    record: usize,
}

impl Found<'_> {
    fn source(&self) -> Ref {
        self.element.source.clone()
    }
}

/// ID から要素への索引
struct Index<'a> {
    elements: BTreeMap<String, &'a Element<ModelPoint>>,
    /// レコードの番号から ID。複数の要素を作ったレコードは `None`
    records: BTreeMap<usize, Option<String>>,
    /// 実寸の空間の ID
    spaces: BTreeMap<String, SpaceId>,
}

impl<'a> Index<'a> {
    fn new(drawing: &'a Drawing2D, cx: &mut Cx) -> Result<Self, Error> {
        let mut index = Index {
            elements: BTreeMap::new(),
            records: BTreeMap::new(),
            spaces: BTreeMap::new(),
        };
        for space in &drawing.spaces {
            let place = id::space(space.id);
            let mut namer = id::Namer::new(&place);
            for (at, element) in space.elements.iter().enumerate() {
                work(cx, 1)?;
                index.note(namer.next(at, element), element);
            }
            index.spaces.insert(place, space.id);
        }
        let mut definitions = id::DefinitionNamer::default();
        for definition in &drawing.symbols {
            work(cx, 1)?;
            let place = definitions.next(definition);
            let mut namer = id::Namer::new(&place);
            for (at, element) in definition.space.elements.iter().enumerate() {
                work(cx, 1)?;
                index.note(namer.next(at, element), element);
            }
        }
        Ok(index)
    }

    fn note(&mut self, id: String, element: &'a Element<ModelPoint>) {
        if let Some(record) = element
            .source
            .record()
            .and_then(|r| usize::try_from(r).ok())
        {
            self.records
                .entry(record)
                .and_modify(|id| *id = None)
                .or_insert_with(|| Some(id.clone()));
        }
        self.elements.insert(id, element);
    }

    /// レコードが作った、ただ 1 つの要素
    fn found(&self, record: usize) -> Option<Found<'a>> {
        let id = self.records.get(&record)?.as_ref()?;
        let element = *self.elements.get(id)?;
        Some(Found {
            id: id.clone(),
            element,
            record,
        })
    }
}

/// 図形の共通の欄
#[derive(Clone, Copy)]
struct Base {
    curve: u32,
    pen_style: u8,
    pen_color: u16,
    pen_width: u16,
    layer: u16,
    group: u16,
    flag: u16,
}

impl Base {
    fn read(src: &[u8], at: usize, version: u32) -> Option<Base> {
        let mut r = Reader::new(src, at);
        Some(Base {
            curve: r.u32().ok()?,
            pen_style: r.u8().ok()?,
            pen_color: r.u16().ok()?,
            pen_width: if version >= PEN_WIDTH_SINCE {
                r.u16().ok()?
            } else {
                0
            },
            layer: r.u16().ok()?,
            group: r.u16().ok()?,
            flag: r.u16().ok()?,
        })
    }

    fn bytes(&self, version: u32) -> Vec<u8> {
        let mut out = Vec::with_capacity(15);
        out.extend(self.curve.to_le_bytes());
        out.push(self.pen_style);
        out.extend(self.pen_color.to_le_bytes());
        if version >= PEN_WIDTH_SINCE {
            out.extend(self.pen_width.to_le_bytes());
        }
        out.extend(self.layer.to_le_bytes());
        out.extend(self.group.to_le_bytes());
        out.extend(self.flag.to_le_bytes());
        out
    }

    /// 変換と同じレイヤ
    fn layer(&self) -> LayerId {
        LayerId(u32::from(self.group & 0xF) * 16 + u32::from(self.layer & 0xF) + 1)
    }
}

/// 元のバイト列への書き直し
struct Plan<'f> {
    file: &'f JwwFile,
    header: &'f Header,
    /// 同じ大きさで書き直す位置と、新しいバイト列
    patches: Vec<(usize, Vec<u8>)>,
    /// 除くレコード
    removed: BTreeSet<usize>,
    /// 図形の一覧の終わりに足す図形のクラスと、印の後ろのバイト列
    added: Vec<(&'static [u8], Vec<u8>)>,
}

/// 書き直したバイト列と、レコードの番号の対応
struct Written {
    src: Vec<u8>,
    /// 元のレコードの番号から、書き直した後の番号
    moved: BTreeMap<usize, usize>,
    removed: BTreeSet<usize>,
    /// 足した図形の、書き直した後の番号
    added: Vec<usize>,
}

impl Plan<'_> {
    fn version(&self) -> u32 {
        self.header.version
    }

    /// 診断に書く種類（クラスの名前）
    fn kind(&self, record: usize) -> String {
        self.file
            .records()
            .get(record)
            .map(|record| String::from_utf8_lossy(self.file.kind_name(record)).into_owned())
            .unwrap_or_default()
    }

    /// 図形のレコードのクラスの名前と、共通の欄の位置
    fn entity(&self, record: usize) -> Option<(&[u8], usize)> {
        let found = self.file.records().get(record)?;
        (found.kind() == RecordKind::Entity)
            .then(|| (self.file.kind_name(found), found.body().start))
    }

    fn remove(&mut self, record: usize) -> Option<()> {
        self.entity(record)?;
        self.removed.insert(record);
        Some(())
    }

    /// レイヤとスタイルのうち、変わったものを共通の欄に書く。読み直したときのレイヤとスタイルを返す
    fn styled(&mut self, found: &Found, layer: LayerId, style: &Style) -> Option<(LayerId, Style)> {
        let (name, at) = self.entity(found.record)?;
        if name != LINE && name != ARC {
            return None;
        }
        let was = Base::read(&self.file.src, at, self.version())?;
        let in_definition = self.file.records().get(found.record)?.block().is_some();
        let base = self.base(was, found.element, layer, style, in_definition)?;
        self.patches.push((at, base.bytes(self.version())));
        Some((base.layer(), self.style(&base)))
    }

    /// 求めたレイヤとスタイルの共通の欄。変わらない値は、元の欄のまま
    fn base(
        &self,
        was: Base,
        element: &Element<ModelPoint>,
        layer: LayerId,
        style: &Style,
        in_definition: bool,
    ) -> Option<Base> {
        let mut base = was;
        if style.line_type_scale != 1.0 || style.transparency != Inherit::Layer {
            return None;
        }
        if layer != element.layer {
            let n = layer.0.checked_sub(1)?;
            let group = u16::try_from(n / 16).ok().filter(|group| *group < 16)?;
            // レイヤグループは空間と縮尺を決めるので、ブロックの定義の外では変えない
            if !in_definition && group != was.group & 0xF {
                return None;
            }
            // 使わない上位ビットは意味が未確認なので残す。
            base.group = (was.group & !0xF) | group;
            base.layer = (was.layer & !0xF) | u16::try_from(n % 16).ok()?;
        }
        if style.color != element.style.color {
            base.pen_color = self.pen(style.color)?;
        }
        if style.line_type != element.style.line_type {
            base.pen_style = self.line_type(style.line_type)?;
        }
        if style.line_weight != element.style.line_weight {
            base.pen_width = self.width(style.line_weight?)?;
        }
        Some(base)
    }

    /// 共通の欄を読み直したときのスタイル（変換の `line_style` と同じ）
    fn style(&self, base: &Base) -> Style {
        Style {
            color: Inherit::Own(self.header.color(base.pen_color)),
            line_type: Inherit::Own(LineTypeId(u32::from(base.pen_style.max(1)))),
            line_type_scale: 1.0,
            line_weight: self.header.weight(base.pen_color, base.pen_width),
            transparency: Inherit::Layer,
        }
    }

    /// 色の番号。JWW の番号か、RGB だけなら同じ RGB のペン（1〜9、SXF の拡張色）
    fn pen(&self, color: Inherit<Color>) -> Option<u16> {
        let Inherit::Own(color) = color else {
            return None;
        };
        let sxf = self.version() >= SXF_SINCE;
        let valid = |pen: u16| pen <= 9 || (sxf && (100..=356).contains(&pen));
        let pen = match color.code {
            Some(ColorCode {
                system: ColorSystem::Jww,
                value,
            }) => u16::try_from(value).ok().filter(|pen| valid(*pen))?,
            Some(_) => return None,
            None => {
                let rgb = color.rgb?;
                (1..=9)
                    .chain(100..=356)
                    .filter(|pen| valid(*pen))
                    .find(|pen| self.header.color(*pen).rgb == Some(rgb))?
            }
        };
        if color.rgb.is_some() && color.rgb != self.header.color(pen).rgb {
            return None;
        }
        Some(pen)
    }

    /// 線種の番号。実線と破線（1〜9）、ランダム線と倍長線（11〜19）、版 420 からの SXF の線種（30〜62）
    fn line_type(&self, line_type: Inherit<LineTypeId>) -> Option<u8> {
        let Inherit::Own(LineTypeId(code)) = line_type else {
            return None;
        };
        let code = u8::try_from(code).ok()?;
        let sxf = self.version() >= SXF_SINCE && (30..=62).contains(&code);
        ((1..=9).contains(&code) || (11..=19).contains(&code) || sxf).then_some(code)
    }

    /// 図形の線幅の値。見出しの単位（1/100 mm かドット）で書け、読み直すと同じ太さになるものだけ
    fn width(&self, weight: Inherit<LineWeight>) -> Option<u16> {
        let Inherit::Own(LineWeight::Width(PaperLength(mm))) = weight else {
            return None;
        };
        if self.version() < PEN_WIDTH_SINCE || !mm.is_finite() || mm <= 0.0 {
            return None;
        }
        let units = if self.header.max_draw_width < 0 {
            mm * 100.0
        } else {
            mm * self.header.dpi? / 25.4
        };
        let value = u16::try_from(units.round() as i64)
            .ok()
            .filter(|v| *v > 0)?;
        let read = self.header.width(u32::from(value))?;
        ((read - mm).abs() <= NEAR).then_some(value)
    }

    /// 座標に掛ける倍率。ブロックの定義の中は 1、ほかはレイヤグループの縮尺の分母
    fn scale(&self, group: u16, in_definition: bool) -> f64 {
        if in_definition {
            1.0
        } else {
            self.header.scale(usize::from(group & 0xF))
        }
    }

    /// 形の値を書く。クラスは変えない
    fn shaped(&mut self, found: &Found, shape: &Shape<ModelPoint>) -> Option<()> {
        let (name, at) = self.entity(found.record)?;
        if class_of(shape) != Some(name) {
            return None;
        }
        let base = Base::read(&self.file.src, at, self.version())?;
        let in_definition = self.file.records().get(found.record)?.block().is_some();
        let space = space_of(&found.element.shape)?;
        let bytes = shape_bytes(shape, space, self.scale(base.group, in_definition))?;
        let size = base.bytes(self.version()).len();
        self.patches.push((at + size, bytes));
        Some(())
    }

    /// レイヤグループの空間の図形の一覧の終わりに、線・円・円弧を足す。読み直したときのレイヤ・スタイル・形を返す
    fn add(
        &mut self,
        space: SpaceId,
        element: &Element<ModelPoint>,
    ) -> Option<(LayerId, Style, Shape<ModelPoint>)> {
        let group = u16::try_from(space.0).ok().filter(|group| *group < 16)?;
        let class = class_of(&element.shape)?;
        let n = element.layer.0.checked_sub(1)?;
        if n / 16 != u32::from(group) {
            return None;
        }
        let style = element.style;
        if style.line_type_scale != 1.0 || style.transparency != Inherit::Layer {
            return None;
        }
        let base = Base {
            curve: 0,
            pen_style: self.line_type(style.line_type)?,
            pen_color: self.pen(style.color)?,
            pen_width: match style.line_weight {
                // 書かなければ、ペンの印刷の幅に従う
                None => 0,
                Some(weight) => self.width(weight)?,
            },
            layer: u16::try_from(n % 16).ok()?,
            group,
            flag: 0,
        };
        let mut bytes = base.bytes(self.version());
        bytes.extend(shape_bytes(
            &element.shape,
            space,
            self.scale(group, false),
        )?);
        // 新しいクラスの印は、ファイルのクラスのスキーマで書く
        self.file.classes().first()?;
        self.added.push((class, bytes));
        Some((base.layer(), self.style(&base), element.shape.clone()))
    }

    /// 書き直したバイト列。一覧を読めなければ `None`
    fn bytes(self, cx: &mut Cx) -> Result<Option<Written>, Error> {
        work(cx, (self.file.src.len() as u64).div_ceil(1024))?;
        let mut src = self.file.src.clone();
        for (at, bytes) in &self.patches {
            work(cx, 1)?;
            let Some(target) = src.get_mut(*at..*at + bytes.len()) else {
                return Ok(None);
            };
            target.copy_from_slice(bytes);
        }
        if self.removed.is_empty() && self.added.is_empty() {
            let moved = (0..self.file.records().len()).map(|at| (at, at)).collect();
            return Ok(Some(Written {
                src,
                moved,
                removed: BTreeSet::new(),
                added: Vec::new(),
            }));
        }
        // 一覧の再構築でも元のバイトを写す。足した図形の中身も数える。
        let added = self.added.iter().fold(0u64, |total, (_, body)| {
            total.saturating_add(body.len() as u64)
        });
        work(cx, (src.len() as u64).saturating_add(added).div_ceil(1024))?;
        Ok(Lists {
            plan: &self,
            src: &src,
            out: Vec::with_capacity(src.len()),
            numbers: BTreeMap::new(),
            next: 1,
            moved: BTreeMap::new(),
            records: 0,
        }
        .write(cx)?
        .map(|(out, moved, added)| Written {
            src: out,
            moved,
            removed: self.removed.clone(),
            added,
        }))
    }
}

/// 図形の一覧とブロックの定義の一覧を、件数とオブジェクトの印を数え直して書く
struct Lists<'p> {
    plan: &'p Plan<'p>,
    src: &'p [u8],
    out: Vec<u8>,
    /// 書いたクラスの名前とスキーマから、その番号
    numbers: BTreeMap<(Vec<u8>, u16), u32>,
    /// 次の番号。クラスとオブジェクトの番号は、ファイルの全体で 1 から数える
    next: u32,
    moved: BTreeMap<usize, usize>,
    /// 書いたレコードの数
    records: usize,
}

impl Lists<'_> {
    #[expect(clippy::type_complexity)]
    fn write(
        mut self,
        cx: &mut Cx,
    ) -> Result<Option<(Vec<u8>, BTreeMap<usize, usize>, Vec<usize>)>, Error> {
        let records = self.plan.file.records();
        let Some(header) = records.first() else {
            return Ok(None);
        };
        let header_end = header.span().end;
        self.out
            .extend_from_slice(self.src.get(..header_end).unwrap_or_default());
        self.moved.insert(0, 0);
        self.records = 1;
        let mut r = Reader::new(self.src, header_end);
        let Some(entities) = count(&mut r) else {
            return Ok(None);
        };
        let mut at = 1;
        let mut kept = Vec::new();
        for _ in 0..entities {
            let Some(record) = records.get(at) else {
                return Ok(None);
            };
            if record.block().is_some() {
                return Ok(None);
            }
            if !self.plan.removed.contains(&at) {
                kept.push(at);
            }
            r.pos = record.span().end;
            at += 1;
        }
        write_count(&mut self.out, kept.len() + self.plan.added.len());
        for record in kept {
            work(cx, 1)?;
            if !self.object(record) {
                return Ok(None);
            }
        }
        let mut added = Vec::new();
        for (class, body) in &self.plan.added {
            work(cx, 1)?;
            let classes = self.plan.file.classes();
            work(cx, classes.len() as u64)?;
            // 同じクラスがすでにあれば、そのスキーマを使う。なければ全クラスが同じ場合だけ推定できる。
            let same = classes
                .iter()
                .find(|item| self.plan.file.bytes(item.name()) == *class);
            let schema = same.map(crate::Class::schema).or_else(|| {
                let schema = classes.first()?.schema();
                classes
                    .iter()
                    .all(|item| item.schema() == schema)
                    .then_some(schema)
            });
            let Some(schema) = schema else {
                return Ok(None);
            };
            self.tag(class, schema);
            self.out.extend_from_slice(body);
            added.push(self.records);
            self.records += 1;
        }
        let Some(blocks) = count(&mut r) else {
            return Ok(None);
        };
        write_count(&mut self.out, blocks as usize);
        for _ in 0..blocks {
            let Some(record) = records.get(at) else {
                return Ok(None);
            };
            let definition = at;
            // 定義のレコードは、中の一覧の件数で終わる
            let head = match record.kind() {
                RecordKind::Null => record.span().start + 2,
                RecordKind::Block => {
                    let mut head = Reader::new(self.src, record.body().start);
                    let skip = Base::read(self.src, head.pos, self.plan.version())
                        .map(|base| base.bytes(self.plan.version()).len());
                    let Some(skip) = skip else {
                        return Ok(None);
                    };
                    if head.skip(skip + 12).is_err() || head.string().is_err() {
                        return Ok(None);
                    }
                    head.pos
                }
                _ => return Ok(None),
            };
            let mut inner = Reader::new(self.src, head);
            let Some(n) = count(&mut inner) else {
                return Ok(None);
            };
            if inner.pos != record.span().end {
                return Ok(None);
            }
            if !self.object_until(definition, head) {
                return Ok(None);
            }
            at += 1;
            let mut kept = Vec::new();
            for _ in 0..n {
                let Some(record) = records.get(at) else {
                    return Ok(None);
                };
                if record.block() != Some(definition) {
                    return Ok(None);
                }
                if !self.plan.removed.contains(&at) {
                    kept.push(at);
                }
                at += 1;
            }
            write_count(&mut self.out, kept.len());
            for record in kept {
                work(cx, 1)?;
                if !self.object(record) {
                    return Ok(None);
                }
            }
        }
        // 画像の一覧と、読めなかった残りは写す
        let rest = records
            .get(at..)
            .and_then(|rest| rest.first())
            .map_or(self.src.len(), |record| record.span().start);
        let end = records
            .get(at.wrapping_sub(1))
            .map_or(header_end, |record| record.span().end);
        if end > rest {
            return Ok(None);
        }
        self.out
            .extend_from_slice(self.src.get(end..).unwrap_or_default());
        for old in at..records.len() {
            self.moved.insert(old, self.records);
            self.records += 1;
        }
        Ok(Some((self.out, self.moved, added)))
    }

    /// オブジェクトを、印を数え直して写す
    fn object(&mut self, record: usize) -> bool {
        let end = self
            .plan
            .file
            .records()
            .get(record)
            .map(|found| found.span().end);
        end.is_some_and(|end| self.object_until(record, end))
    }

    /// オブジェクトの印と、中身の `end` までを写す
    fn object_until(&mut self, record: usize, end: usize) -> bool {
        let Some(found) = self.plan.file.records().get(record) else {
            return false;
        };
        match found.kind() {
            RecordKind::Null => self.out.extend([0, 0]),
            RecordKind::Entity | RecordKind::Block => {
                let Some(class) = found
                    .tag()
                    .and_then(|(_, class)| self.plan.file.classes().get(class))
                else {
                    return false;
                };
                let name = self.plan.file.bytes(class.name()).to_vec();
                self.tag(&name, class.schema());
                let Some(body) = self.src.get(found.body().start..end) else {
                    return false;
                };
                self.out.extend_from_slice(body);
            }
            _ => return false,
        }
        self.moved.insert(record, self.records);
        self.records += 1;
        true
    }

    /// オブジェクトの前の印。初めてのクラスは名前を書き、ほかは番号で指す（MFC の CArchive）
    fn tag(&mut self, name: &[u8], schema: u16) {
        let key = (name.to_vec(), schema);
        match self.numbers.get(&key) {
            Some(&number) if number < 0x7FFF => {
                self.out.extend((0x8000 | number as u16).to_le_bytes());
            }
            Some(&number) => {
                self.out.extend([0xFF, 0x7F]);
                self.out.extend((0x8000_0000 | number).to_le_bytes());
            }
            None => {
                self.out.extend([0xFF, 0xFF]);
                self.out.extend(schema.to_le_bytes());
                self.out.extend((name.len() as u16).to_le_bytes());
                self.out.extend_from_slice(name);
                self.numbers.insert(key, self.next);
                self.next += 1;
            }
        }
        // オブジェクトも番号を 1 つ使う
        self.next += 1;
    }
}

/// MFC の件数。`FFFF` なら 4 バイトが続く
fn count(r: &mut Reader) -> Option<u32> {
    match r.u16().ok()? {
        0xFFFF => r.u32().ok(),
        n => Some(u32::from(n)),
    }
}

fn write_count(out: &mut Vec<u8>, n: usize) {
    match u16::try_from(n) {
        Ok(n) if n < 0xFFFF => out.extend(n.to_le_bytes()),
        _ => {
            out.extend([0xFF, 0xFF]);
            out.extend(u32::try_from(n).unwrap_or(u32::MAX).to_le_bytes());
        }
    }
}

/// 形の点の空間
fn space_of(shape: &Shape<ModelPoint>) -> Option<SpaceId> {
    match shape {
        Shape::Line { start, .. } => Some(start.space),
        Shape::Circle { center, .. } => Some(center.space),
        Shape::Curve(Edge::Arc { center, .. } | Edge::Elliptic { center, .. }) => {
            Some(center.space)
        }
        _ => None,
    }
}

/// 線・円・円弧の形の値。座標は `scale` で割って用紙の値にする。円弧は傾き 0・扁平率 1 で書く
fn shape_bytes(shape: &Shape<ModelPoint>, space: SpaceId, scale: f64) -> Option<Vec<u8>> {
    let mut values = Vec::new();
    let point = |values: &mut Vec<f64>, point: &ModelPoint| {
        (point.space == space).then(|| values.extend([point.x / scale, point.y / scale]))
    };
    let length = |length: &Length| match length {
        Length::Model(ModelLength { space: at, value }) if *at == space => Some(value / scale),
        _ => None,
    };
    let full = match shape {
        Shape::Line { start, end } => {
            point(&mut values, start)?;
            point(&mut values, end)?;
            None
        }
        Shape::Circle { center, radius } => {
            point(&mut values, center)?;
            values.extend([length(radius)?, 0.0, TURN, 0.0, 1.0]);
            Some(1u32)
        }
        Shape::Curve(Edge::Arc {
            center,
            radius,
            start,
            end,
            counterclockwise,
        }) => {
            point(&mut values, center)?;
            let sweep = if *counterclockwise {
                (end - start).rem_euclid(360.0)
            } else {
                -(start - end).rem_euclid(360.0)
            };
            if sweep == 0.0 {
                return None;
            }
            values.extend([
                length(radius)?,
                start.to_radians(),
                sweep.to_radians(),
                0.0,
                1.0,
            ]);
            Some(0)
        }
        _ => return None,
    };
    if !values.iter().all(|value| value.is_finite()) {
        return None;
    }
    let mut out: Vec<u8> = values
        .iter()
        .flat_map(|value| value.to_le_bytes())
        .collect();
    if let Some(full) = full {
        out.extend(full.to_le_bytes());
    }
    Some(out)
}
