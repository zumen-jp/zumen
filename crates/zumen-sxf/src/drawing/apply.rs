//! 図面モデルの変更を、SFC のフィーチャのコメントに書き戻す（決定 0167）。
//!
//! 変更は、元のバイト列のうち、引数の値の範囲を書き直すか、フィーチャのコメントを除くか足すかで当てる。ほかのバイトは
//! 写す。当てた後に読み直して図面モデルに変換し、変えた要素が求めた値になり、ほかの要素が ID も値も変わらないことを
//! 確かめる。合わなければ、何も変えずに断る。

use core::ops::Range;
use std::collections::{BTreeMap, BTreeSet};

use zumen_base::{Diagnostic, Diagnostics, Key, Ref, codes};
use zumen_drawing::{
    Applied, Changes, Color, ColorCode, ColorSystem, Drawing2D, Edge, Edit, Element, Hash, Inherit,
    LayerId, Length, LineTypeId, LineWeight, ModelPoint, PaperLength, PaperPoint, Placed, Polyline,
    Seen, Shape, Style, id, transform,
};
use zumen_sdk::{Apply, Codec, Cx, Error, Lift};

mod p21;

use super::{
    Args, FeatureRecord, LINE_TYPES, PREDEFINED_COLORS, USER_LINE_TYPE, USER_WIDTH, WIDTHS,
    sfc_records, work,
};
use crate::p21::records as p21_records;
use crate::{Marker, Mode, Quote, Sxf, SxfFile, Value};

/// レイヤ・色・線種・線幅のコードを引数の 0〜3 に持ち、スタイルを変えられるフィーチャ
const STYLED: [&str; 8] = [
    "line",
    "polyline",
    "circle",
    "arc",
    "ellipse",
    "ellipse_arc",
    "spline",
    "clothoid",
];

/// 形を書き直せ、足せるフィーチャ
const SHAPED: [&str; 4] = ["line", "polyline", "circle", "arc"];

/// 除けるフィーチャ。複合曲線と複合図形の定義・配置は、ほかのフィーチャが順の番号や名前で指すので除かない
const REMOVABLE: [&str; 20] = [
    "line",
    "polyline",
    "circle",
    "arc",
    "ellipse",
    "ellipse_arc",
    "spline",
    "clothoid",
    "text_string",
    "point_marker",
    "externally_defined_symbol",
    "linear_dim",
    "curve_dim",
    "angular_dim",
    "radius_dim",
    "diameter_dim",
    "label",
    "balloon",
    "fill_area_style_colour",
    "fill_area_style_hatching",
];

impl Apply<Changes> for SxfFile {
    type Output = Vec<Applied>;

    fn apply(&mut self, changes: &Changes, cx: &mut Cx) -> Result<Vec<Applied>, Error> {
        // 診断に書く形式の名前
        let format = match self.mode() {
            Mode::Sfc => "SFC",
            Mode::P21 => "P21",
        };
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
        // SFC のフィーチャの引数を取り出す作業を、バイト数に応じて数える。
        if self.mode() == Mode::Sfc {
            work(cx, (self.src.len() as u64).div_ceil(1024), &Ref::default())?;
        }
        // 変換の診断は、書き戻しの診断に混ぜない
        let mut quiet = Diagnostics::new();
        let records = match self.mode() {
            Mode::Sfc => sfc_records(self),
            Mode::P21 => p21_records(self, &mut Cx::new(cx.budget, &mut quiet, cx.cancel))?,
        };
        let drawing = self.lift(&mut Cx::new(cx.budget, &mut quiet, cx.cancel))?;
        let before = Index::new(&drawing, cx)?;
        let tables = Tables::new(&records, &drawing, cx)?;
        let grouped = grouped(&records, cx)?;
        // 改行の種類を調べるため、元のバイト列を走査する。
        work(cx, (self.src.len() as u64).div_ceil(1024), &Ref::default())?;
        let writer: Box<dyn Writer> = match self.mode() {
            Mode::Sfc => Box::new(Sfc::new(self, &records)),
            Mode::P21 => Box::new(p21::P21::new(
                self,
                crate::p21::codes(self, &mut Cx::new(cx.budget, &mut quiet, cx.cancel))?,
                tables.widths.clone(),
            )),
        };
        let mut plan = Plan::new(&records, writer);
        let mut expected = Vec::new();
        let mut targets = BTreeSet::new();
        let mut rejected = false;
        for change in &changes.edits {
            work(cx, 1, &Ref::default())?;
            let result = match change {
                Edit::Remove { target } => {
                    let Some(found) =
                        target_of(&before, target, &mut targets, (format, "remove"), cx)?
                    else {
                        rejected = true;
                        continue;
                    };
                    let record = plan.record(found.record);
                    let name = record.map(|record| record.name.to_ascii_lowercase());
                    let removed = name.filter(|name| {
                        REMOVABLE.contains(&name.as_str()) && !grouped.contains(&found.record)
                    });
                    match removed.and_then(|_| plan.remove(found.record)) {
                        Some(()) => Some(Expected::Removed(found.instance)),
                        None => {
                            cx.report(
                                unsupported(format, "remove", &found.kind(&plan))
                                    .at(found.source()),
                            );
                            None
                        }
                    }
                }
                Edit::Restyle { target, style } => {
                    let Some(found) =
                        target_of(&before, target, &mut targets, (format, "restyle"), cx)?
                    else {
                        rejected = true;
                        continue;
                    };
                    let (layer, new) = found.element.restyled(style);
                    work(
                        cx,
                        tables.style_work(&new, Some(found.element.style())),
                        &Ref::default(),
                    )?;
                    let mut element = found.element.with(layer, new, None);
                    plan.styled(&found, &mut element, &tables, &grouped)
                        .map(|()| Expected::Element(found.instance, Box::new(element)))
                        .or_else(|| {
                            cx.report(
                                unsupported(format, "restyle", &found.kind(&plan))
                                    .at(found.source()),
                            );
                            None
                        })
                }
                Edit::Replace { target, element } => {
                    let Some(found) =
                        target_of(&before, target, &mut targets, (format, "replace"), cx)?
                    else {
                        rejected = true;
                        continue;
                    };
                    // 渡された形を複製して書式化する前に、可変長の内容を予算で数える。
                    match element.as_ref() {
                        Placed::Model(element) => Any::Model(element).hash(cx)?,
                        Placed::Paper(element) => Any::Paper(element).hash(cx)?,
                    };
                    let element = found.element.replaced(element);
                    if let Some(element) = &element {
                        let style = match element {
                            Placed::Model(element) => &element.style,
                            Placed::Paper(element) => &element.style,
                        };
                        work(
                            cx,
                            tables.style_work(style, Some(found.element.style())),
                            &Ref::default(),
                        )?;
                    }
                    element
                        .and_then(|mut element| {
                            plan.styled(&found, &mut element, &tables, &grouped)?;
                            plan.shaped(&found, &element, &grouped)?;
                            Some(Expected::Element(found.instance, Box::new(element)))
                        })
                        .or_else(|| {
                            cx.report(
                                unsupported(format, "replace", &found.kind(&plan))
                                    .at(found.source()),
                            );
                            None
                        })
                }
                Edit::Transform { target, transform } => {
                    let Some(found) =
                        target_of(&before, target, &mut targets, (format, "transform"), cx)?
                    else {
                        rejected = true;
                        continue;
                    };
                    // 変換は元の形を走査し、結果を複製して書式化する。
                    found.element.hash(cx)?;
                    let element = found.element.transformed(transform);
                    element
                        .and_then(|element| {
                            plan.shaped(&found, &element, &grouped)?;
                            Some(Expected::Element(found.instance, Box::new(element)))
                        })
                        .or_else(|| {
                            cx.report(
                                unsupported(format, "transform", &found.kind(&plan))
                                    .at(found.source()),
                            );
                            None
                        })
                }
                Edit::Add { place, element } => {
                    let added = match element.as_ref() {
                        Placed::Paper(element)
                            if before.sheets.get(place) == Some(&element.shape_sheet()) =>
                        {
                            Any::Paper(element).hash(cx)?;
                            work(cx, tables.style_work(&element.style, None), &Ref::default())?;
                            plan.add(element, &tables)
                        }
                        _ => None,
                    };
                    added
                        .map(|(instance, element)| {
                            Expected::Element(instance, Box::new(Placed::Paper(element)))
                        })
                        .or_else(|| {
                            cx.report(unsupported(format, "add", kind_of(element)));
                            None
                        })
                }
                Edit::ReplaceLayer { .. } | Edit::AddLayer { .. } | Edit::RemoveLayer { .. } => {
                    cx.report(unsupported(format, edit_name(change), "layer_feature"));
                    None
                }
                Edit::ReplaceTextStyle { .. }
                | Edit::AddTextStyle { .. }
                | Edit::RemoveTextStyle { .. } => {
                    cx.report(unsupported(format, edit_name(change), "text_font_feature"));
                    None
                }
                Edit::ReplaceLineType { .. }
                | Edit::AddLineType { .. }
                | Edit::RemoveLineType { .. } => {
                    cx.report(unsupported(
                        format,
                        edit_name(change),
                        "user_defined_font_feature",
                    ));
                    None
                }
                Edit::ReplaceDimensionStyle { .. }
                | Edit::AddDimensionStyle { .. }
                | Edit::RemoveDimensionStyle { .. } => {
                    cx.report(unsupported(format, edit_name(change), "dimension_style"));
                    None
                }
            };
            match result {
                Some(result) => expected.push((edit_name(change), result)),
                None => rejected = true,
            }
        }
        if rejected || !current {
            return Err(Error::Rejected);
        }
        let src = plan.bytes(&self.src, cx)?;
        // 読み直しは、書いたバイト列をもう一度写して保持する。
        work(cx, (src.len() as u64).div_ceil(1024), &Ref::default())?;
        let updated = quietly(cx, |cx| Sxf::read(&src, cx));
        let updated = match updated {
            Ok(updated) => updated,
            Err(Error::Stopped(stop)) => return Err(Error::Stopped(stop)),
            Err(_) => {
                cx.report(unsupported(format, "apply", "round_trip"));
                return Err(Error::Rejected);
            }
        };
        let drawing = quietly(cx, |cx| updated.lift(cx))?;
        let after = Index::new(&drawing, cx)?;
        let mut applied = Vec::new();
        for (edit, expected) in &expected {
            work(cx, 1, &Ref::default())?;
            let found = match expected {
                Expected::Removed(instance) => {
                    if after.instances.contains_key(instance) {
                        cx.report(unsupported(format, edit, "round_trip"));
                        return Err(Error::Rejected);
                    }
                    applied.push(Applied {
                        id: None,
                        hash: None,
                    });
                    continue;
                }
                Expected::Element(instance, element) => after.found(*instance).filter(|found| {
                    // P21 の円弧の角度は、ラジアンで書いて度で読むので、最後の桁が違いうる
                    found.element.same(element)
                        || (self.mode() == Mode::P21 && found.element.near(element))
                }),
            };
            let Some(found) = found else {
                cx.report(unsupported(format, edit, "round_trip"));
                return Err(Error::Rejected);
            };
            applied.push(Applied {
                id: Some(found.id.clone()),
                hash: Some(found.element.hash(cx)?),
            });
        }
        // ほかの要素は、ID も値も変わらない
        let changed: BTreeSet<u64> = expected
            .iter()
            .map(|(_, expected)| match expected {
                Expected::Removed(instance) | Expected::Element(instance, _) => *instance,
            })
            .collect();
        for (instance, id) in &before.instances {
            work(cx, 1, &Ref::default())?;
            if changed.contains(instance) {
                continue;
            }
            let kept = match (id, before.found(*instance), after.instances.get(instance)) {
                (Some(_), Some(was), Some(Some(_))) => after
                    .found(*instance)
                    .is_some_and(|now| now.id == was.id && now.element.same(&was.element.placed())),
                (None, _, Some(None)) => true,
                _ => false,
            };
            if !kept {
                let at = before.found(*instance).map(|was| was.source());
                cx.report(unsupported(format, "apply", "round_trip").at(at.unwrap_or_default()));
                return Err(Error::Rejected);
            }
        }
        *self = updated;
        Ok(applied)
    }
}

/// 診断を混ぜずに読む・変換する。止めたときだけ、止めた理由を文脈に足す
fn quietly<T>(cx: &mut Cx, run: impl FnOnce(&mut Cx) -> Result<T, Error>) -> Result<T, Error> {
    let mut quiet = Diagnostics::new();
    let result = run(&mut Cx::new(cx.budget, &mut quiet, cx.cancel));
    if let Err(Error::Stopped(stop)) = &result {
        cx.report(stop.to_diagnostic());
    }
    result
}

/// 当てた後に確かめること
enum Expected {
    /// このインスタンスの要素がなくなる
    Removed(u64),
    /// このインスタンスの要素が、この値になる
    Element(u64, Box<Placed>),
}

/// 変更の名前。診断の `edit` に書く
fn edit_name(edit: &Edit) -> &'static str {
    match edit {
        Edit::Replace { .. } => "replace",
        Edit::Restyle { .. } => "restyle",
        Edit::Transform { .. } => "transform",
        Edit::Add { .. } => "add",
        Edit::Remove { .. } => "remove",
        Edit::ReplaceLayer { .. } => "replace_layer",
        Edit::AddLayer { .. } => "add_layer",
        Edit::RemoveLayer { .. } => "remove_layer",
        Edit::ReplaceTextStyle { .. } => "replace_text_style",
        Edit::AddTextStyle { .. } => "add_text_style",
        Edit::RemoveTextStyle { .. } => "remove_text_style",
        Edit::ReplaceLineType { .. } => "replace_line_type",
        Edit::AddLineType { .. } => "add_line_type",
        Edit::RemoveLineType { .. } => "remove_line_type",
        Edit::ReplaceDimensionStyle { .. } => "replace_dimension_style",
        Edit::AddDimensionStyle { .. } => "add_dimension_style",
        Edit::RemoveDimensionStyle { .. } => "remove_dimension_style",
    }
}

/// 当てられない変更の診断
fn unsupported(format: &str, edit: &str, kind: &str) -> Diagnostic {
    Diagnostic::new(codes::CHANGE_EDIT_UNSUPPORTED)
        .with("edit", edit)
        .with("kind", kind)
        .with("format", format)
}

/// 足す要素のフィーチャの種類
fn kind_of(element: &Placed) -> &'static str {
    let shape = match element {
        Placed::Paper(element) => feature_name(&element.shape),
        Placed::Model(element) => feature_name(&element.shape),
    };
    shape.unwrap_or("element")
}

/// 形を書くフィーチャの名前
fn feature_name<P>(shape: &Shape<P>) -> Option<&'static str> {
    Some(match shape {
        Shape::Line { .. } => "line_feature",
        Shape::Polyline(_) => "polyline_feature",
        Shape::Circle { .. } => "circle_feature",
        Shape::Curve(Edge::Arc { .. }) => "arc_feature",
        _ => return None,
    })
}

/// 変更が指す要素を探し、前提を確かめる。見つからない・ハッシュが違う・書き戻せないときは診断を足して `None`
fn target_of<'a>(
    index: &Index<'a>,
    seen: &Seen,
    targets: &mut BTreeSet<u64>,
    (format, edit): (&str, &str),
    cx: &mut Cx,
) -> Result<Option<Found<'a>>, Error> {
    let Some(&element) = index.elements.get(&seen.id) else {
        cx.report(Diagnostic::new(codes::CHANGE_ELEMENT_MISSING).with("id", seen.id.as_str()));
        return Ok(None);
    };
    if element.hash(cx)? != seen.hash {
        cx.report(Diagnostic::new(codes::CHANGE_ELEMENT_STALE).with("id", seen.id.as_str()));
        return Ok(None);
    }
    let source = element.source();
    let found = match (source.key(), source.record()) {
        (Some(Key::Instance(instance)), Some(record)) => index
            .found(instance)
            .filter(|found| found.id == seen.id)
            .and_then(|found| Some((found, usize::try_from(record).ok()?))),
        _ => None,
    };
    // 1 つのフィーチャから複数の要素ができたもの（作図グループの展開など）は、1 つだけを書き戻せない
    let Some((found, record)) = found.filter(|(found, _)| targets.insert(found.instance)) else {
        cx.report(unsupported(format, edit, "shared").at(source.clone()));
        return Ok(None);
    };
    Ok(Some(Found { record, ..found }))
}

/// 要素
#[derive(Clone, Copy)]
enum Any<'a> {
    Model(&'a Element<ModelPoint>),
    Paper(&'a Element<PaperPoint>),
}

impl<'a> Any<'a> {
    fn style(&self) -> &'a Style {
        match self {
            Any::Model(element) => &element.style,
            Any::Paper(element) => &element.style,
        }
    }

    fn source(&self) -> &'a Ref {
        match self {
            Any::Model(element) => &element.source,
            Any::Paper(element) => &element.source,
        }
    }

    fn hash(&self, cx: &mut Cx) -> Result<Hash, Error> {
        let result = match self {
            Any::Model(element) => Hash::element_checked(*element, cx),
            Any::Paper(element) => Hash::element_checked(*element, cx),
        };
        result.map_err(|stop| {
            cx.report(stop.to_diagnostic().at(self.source().clone()));
            Error::Stopped(stop)
        })
    }

    fn placed(&self) -> Placed {
        match self {
            Any::Model(element) => Placed::Model((*element).clone()),
            Any::Paper(element) => Placed::Paper((*element).clone()),
        }
    }

    /// 出所を除いて、同じ値か
    fn same(&self, placed: &Placed) -> bool {
        fn same<P: PartialEq>(a: &Element<P>, b: &Element<P>) -> bool {
            a.layer == b.layer && a.style == b.style && a.shape == b.shape
        }
        match (self, placed) {
            (Any::Model(a), Placed::Model(b)) => same(a, b),
            (Any::Paper(a), Placed::Paper(b)) => same(a, b),
            _ => false,
        }
    }

    /// 出所を除いて、円弧の角度のほかは同じ値で、角度が近いか
    fn near(&self, placed: &Placed) -> bool {
        fn near<P: PartialEq>(a: &Element<P>, b: &Element<P>) -> bool {
            let shape = match (&a.shape, &b.shape) {
                (
                    Shape::Curve(Edge::Arc {
                        center,
                        radius,
                        start,
                        end,
                        counterclockwise,
                    }),
                    Shape::Curve(Edge::Arc {
                        center: center_b,
                        radius: radius_b,
                        start: start_b,
                        end: end_b,
                        counterclockwise: counterclockwise_b,
                    }),
                ) => {
                    let angle = |a: f64, b: f64| {
                        let turn = (a - b).rem_euclid(360.0);
                        turn.min(360.0 - turn) <= 1e-9 * a.abs().max(b.abs()).max(1.0)
                    };
                    center == center_b
                        && radius == radius_b
                        && counterclockwise == counterclockwise_b
                        && angle(*start, *start_b)
                        && angle(*end, *end_b)
                }
                (a, b) => a == b,
            };
            a.layer == b.layer && a.style == b.style && shape
        }
        match (self, placed) {
            (Any::Model(a), Placed::Model(b)) => near(a, b),
            (Any::Paper(a), Placed::Paper(b)) => near(a, b),
            _ => false,
        }
    }

    fn restyled(&self, restyle: &zumen_drawing::Restyle) -> (LayerId, Style) {
        match self {
            Any::Model(element) => restyle.over(element.layer, &element.style),
            Any::Paper(element) => restyle.over(element.layer, &element.style),
        }
    }

    /// レイヤとスタイルを変え、形があれば形も変えた要素
    fn with(&self, layer: LayerId, style: Style, shape: Option<Placed>) -> Placed {
        match (self, shape) {
            (Any::Model(element), Some(Placed::Model(new))) => Placed::Model(Element {
                layer,
                style,
                shape: new.shape,
                source: element.source.clone(),
            }),
            (Any::Paper(element), Some(Placed::Paper(new))) => Placed::Paper(Element {
                layer,
                style,
                shape: new.shape,
                source: element.source.clone(),
            }),
            _ => {
                let mut placed = self.placed();
                match &mut placed {
                    Placed::Model(element) => {
                        element.layer = layer;
                        element.style = style;
                    }
                    Placed::Paper(element) => {
                        element.layer = layer;
                        element.style = style;
                    }
                }
                placed
            }
        }
    }

    /// 置き換えた要素。空間か用紙が違えば `None`
    fn replaced(&self, new: &Placed) -> Option<Placed> {
        match (self, new) {
            (Any::Model(_), Placed::Model(element)) => {
                Some(self.with(element.layer, element.style, Some(new.clone())))
            }
            (Any::Paper(_), Placed::Paper(element)) => {
                Some(self.with(element.layer, element.style, Some(new.clone())))
            }
            _ => None,
        }
    }

    /// 変換で動かした要素。形を元の種類で表せなければ `None`
    fn transformed(&self, affine: &zumen_drawing::Affine) -> Option<Placed> {
        Some(match self {
            Any::Model(element) => Placed::Model(Element {
                shape: transform::transform(&element.shape, affine)?,
                ..(*element).clone()
            }),
            Any::Paper(element) => Placed::Paper(Element {
                shape: transform::transform(&element.shape, affine)?,
                ..(*element).clone()
            }),
        })
    }
}

/// 要素とその出所
#[derive(Clone)]
struct Found<'a> {
    id: String,
    element: Any<'a>,
    /// インスタンスの番号
    instance: u64,
    /// ファイルのレコードの番号
    record: usize,
}

impl Found<'_> {
    fn source(&self) -> Ref {
        self.element.source().clone()
    }

    /// 診断に書く種類（フィーチャの名前）
    fn kind(&self, plan: &Plan) -> String {
        plan.record(self.record)
            .map(|record| record.kind.clone())
            .unwrap_or_default()
    }
}

/// ID から要素への索引
struct Index<'a> {
    elements: BTreeMap<String, Any<'a>>,
    /// インスタンスの番号から ID。複数の要素を作った番号は `None`
    instances: BTreeMap<u64, Option<String>>,
    /// 用紙の ID
    sheets: BTreeMap<String, zumen_drawing::SheetId>,
}

impl<'a> Index<'a> {
    fn new(drawing: &'a Drawing2D, cx: &mut Cx) -> Result<Self, Error> {
        let mut index = Index {
            elements: BTreeMap::new(),
            instances: BTreeMap::new(),
            sheets: BTreeMap::new(),
        };
        for space in &drawing.spaces {
            let place = id::space(space.id);
            let mut namer = id::Namer::new(&place);
            for (at, element) in space.elements.iter().enumerate() {
                work(cx, 1, &element.source)?;
                index.note(namer.next(at, element), Any::Model(element));
            }
        }
        let mut definitions = id::DefinitionNamer::default();
        for definition in &drawing.symbols {
            work(cx, 1, &definition.source)?;
            let place = definitions.next(definition);
            let mut namer = id::Namer::new(&place);
            for (at, element) in definition.space.elements.iter().enumerate() {
                work(cx, 1, &element.source)?;
                index.note(namer.next(at, element), Any::Model(element));
            }
        }
        let mut sheets = id::SheetNamer::default();
        for sheet in &drawing.sheets {
            work(cx, 1, &sheet.source)?;
            let place = sheets.next(sheet);
            let mut namer = id::Namer::new(&place);
            for (at, element) in sheet.elements.iter().enumerate() {
                work(cx, 1, &element.source)?;
                index.note(namer.next(at, element), Any::Paper(element));
            }
            index.sheets.insert(place, sheet.id);
        }
        Ok(index)
    }

    fn note(&mut self, id: String, element: Any<'a>) {
        if let Some(Key::Instance(instance)) = element.source().key() {
            self.instances
                .entry(instance)
                .and_modify(|id| *id = None)
                .or_insert_with(|| Some(id.clone()));
        }
        self.elements.insert(id, element);
    }

    /// インスタンスの番号が作った、ただ 1 つの要素
    fn found(&self, instance: u64) -> Option<Found<'a>> {
        let id = self.instances.get(&instance)?.as_ref()?;
        let element = *self.elements.get(id)?;
        Some(Found {
            id: id.clone(),
            element,
            instance,
            record: 0,
        })
    }
}

trait OnSheet {
    fn shape_sheet(&self) -> zumen_drawing::SheetId;
}

impl OnSheet for Element<PaperPoint> {
    /// 形の点が置かれた用紙。点がなければ、図面の用紙
    fn shape_sheet(&self) -> zumen_drawing::SheetId {
        let point = match &self.shape {
            Shape::Line { start, .. } => Some(start),
            Shape::Circle { center, .. } | Shape::Curve(Edge::Arc { center, .. }) => Some(center),
            Shape::Polyline(line) => line.vertices.first().map(|vertex| &vertex.point),
            _ => None,
        };
        point.map_or(super::SHEET, |point| point.sheet)
    }
}

/// 表のコード。レイヤ・利用者定義線種は変換した図面モデルから、利用者定義色・線幅はフィーチャから数える
struct Tables {
    layers: BTreeSet<u32>,
    line_types: BTreeSet<u32>,
    /// 利用者定義色の RGB。コードは 17 から
    colors: Vec<Option<[u8; 3]>>,
    /// 利用者定義線幅（mm）。コードは 11 から
    widths: Vec<Option<f64>>,
}

impl Tables {
    /// 利用者定義色と線幅を順に探す作業。変わらない欄は探さない。
    fn style_work(&self, style: &Style, old: Option<&Style>) -> u64 {
        let color = old.is_none_or(|old| old.color != style.color)
            && matches!(
                style.color,
                Inherit::Own(Color {
                    code: None,
                    rgb: Some(_)
                })
            );
        let width = old.is_none_or(|old| old.line_weight != style.line_weight)
            && matches!(style.line_weight, Some(Inherit::Own(LineWeight::Width(PaperLength(mm)))) if !WIDTHS.contains(&mm));
        (if color { self.colors.len() as u64 } else { 0 }).saturating_add(if width {
            self.widths.len() as u64
        } else {
            0
        })
    }

    fn new(records: &[FeatureRecord], drawing: &Drawing2D, cx: &mut Cx) -> Result<Self, Error> {
        let mut colors = Vec::new();
        let mut widths = Vec::new();
        for record in records {
            work(cx, 1, &Ref::default())?;
            let args = Args {
                values: &record.args,
            };
            // 変換と同じく、読めない定義もコードを数える
            if record.name.eq_ignore_ascii_case("user_defined_colour") {
                let channel = |at| args.int(at).and_then(|value| u8::try_from(value).ok());
                colors.push((|| Some([channel(0)?, channel(1)?, channel(2)?]))());
            } else if record.name.eq_ignore_ascii_case("width") {
                widths.push(args.num(0).filter(|value| *value > 0.0));
            }
        }
        Ok(Tables {
            layers: drawing
                .layers
                .iter()
                .filter(|layer| layer.state.is_some())
                .map(|layer| layer.id.0)
                .collect(),
            line_types: drawing
                .line_types
                .iter()
                .filter(|line_type| i64::from(line_type.id.0) > USER_LINE_TYPE)
                .filter(|line_type| line_type.source != Ref::default())
                .map(|line_type| line_type.id.0)
                .collect(),
            colors,
            widths,
        })
    }

    fn layer(&self, layer: LayerId) -> Option<i64> {
        self.layers.contains(&layer.0).then_some(i64::from(layer.0))
    }

    /// 色のコードと、そのコードを読み直したときの色。利用者定義色は、RGB が合うときだけ
    fn color(&self, color: Inherit<Color>) -> Option<(i64, Color)> {
        let Inherit::Own(color) = color else {
            return None;
        };
        let user = |code: i64| {
            let at = usize::try_from(code.checked_sub(PREDEFINED_COLORS + 1)?).ok()?;
            self.colors.get(at).copied().flatten()
        };
        let code = match color.code {
            Some(ColorCode {
                system: ColorSystem::Sxf,
                value,
            }) => i64::from(value),
            Some(_) => return None,
            // RGB だけなら、同じ RGB の利用者定義色を探す
            None => {
                let rgb = color.rgb?;
                let at = self.colors.iter().position(|color| *color == Some(rgb))?;
                PREDEFINED_COLORS + 1 + i64::try_from(at).ok()?
            }
        };
        // 既定義色の RGB は、変換が表から引かない
        let rgb = if (1..=PREDEFINED_COLORS).contains(&code) {
            None
        } else {
            Some(user(code)?)
        };
        if color.rgb.is_some() && color.rgb != rgb {
            return None;
        }
        let read = Color {
            code: Some(ColorCode {
                system: ColorSystem::Sxf,
                value: u32::try_from(code).ok()?,
            }),
            rgb,
        };
        Some((code, read))
    }

    fn line_type(&self, line_type: Inherit<LineTypeId>) -> Option<i64> {
        let Inherit::Own(LineTypeId(code)) = line_type else {
            return None;
        };
        let predefined =
            usize::try_from(code).is_ok_and(|code| (1..=LINE_TYPES.len()).contains(&code));
        (predefined || self.line_types.contains(&code)).then_some(i64::from(code))
    }

    fn width(&self, weight: Inherit<LineWeight>) -> Option<i64> {
        let Inherit::Own(LineWeight::Width(PaperLength(mm))) = weight else {
            return None;
        };
        if let Some(at) = WIDTHS.iter().position(|width| *width == mm) {
            return i64::try_from(at + 1).ok();
        }
        let at = self.widths.iter().position(|width| *width == Some(mm))?;
        USER_WIDTH.checked_add(i64::try_from(at).ok()?)
    }
}

/// 複合曲線がまとめるフィーチャのレコードの番号。変換（`Lifter::run`）と同じ順で、前の定義の後のフィーチャを数える
fn grouped(records: &[FeatureRecord], cx: &mut Cx) -> Result<BTreeSet<usize>, Error> {
    const TABLES: [&str; 8] = [
        "layer",
        "user_defined_colour",
        "user_defined_font",
        "width",
        "text_font",
        "pre_defined_colour",
        "pre_defined_font",
        "drawing_attribute",
    ];
    let mut pending: Vec<usize> = Vec::new();
    let mut grouped = BTreeSet::new();
    for (index, record) in records.iter().enumerate() {
        work(cx, 1, &Ref::default())?;
        let is = |name: &str| record.name.eq_ignore_ascii_case(name);
        if TABLES.iter().any(|name| is(name)) {
            continue;
        }
        if is("drawing_sheet") || is("sfig_org") {
            pending.clear();
        } else if is("composite_curve_org") {
            // 複合曲線自身は変更できないので保留せず、各構成要素を一度だけ処理する。
            for at in pending.drain(..) {
                if let Some(member) = records.get(at) {
                    grouped.insert(member.record);
                }
            }
        } else {
            pending.push(index);
        }
    }
    Ok(grouped)
}

/// 書き方。SFC はフィーチャのコメントを、P21 は実体を書き直す
trait Writer {
    /// レイヤ・色・線種・線幅のうち、変わったもののコード（引数の 0〜3）を書く
    fn codes(&mut self, record: usize, codes: &[(usize, i64)]) -> Option<()>;
    /// 形を書く。種類は変えない
    fn shape(&mut self, record: usize, args: &Arguments) -> Option<()>;
    /// 要素を除く
    fn remove(&mut self, record: usize) -> Option<()>;
    /// 用紙に要素を足し、足したインスタンスの番号を返す。引数は、レイヤ・色・線種・線幅のコードも持つ
    fn add(&mut self, args: &Arguments) -> Option<u64>;
    /// 書き直したバイト列
    fn bytes(self: Box<Self>, src: &[u8], cx: &mut Cx) -> Result<Vec<u8>, Error>;
}

/// 元のバイト列への書き直し
struct Plan<'f> {
    /// レコードの番号から、そのフィーチャ
    records: BTreeMap<usize, &'f FeatureRecord>,
    writer: Box<dyn Writer + 'f>,
}

impl<'f> Plan<'f> {
    fn new(records: &'f [FeatureRecord], writer: Box<dyn Writer + 'f>) -> Self {
        Plan {
            records: records
                .iter()
                .map(|record| (record.record, record))
                .collect(),
            writer,
        }
    }

    fn record(&self, record: usize) -> Option<&'f FeatureRecord> {
        self.records.get(&record).copied()
    }

    fn remove(&mut self, record: usize) -> Option<()> {
        self.writer.remove(record)
    }

    /// レイヤとスタイルのうち、変わったもののコードを書く
    fn styled(
        &mut self,
        found: &Found,
        element: &mut Placed,
        tables: &Tables,
        grouped: &BTreeSet<usize>,
    ) -> Option<()> {
        let name = self.record(found.record)?.name.to_ascii_lowercase();
        if !STYLED.contains(&name.as_str()) || grouped.contains(&found.record) {
            return None;
        }
        let (layer, style) = match element {
            Placed::Model(element) => (element.layer, &mut element.style),
            Placed::Paper(element) => (element.layer, &mut element.style),
        };
        let (was_layer, was) = match found.element {
            Any::Model(element) => (element.layer, element.style),
            Any::Paper(element) => (element.layer, element.style),
        };
        if style.line_type_scale != 1.0 {
            return None;
        }
        let mut codes = Vec::new();
        if layer != was_layer {
            codes.push((0, tables.layer(layer)?));
        }
        if style.color != was.color {
            let (code, color) = tables.color(style.color)?;
            style.color = Inherit::Own(color);
            codes.push((1, code));
        }
        if style.line_type != was.line_type {
            codes.push((2, tables.line_type(style.line_type)?));
        }
        if style.line_weight != was.line_weight {
            codes.push((3, tables.width(style.line_weight?)?));
        }
        if codes.is_empty() {
            return Some(());
        }
        self.writer.codes(found.record, &codes)
    }

    /// 形を書く。種類は変えない
    fn shaped(&mut self, found: &Found, element: &Placed, grouped: &BTreeSet<usize>) -> Option<()> {
        let name = self.record(found.record)?.name.to_ascii_lowercase();
        if !SHAPED.contains(&name.as_str()) || grouped.contains(&found.record) {
            return None;
        }
        let args = match element {
            Placed::Model(element) => arguments(&element.shape)?,
            Placed::Paper(element) => arguments(&element.shape)?,
        };
        if args.name != name {
            return None;
        }
        self.writer.shape(found.record, &args)
    }

    /// 用紙に要素を足す。足したインスタンスの番号と、読み直したときの要素を返す
    fn add(
        &mut self,
        element: &Element<PaperPoint>,
        tables: &Tables,
    ) -> Option<(u64, Element<PaperPoint>)> {
        let mut element = element.clone();
        let style = element.style;
        if style.line_type_scale != 1.0 {
            return None;
        }
        let mut args = arguments(&element.shape)?;
        let (color, read) = tables.color(style.color)?;
        element.style.color = Inherit::Own(read);
        let codes = [
            tables.layer(element.layer)?,
            color,
            tables.line_type(style.line_type)?,
            tables.width(style.line_weight?)?,
        ];
        for (at, code) in codes.iter().enumerate() {
            *args.values.get_mut(at)? = code.to_string();
        }
        let instance = self.writer.add(&args)?;
        Some((instance, element))
    }

    fn bytes(self, src: &[u8], cx: &mut Cx) -> Result<Vec<u8>, Error> {
        self.writer.bytes(src, cx)
    }
}

/// SFC の書き方。引数の値の範囲を書き直し、フィーチャのコメントを除くか足す
struct Sfc<'f> {
    file: &'f SxfFile,
    /// 書き直す範囲と、新しいバイト列。足すものは空の範囲
    patches: Vec<(Range<usize>, Vec<u8>)>,
    /// 次に足すインスタンスの番号
    next: u64,
    /// 足す所と、その印。用紙のフィーチャの前か、最後のフィーチャの後
    insert: Option<(usize, Marker, bool)>,
    newline: &'static str,
}

impl<'f> Sfc<'f> {
    fn new(file: &'f SxfFile, records: &[FeatureRecord]) -> Self {
        let next = file
            .records()
            .iter()
            .filter_map(|record| record.id())
            .max()
            .unwrap_or(0)
            .saturating_add(1);
        let sheet = records
            .iter()
            .find(|record| record.name.eq_ignore_ascii_case("drawing_sheet"))
            .and_then(|record| file.records().get(record.record))
            .and_then(|record| Some((record.span().start, record.marker()?, true)));
        let last = records
            .last()
            .and_then(|record| file.records().get(record.record))
            .and_then(|record| Some((record.span().end, record.marker()?, false)));
        Sfc {
            file,
            patches: Vec::new(),
            next,
            insert: sheet.or(last),
            newline: newline(&file.src),
        }
    }

    /// 引数の値の範囲を書き直す。囲まない値か、`'…'` で囲んだ値だけ
    fn argument(&mut self, record: usize, at: usize, text: &str) -> Option<()> {
        let part = self.file.records().get(record)?.parts().first()?;
        let range = match part.args().get(at)? {
            Value::Text {
                raw,
                quote: Quote::Plain,
            }
            | Value::Token(raw) => raw.clone(),
            _ => return None,
        };
        self.patches.push((range, text.as_bytes().to_vec()));
        Some(())
    }
}

impl Writer for Sfc<'_> {
    fn codes(&mut self, record: usize, codes: &[(usize, i64)]) -> Option<()> {
        for (at, code) in codes {
            self.argument(record, *at, &code.to_string())?;
        }
        Some(())
    }

    fn shape(&mut self, record: usize, args: &Arguments) -> Option<()> {
        for (at, text) in args.values.iter().enumerate().skip(4) {
            self.argument(record, at, text)?;
        }
        Some(())
    }

    /// フィーチャのコメントを、後の改行とともに除く
    fn remove(&mut self, record: usize) -> Option<()> {
        let span = self.file.records().get(record).map(crate::Record::span)?;
        self.patches
            .push((span.start..line_end(&self.file.src, span.end), Vec::new()));
        Some(())
    }

    fn add(&mut self, args: &Arguments) -> Option<u64> {
        let (at, marker, before) = self.insert?;
        let (_, open, close) = Marker::ALL
            .iter()
            .find(|(found, _, _)| *found == marker)
            .copied()?;
        let instance = self.next;
        self.next = self.next.checked_add(1)?;
        let nl = self.newline;
        let values: Vec<String> = args
            .values
            .iter()
            .map(|value| format!("'{value}'"))
            .collect();
        let feature = format!(
            "{open}{nl}#{instance} = {}_feature({}){nl}{close}",
            args.name,
            values.join(","),
            open = String::from_utf8_lossy(open),
            close = String::from_utf8_lossy(close),
        );
        let text = if before {
            format!("{feature}{nl}")
        } else {
            format!("{nl}{feature}")
        };
        self.patches.push((at..at, text.into_bytes()));
        Some(instance)
    }

    fn bytes(self: Box<Self>, src: &[u8], cx: &mut Cx) -> Result<Vec<u8>, Error> {
        patched(self.patches, src, cx)
    }
}

/// ファイルの改行
fn newline(src: &[u8]) -> &'static str {
    if src.windows(2).any(|pair| pair == b"\r\n") {
        "\r\n"
    } else {
        "\n"
    }
}

/// 範囲の終わりの後に改行があれば、その後の位置
fn line_end(src: &[u8], end: usize) -> usize {
    let rest = src.get(end..).unwrap_or_default();
    end + if rest.starts_with(b"\r\n") {
        2
    } else {
        usize::from(rest.starts_with(b"\n"))
    }
}

/// 範囲を書き直したバイト列。範囲は重ならない
fn patched(
    mut patches: Vec<(Range<usize>, Vec<u8>)>,
    src: &[u8],
    cx: &mut Cx,
) -> Result<Vec<u8>, Error> {
    let added = patches.iter().fold(0u64, |total, (_, bytes)| {
        total.saturating_add(bytes.len() as u64)
    });
    work(
        cx,
        (src.len() as u64).saturating_add(added).div_ceil(1024),
        &Ref::default(),
    )?;
    patches.sort_by_key(|(range, _)| range.start);
    let mut out = Vec::with_capacity(src.len());
    let mut end = 0;
    for (range, bytes) in &patches {
        work(cx, 1, &Ref::default())?;
        out.extend_from_slice(src.get(end..range.start).unwrap_or_default());
        out.extend_from_slice(bytes);
        end = range.end;
    }
    out.extend_from_slice(src.get(end..).unwrap_or_default());
    Ok(out)
}

/// フィーチャの名前と引数。レイヤ・色・線種・線幅（0〜3）は空にしておく
struct Arguments {
    name: &'static str,
    values: Vec<String>,
}

/// 形を書く引数。数は、読み直すと同じ値になる最短の 10 進で書く
fn arguments<P: transform::Point>(shape: &Shape<P>) -> Option<Arguments> {
    let number = |value: f64| value.is_finite().then(|| format!("{value}"));
    let point = |point: &P| {
        let [x, y] = point.xy();
        Some([number(x)?, number(y)?])
    };
    let length = |length: &Length| match length {
        Length::Paper(PaperLength(value)) => number(*value),
        Length::Model(length) => number(length.value),
    };
    let mut values = vec![String::new(); 4];
    let name = match shape {
        Shape::Line { start, end } => {
            values.extend(point(start)?);
            values.extend(point(end)?);
            "line"
        }
        Shape::Circle { center, radius } => {
            values.extend(point(center)?);
            values.push(length(radius)?);
            "circle"
        }
        Shape::Curve(Edge::Arc {
            center,
            radius,
            start,
            end,
            counterclockwise,
        }) => {
            values.extend(point(center)?);
            values.push(length(radius)?);
            // 向きのフラグは 0 が反時計回り
            values.push(if *counterclockwise { "0" } else { "1" }.to_owned());
            values.push(number(*start)?);
            values.push(number(*end)?);
            "arc"
        }
        Shape::Polyline(Polyline {
            vertices,
            closed: false,
            width: None,
            vertex_widths,
        }) if vertex_widths.is_empty()
            && vertices.len() >= 2
            && vertices.iter().all(|vertex| vertex.bulge == 0.0) =>
        {
            let mut xs = Vec::new();
            let mut ys = Vec::new();
            for vertex in vertices {
                let [x, y] = point(&vertex.point)?;
                xs.push(x);
                ys.push(y);
            }
            values.push(vertices.len().to_string());
            values.push(format!("({})", xs.join(",")));
            values.push(format!("({})", ys.join(",")));
            "polyline"
        }
        _ => return None,
    };
    Some(Arguments { name, values })
}
