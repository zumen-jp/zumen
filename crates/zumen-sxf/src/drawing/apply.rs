//! 図面モデルの変更を、SFC のフィーチャのコメントに書き戻す（決定 0167）。
//!
//! 変更は、元のバイト列のうち、引数の値の範囲を書き直すか、フィーチャのコメントを除くか足すかで当てる。ほかのバイトは
//! 写す。当てた後に読み直して図面モデルに変換し、変えた要素が求めた値になり、ほかの要素が ID も値も変わらないことを
//! 確かめる。合わなければ、何も変えずに断る。

use core::ops::Range;
use std::collections::{BTreeMap, BTreeSet};

use zumen_base::{Diagnostic, Diagnostics, Key, Kind, Ref, codes};
use zumen_drawing::{
    Applied, Changes, Color, ColorCode, ColorSystem, Drawing2D, Edge, Edit, Element, Encode, Hash,
    Inherit, Layer, LayerId, Length, LineTypeId, LineWeight, ModelPoint, PaperLength, PaperPoint,
    Placed, Point, PointLook, Polyline, Seen, Shape, Style, id, transform,
};
use zumen_sdk::{Apply, Codec, Cx, Error, Lift, quietly};

mod layers;
pub(super) mod p21;
mod text;

use super::{
    Args, FeatureRecord, LINE_TYPES, PREDEFINED_COLORS, USER_LINE_TYPE, USER_WIDTH, WIDTHS,
    sfc_records, work,
};
use crate::p21::records_and_codes as p21_records_and_codes;
use crate::{Marker, Mode, Quote, SfcRecord, Sxf, SxfFile, Value};

/// レイヤ・色・線種・線幅のコードを引数の 0〜3 に持ち、スタイルを変えられるフィーチャ
pub(super) const STYLED: [&str; 8] = [
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
pub(super) const SHAPED: [&str; 4] = ["line", "polyline", "circle", "arc"];

/// 点マーカー。レイヤと色のコードだけを引数の 0〜1 に持つ。SFC（決定 0183）と P21（決定 0190）で書き直し、足す
pub(super) const MARKER: &str = "point_marker";

/// 除けるフィーチャ。複合曲線と複合図形の定義・配置は、ほかのフィーチャが順の番号や名前で指すので除かない
pub(super) const REMOVABLE: [&str; 20] = [
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
        let (updated, applied) = self.applied(changes, cx)?;
        *self = updated;
        Ok(applied)
    }
}

impl SxfFile {
    /// 元のファイルを複製せずに変更を当て、書き戻した後のファイルと変更の結果を返す。
    pub fn applied(
        &self,
        changes: &Changes,
        cx: &mut Cx,
    ) -> Result<(SxfFile, Vec<Applied>), Error> {
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
        let (records, codes, drawing) = {
            let mut lift_cx = Cx::new(cx.budget, &mut quiet, cx.cancel);
            lift_cx.with_diagnostics_in_record_order(|cx| {
                let (records, codes) = match self.mode() {
                    Mode::Sfc => (sfc_records(self), None),
                    Mode::P21 => {
                        let (records, codes) = p21_records_and_codes(self, cx)?;
                        (records, Some(codes))
                    }
                };
                let drawing = self.lift_from_records(&records, None, cx)?;
                Ok::<_, Error>((records, codes, drawing))
            })?
        };
        let need_ids = changes.edits.iter().any(|change| {
            matches!(
                change,
                Edit::Remove { .. }
                    | Edit::Restyle { .. }
                    | Edit::Replace { .. }
                    | Edit::Transform { .. }
            )
        });
        let before = Index::new(&drawing, need_ids, cx)?;
        let before_layers = &drawing.layers;
        let mut layer_edits = layers::Changes::default();
        let tables = Tables::new(&records, &drawing, cx)?;
        let grouped = grouped(&records, cx)?;
        // 改行の種類を調べるため、元のバイト列を走査する。
        work(cx, (self.src.len() as u64).div_ceil(1024), &Ref::default())?;
        let writer: Box<dyn Writer> = match (self.mode(), codes) {
            (Mode::Sfc, _) => Box::new(Sfc::new(self, &records)),
            (Mode::P21, Some(codes)) => {
                Box::new(p21::P21::new(self, codes, tables.widths.clone(), cx)?)
            }
            (Mode::P21, None) => return Err(Error::Rejected),
        };
        let mut plan = Plan::new(&records, self.records().len(), writer);
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
                    let result = if removed.is_some() {
                        plan.remove(found.record, cx)?
                    } else {
                        None
                    };
                    match result {
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
                    found.element.count_copy(cx, 1)?;
                    let mut element = found.element.with(layer, new, None);
                    plan.styled(&found, &mut element, &tables, &grouped, false, cx)?
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
                    match element.as_ref() {
                        Placed::Model(element) => count_element_copy(element, cx, 1)?,
                        Placed::Paper(element) => count_element_copy(element, cx, 1)?,
                    }
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
                    let element = if let Some(mut element) = element {
                        if plan
                            .styled(&found, &mut element, &tables, &grouped, true, cx)?
                            .is_some()
                            && plan.shaped(&found, &mut element, &grouped, cx)?.is_some()
                        {
                            Some(Expected::Element(found.instance, Box::new(element)))
                        } else {
                            None
                        }
                    } else {
                        None
                    };
                    element.or_else(|| {
                        cx.report(
                            unsupported(format, "replace", &found.kind(&plan)).at(found.source()),
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
                    found.element.count_copy(cx, 2)?;
                    let element = found.element.transformed(transform);
                    let element = if let Some(mut element) = element {
                        plan.shaped(&found, &mut element, &grouped, cx)?
                            .map(|()| Expected::Element(found.instance, Box::new(element)))
                    } else {
                        None
                    };
                    element.or_else(|| {
                        cx.report(
                            unsupported(format, "transform", &found.kind(&plan)).at(found.source()),
                        );
                        None
                    })
                }
                Edit::Add { place, element } => {
                    // SXF のフィーチャは色・線種・線幅を自分で持ち、レイヤかブロックに従う値を持たないので、書かない値は
                    // 何を書けばよいかが分かる診断で断る（決定 0292）
                    let mut unset = None;
                    let added = match element.as_ref() {
                        Placed::Paper(element)
                            if before.sheets.get(place) == Some(&element.shape_sheet()) =>
                        {
                            work(cx, tables.style_work(&element.style, None), &Ref::default())?;
                            plan.add(element, &tables, &mut unset, cx)?
                        }
                        _ => None,
                    };
                    if let Some(field) = unset {
                        cx.report(
                            Diagnostic::new(codes::CHANGE_STYLE_UNSUPPORTED)
                                .with("field", field)
                                .with("format", format),
                        );
                        rejected = true;
                        continue;
                    }
                    added
                        .map(|(instance, element)| {
                            Expected::Element(instance, Box::new(Placed::Paper(element)))
                        })
                        .or_else(|| {
                            cx.report(unsupported(format, "add", kind_of(element)));
                            None
                        })
                }
                Edit::ReplaceLayer { target, layer } => layer_edits
                    .replace(
                        before_layers,
                        target,
                        layer,
                        plan.writer.as_mut(),
                        format,
                        cx,
                    )?
                    .map(Expected::Layer),
                Edit::AddLayer { layer } => layer_edits
                    .add(before_layers, layer, plan.writer.as_mut(), format, cx)?
                    .map(Expected::Layer),
                Edit::RemoveLayer { .. } => {
                    cx.report(unsupported(format, change.name(), "layer_feature"));
                    None
                }
                Edit::ReplaceTextStyle { .. }
                | Edit::AddTextStyle { .. }
                | Edit::RemoveTextStyle { .. } => {
                    cx.report(unsupported(format, change.name(), "text_font_feature"));
                    None
                }
                Edit::ReplaceLineType { .. }
                | Edit::AddLineType { .. }
                | Edit::RemoveLineType { .. } => {
                    cx.report(unsupported(
                        format,
                        change.name(),
                        "user_defined_font_feature",
                    ));
                    None
                }
                Edit::ReplaceDimensionStyle { .. }
                | Edit::AddDimensionStyle { .. }
                | Edit::RemoveDimensionStyle { .. } => {
                    cx.report(unsupported(format, change.name(), "dimension_style"));
                    None
                }
                // SXF は文書の点の表し方を持たない。点は既定義のマーカーだけ（決定 0190）
                Edit::SetPointStyle { .. } => {
                    cx.report(unsupported(format, change.name(), "point_style"));
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
        let estimated_texts = core::mem::take(&mut plan.estimated_texts);
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
        let after = Index::new(&drawing, false, cx)?;
        let mut applied = Vec::new();
        for (edit, expected) in &expected {
            work(cx, 1, &Ref::default())?;
            let found = match expected {
                Expected::Layer(expected) => {
                    let found = drawing.layers.iter().find(|layer| layer.id == expected.id);
                    let same = if let Some(found) = found {
                        layers::hash(found, cx)? == layers::hash(expected, cx)?
                    } else {
                        false
                    };
                    if !same {
                        cx.report(unsupported(format, edit, "round_trip"));
                        return Err(Error::Rejected);
                    }
                    applied.push(Applied {
                        id: Some(id::layer(expected)),
                        hash: Some(layers::hash(expected, cx)?),
                    });
                    continue;
                }
                Expected::Removed(instance) => {
                    if after.has_instance(*instance) {
                        cx.report(unsupported(format, edit, "round_trip"));
                        return Err(Error::Rejected);
                    }
                    applied.push(Applied {
                        id: None,
                        hash: None,
                    });
                    continue;
                }
                Expected::Element(instance, element) => {
                    if let Some(found) = after.found(*instance) {
                        found.element.count_copy(cx, 2)?;
                        // P21 の円弧と点マーカーの角度は、ラジアンか向きで書いて度で読むので、最後の桁が違いうる
                        (found.element.same(element)
                            || (self.mode() == Mode::P21 && found.element.near(element)))
                        .then_some(found)
                    } else {
                        None
                    }
                }
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
            .filter_map(|(_, expected)| match expected {
                Expected::Removed(instance) | Expected::Element(instance, _) => Some(*instance),
                Expected::Layer(_) => None,
            })
            .collect();
        for &(instance, slot) in &before.instances {
            work(cx, 1, &Ref::default())?;
            if changed.contains(&instance) {
                continue;
            }
            let kept = match (slot, after.instance_slot(instance)) {
                (Some(was_idx), Some(Some(now_idx))) => {
                    if let (Some((was_id, was)), Some((now_id, now))) =
                        (before.entries.get(was_idx), after.entries.get(now_idx))
                    {
                        was.count_compare(cx)?;
                        now_id == was_id && now.same_any(was)
                    } else {
                        false
                    }
                }
                (None, Some(None)) => true,
                _ => false,
            };
            if !kept {
                let at = slot
                    .and_then(|idx| before.entries.get(idx))
                    .map(|(_, elem)| elem.source().clone());
                cx.report(unsupported(format, "apply", "round_trip").at(at.unwrap_or_default()));
                return Err(Error::Rejected);
            }
        }
        if !layer_edits.verify(before_layers, &drawing.layers, cx)? {
            cx.report(unsupported(format, "apply", "round_trip"));
            return Err(Error::Rejected);
        }
        for instance in estimated_texts {
            if let Some(found) = after.found(instance) {
                cx.report(Diagnostic::new(codes::SXF_TEXT_WIDTH_APPROXIMATED).at(found.source()));
            }
        }
        Ok((updated, applied))
    }
}
/// 当てた後に確かめること
enum Expected {
    /// この定義のレイヤが、この値になる
    Layer(Layer),
    /// このインスタンスの要素がなくなる
    Removed(u64),
    /// このインスタンスの要素が、この値になる
    Element(u64, Box<Placed>),
}
/// 足す要素のうち、書いていない（レイヤかブロックに従う）ので書き戻せない色・線種・線幅のフィールドの名前。点マーカー
/// （`marker`）は線種と線幅を持たないので、色だけを見る
pub(super) fn unset_field(style: &Style, marker: bool) -> Option<&'static str> {
    if !matches!(style.color, Inherit::Own(_)) {
        return Some("color");
    }
    if marker {
        return None;
    }
    if !matches!(style.line_type, Inherit::Own(_)) {
        return Some("line_type");
    }
    (!matches!(style.line_weight, Some(Inherit::Own(_)))).then_some("line_weight")
}

/// 当てられない変更の診断
pub(super) fn unsupported(format: &str, edit: &str, kind: &str) -> Diagnostic {
    Diagnostic::new(codes::CHANGE_EDIT_UNSUPPORTED)
        .with("edit", edit)
        .with("kind", kind)
        .with("format", format)
}

/// 足す要素のフィーチャの種類
pub(super) fn kind_of(element: &Placed) -> &'static str {
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
        Shape::Point(_) => "point_marker_feature",
        Shape::Text(_) | Shape::Formatted(_) => "text_string_feature",
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
    let Some(element) = index.element(&seen.id) else {
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

    fn count_copy(&self, cx: &mut Cx, copies: u64) -> Result<(), Error> {
        match self {
            Any::Model(element) => count_element_copy(element, cx, copies),
            Any::Paper(element) => count_element_copy(element, cx, copies),
        }
    }

    fn count_compare(&self, cx: &mut Cx) -> Result<(), Error> {
        match self {
            Any::Model(element) => count_element_compare(element, cx),
            Any::Paper(element) => count_element_compare(element, cx),
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

    /// 複製を作らずに、出所を除いて同じ値かを比べる
    fn same_any(&self, other: &Any<'_>) -> bool {
        fn same<P: PartialEq>(a: &Element<P>, b: &Element<P>) -> bool {
            a.layer == b.layer && a.style == b.style && a.shape == b.shape
        }
        match (self, other) {
            (Any::Model(a), Any::Model(b)) => same(a, b),
            (Any::Paper(a), Any::Paper(b)) => same(a, b),
            _ => false,
        }
    }

    /// 出所を除いて、円弧の角度のほかは同じ値で、角度が近いか
    fn near(&self, placed: &Placed) -> bool {
        fn near<P: transform::Point + PartialEq>(a: &Element<P>, b: &Element<P>) -> bool {
            let angle = |a: f64, b: f64| {
                let turn = (a - b).rem_euclid(360.0);
                turn.min(360.0 - turn) <= 1e-9 * a.abs().max(b.abs()).max(1.0)
            };
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
                    center == center_b
                        && radius == radius_b
                        && counterclockwise == counterclockwise_b
                        && angle(*start, *start_b)
                        && angle(*end, *end_b)
                }
                // P21 の点マーカーの回転は、向きの成分で書いて角度で読むので、最後の桁が違いうる（決定 0190）
                (Shape::Point(a), Shape::Point(b)) => {
                    a.position == b.position
                        && a.look == b.look
                        && a.construction == b.construction
                        && angle(a.rotation, b.rotation)
                }
                (Shape::Formatted(a), Shape::Formatted(b)) => text::near(a, b),
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
    entries: Vec<(String, Any<'a>)>,
    by_id: BTreeMap<String, usize>,
    /// インスタンスの番号から `entries` の位置。0 は未出現、`u32::MAX` は複数の要素を作った番号、それ以外は `idx + 1`
    instance_dense: Vec<u32>,
    instance_sparse: BTreeMap<u64, Option<usize>>,
    /// 昇順に並べたインスタンスの番号と `entries` の位置（複数の要素を作った番号は `None`）
    instances: Vec<(u64, Option<usize>)>,
    /// 用紙の ID
    sheets: BTreeMap<String, zumen_drawing::SheetId>,
}

impl<'a> Index<'a> {
    fn new(drawing: &'a Drawing2D, with_ids: bool, cx: &mut Cx) -> Result<Self, Error> {
        let total_elements: usize = drawing
            .spaces
            .iter()
            .map(|space| space.elements.len())
            .sum::<usize>()
            .saturating_add(
                drawing
                    .symbols
                    .iter()
                    .map(|symbol| symbol.space.elements.len())
                    .sum::<usize>(),
            )
            .saturating_add(
                drawing
                    .sheets
                    .iter()
                    .map(|sheet| sheet.elements.len())
                    .sum::<usize>(),
            );
        let dense_cap = total_elements.saturating_mul(8).saturating_add(64);
        let mut index = Index {
            entries: Vec::with_capacity(total_elements),
            by_id: BTreeMap::new(),
            instance_dense: vec![0; dense_cap],
            instance_sparse: BTreeMap::new(),
            instances: Vec::with_capacity(total_elements),
            sheets: BTreeMap::new(),
        };
        for space in &drawing.spaces {
            let place = id::space(space.id);
            let mut namer = id::Namer::new(&place);
            for (at, element) in space.elements.iter().enumerate() {
                work(cx, 1, &element.source)?;
                index.note(namer.next(at, element), Any::Model(element), with_ids);
            }
        }
        let mut definitions = id::DefinitionNamer::default();
        for definition in &drawing.symbols {
            work(cx, 1, &definition.source)?;
            let place = definitions.next(definition);
            let mut namer = id::Namer::new(&place);
            for (at, element) in definition.space.elements.iter().enumerate() {
                work(cx, 1, &element.source)?;
                index.note(namer.next(at, element), Any::Model(element), with_ids);
            }
        }
        let mut sheets = id::SheetNamer::default();
        for sheet in &drawing.sheets {
            work(cx, 1, &sheet.source)?;
            let place = sheets.next(sheet);
            let mut namer = id::Namer::new(&place);
            for (at, element) in sheet.elements.iter().enumerate() {
                work(cx, 1, &element.source)?;
                index.note(namer.next(at, element), Any::Paper(element), with_ids);
            }
            index.sheets.insert(place, sheet.id);
        }
        for (id, &raw) in index.instance_dense.iter().enumerate() {
            if raw == 0 {
                continue;
            }
            let slot = (raw != u32::MAX).then(|| (raw - 1) as usize);
            index.instances.push((id as u64, slot));
        }
        for (&id, &slot) in &index.instance_sparse {
            index.instances.push((id, slot));
        }
        Ok(index)
    }

    fn note(&mut self, id: String, element: Any<'a>, with_ids: bool) {
        let idx = self.entries.len();
        if let Some(Key::Instance(instance)) = element.source().key() {
            if let Ok(dense_id) = usize::try_from(instance)
                && let Some(slot) = self.instance_dense.get_mut(dense_id)
            {
                if *slot == 0 {
                    *slot = u32::try_from(idx + 1).unwrap_or(u32::MAX);
                } else {
                    *slot = u32::MAX;
                }
            } else {
                self.instance_sparse
                    .entry(instance)
                    .and_modify(|slot| *slot = None)
                    .or_insert(Some(idx));
            }
        }
        if with_ids {
            self.by_id.insert(id.clone(), idx);
        }
        self.entries.push((id, element));
    }

    fn element(&self, id: &str) -> Option<Any<'a>> {
        let &idx = self.by_id.get(id)?;
        Some(self.entries.get(idx)?.1)
    }

    fn instance_slot(&self, instance: u64) -> Option<Option<usize>> {
        if let Ok(dense_id) = usize::try_from(instance)
            && let Some(&raw) = self.instance_dense.get(dense_id)
        {
            if raw == 0 {
                None
            } else if raw == u32::MAX {
                Some(None)
            } else {
                Some(Some((raw - 1) as usize))
            }
        } else {
            self.instance_sparse.get(&instance).copied()
        }
    }

    fn has_instance(&self, instance: u64) -> bool {
        self.instance_slot(instance).is_some()
    }

    /// インスタンスの番号が作った、ただ 1 つの要素
    fn found(&self, instance: u64) -> Option<Found<'a>> {
        let idx = self.instance_slot(instance)??;
        let (id, element) = self.entries.get(idx)?;
        Some(Found {
            id: id.clone(),
            element: *element,
            instance,
            record: 0,
        })
    }
}

pub(super) fn placed_same(found: &Placed, expected: &Placed) -> bool {
    match found {
        Placed::Model(elem) => Any::Model(elem).same(expected),
        Placed::Paper(elem) => Any::Paper(elem).same(expected),
    }
}

pub(super) fn placed_near(found: &Placed, expected: &Placed) -> bool {
    match found {
        Placed::Model(elem) => Any::Model(elem).near(expected),
        Placed::Paper(elem) => Any::Paper(elem).near(expected),
    }
}

pub(super) trait OnSheet {
    fn shape_sheet(&self) -> zumen_drawing::SheetId;
}

impl OnSheet for Element<PaperPoint> {
    /// 形の点が置かれた用紙。点がなければ、図面の用紙
    fn shape_sheet(&self) -> zumen_drawing::SheetId {
        let point = match &self.shape {
            Shape::Line { start, .. } => Some(start),
            Shape::Circle { center, .. } | Shape::Curve(Edge::Arc { center, .. }) => Some(center),
            Shape::Polyline(line) => line.vertices.first().map(|vertex| &vertex.point),
            Shape::Point(dot) => Some(&dot.position),
            Shape::Text(text) => match &text.placement {
                zumen_drawing::Placement::Anchor { point, .. } => Some(point),
                zumen_drawing::Placement::Between { first, .. } => Some(first),
            },
            Shape::Formatted(text) => Some(&text.point),
            _ => None,
        };
        point.map_or(super::SHEET, |point| point.sheet)
    }
}

/// 表のコード。レイヤ・利用者定義線種は変換した図面モデルから、利用者定義色・線幅はフィーチャから数える
#[derive(Clone)]
pub(super) struct Tables {
    layers: BTreeSet<u32>,
    line_types: BTreeSet<u32>,
    /// 利用者定義色の RGB。コードは 17 から
    colors: Vec<Option<[u8; 3]>>,
    /// 利用者定義線幅（mm）。コードは 11 から
    pub(super) widths: Vec<Option<f64>>,
}

impl Tables {
    pub(super) fn from_live(
        layers: BTreeSet<u32>,
        line_types: BTreeSet<u32>,
        colors: Vec<Option<[u8; 3]>>,
        widths: Vec<Option<f64>>,
    ) -> Self {
        Self {
            layers,
            line_types,
            colors,
            widths,
        }
    }

    /// 利用者定義色と線幅を順に探す作業。変わらないフィールドは探さない。
    pub(super) fn style_work(&self, style: &Style, old: Option<&Style>) -> u64 {
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

    pub(super) fn new(
        records: &[FeatureRecord],
        drawing: &Drawing2D,
        cx: &mut Cx,
    ) -> Result<Self, Error> {
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

    pub(super) fn layer(&self, layer: LayerId) -> Option<i64> {
        self.layers.contains(&layer.0).then_some(i64::from(layer.0))
    }

    /// 色のコードと、そのコードを読み直したときの色。利用者定義色は、RGB が合うときだけ
    pub(super) fn color(&self, color: Inherit<Color>) -> Option<(i64, Color)> {
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

    pub(super) fn line_type(&self, line_type: Inherit<LineTypeId>) -> Option<i64> {
        let Inherit::Own(LineTypeId(code)) = line_type else {
            return None;
        };
        let predefined =
            usize::try_from(code).is_ok_and(|code| (1..=LINE_TYPES.len()).contains(&code));
        (predefined || self.line_types.contains(&code)).then_some(i64::from(code))
    }

    pub(super) fn width(&self, weight: Inherit<LineWeight>) -> Option<i64> {
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
pub(super) fn grouped(records: &[FeatureRecord], cx: &mut Cx) -> Result<BTreeSet<usize>, Error> {
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
    /// レイヤの追加か変更。書いた定義のインスタンス番号を返す。
    fn layer_change(
        &mut self,
        record: Option<usize>,
        layer: &Layer,
        cx: &mut Cx,
    ) -> Result<Option<u64>, Error>;
    /// レイヤ・色・線種・線幅のうち、変わったもののコード（引数の 0〜3）を書く
    fn codes(
        &mut self,
        record: usize,
        codes: &[(usize, i64)],
        cx: &mut Cx,
    ) -> Result<Option<()>, Error>;
    /// 形を書く。種類は変えない
    fn shape(&mut self, record: usize, args: &Arguments, cx: &mut Cx) -> Result<Option<()>, Error>;
    /// 要素を除く
    fn remove(&mut self, record: usize, cx: &mut Cx) -> Result<Option<()>, Error>;
    /// 用紙に要素を足し、足したインスタンスの番号を返す。引数は、レイヤ・色・線種・線幅のコードも持つ
    fn add(&mut self, args: &Arguments, cx: &mut Cx) -> Result<Option<u64>, Error>;
    /// 書き直したバイト列
    fn bytes(self: Box<Self>, src: &[u8], cx: &mut Cx) -> Result<Vec<u8>, Error>;
    /// 点マーカーを書き直し、足せるか
    fn markers(&self) -> bool {
        false
    }
}

/// 元のバイト列への書き直し
struct Plan<'f> {
    features: &'f [FeatureRecord],
    /// レコードの番号から、そのフィーチャ
    records: Vec<Option<&'f FeatureRecord>>,
    sparse_records: BTreeMap<usize, &'f FeatureRecord>,
    writer: Box<dyn Writer + 'f>,
    fonts: Option<Vec<String>>,
    estimated_texts: Vec<u64>,
}

impl<'f> Plan<'f> {
    fn new(
        records: &'f [FeatureRecord],
        record_count: usize,
        writer: Box<dyn Writer + 'f>,
    ) -> Self {
        let mut dense = vec![None; record_count];
        let mut sparse = BTreeMap::new();
        for record in records {
            if let Some(slot) = dense.get_mut(record.record) {
                *slot = Some(record);
            } else {
                sparse.insert(record.record, record);
            }
        }
        Plan {
            features: records,
            records: dense,
            sparse_records: sparse,
            writer,
            fonts: None,
            estimated_texts: Vec::new(),
        }
    }

    fn record(&self, record: usize) -> Option<&'f FeatureRecord> {
        self.records
            .get(record)
            .copied()
            .flatten()
            .or_else(|| self.sparse_records.get(&record).copied())
    }

    fn remove(&mut self, record: usize, cx: &mut Cx) -> Result<Option<()>, Error> {
        self.writer.remove(record, cx)
    }

    /// レイヤとスタイルのうち、変わったもののコードを書く
    fn styled(
        &mut self,
        found: &Found,
        element: &mut Placed,
        tables: &Tables,
        grouped: &BTreeSet<usize>,
        rewrite: bool,
        cx: &mut Cx,
    ) -> Result<Option<()>, Error> {
        let mut result = None;
        let computed = (|| -> Option<()> {
            let name = self.record(found.record)?.name.to_ascii_lowercase();
            let marker = name == MARKER && self.writer.markers();
            let annotation = name == text::NAME;
            if !(marker || annotation || STYLED.contains(&name.as_str()))
                || grouped.contains(&found.record)
            {
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
            if style.line_type_scale != 1.0 || style.transparency != Inherit::Layer {
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
            // 点マーカーは線種と線幅を持たない
            if style.line_type != was.line_type {
                if marker || annotation {
                    return None;
                }
                codes.push((2, tables.line_type(style.line_type)?));
            }
            if style.line_weight != was.line_weight {
                if marker || annotation {
                    return None;
                }
                codes.push((3, tables.width(style.line_weight?)?));
            }
            if codes.is_empty() || (annotation && rewrite) {
                return Some(());
            }
            match self.writer.codes(found.record, &codes, cx) {
                Ok(value) => value,
                Err(error) => {
                    result = Some(error);
                    None
                }
            }
        })();
        match result {
            Some(error) => Err(error),
            None => Ok(computed),
        }
    }

    /// 形を書く。種類は変えない
    fn shaped(
        &mut self,
        found: &Found,
        element: &mut Placed,
        grouped: &BTreeSet<usize>,
        cx: &mut Cx,
    ) -> Result<Option<()>, Error> {
        match &*element {
            Placed::Model(element) => count_shape(&element.shape, cx)?,
            Placed::Paper(element) => count_shape(&element.shape, cx)?,
        }
        let mut error = None;
        let computed = (|| -> Option<()> {
            let name = self.record(found.record)?.name.to_ascii_lowercase();
            let marker = name == MARKER && self.writer.markers();
            if !(marker || name == text::NAME || SHAPED.contains(&name.as_str()))
                || grouped.contains(&found.record)
            {
                return None;
            }
            let args = match self.shape_arguments(element, Some(found.record), cx) {
                Ok(args) => args?,
                Err(stop) => {
                    error = Some(stop);
                    return None;
                }
            };
            if args.name != name {
                return None;
            }
            match self.writer.shape(found.record, &args, cx) {
                Ok(value) => value,
                Err(stop) => {
                    error = Some(stop);
                    None
                }
            }
        })();
        match error {
            Some(error) => Err(error),
            None => Ok(computed),
        }
    }

    /// 用紙に要素を足す。足したインスタンスの番号と、読み直したときの要素を返す。書ける形で、色・線種・線幅を
    /// 書いていない（レイヤかブロックに従う）なら、そのフィールドの名前を `unset` に入れて `None` を返す（決定 0292）
    fn add(
        &mut self,
        element: &Element<PaperPoint>,
        tables: &Tables,
        unset: &mut Option<&'static str>,
        cx: &mut Cx,
    ) -> Result<Option<(u64, Element<PaperPoint>)>, Error> {
        count_element_copy(element, cx, 1)?;
        count_shape(&element.shape, cx)?;
        text::count(&element.shape, cx)?;
        let mut error = None;
        let computed = (|| -> Option<(u64, Element<PaperPoint>)> {
            let mut placed = Placed::Paper(element.clone());
            let mut args = match self.shape_arguments(&mut placed, None, cx) {
                Ok(args) => args?,
                Err(stop) => {
                    error = Some(stop);
                    return None;
                }
            };
            let Placed::Paper(mut element) = placed else {
                return None;
            };
            let style = element.style;
            // 書かない値を、ほかの理由より先に示す（JWW と同じ順、決定 0292）
            if let Some(field) = unset_field(&style, args.name == MARKER || args.name == text::NAME)
            {
                *unset = Some(field);
                return None;
            }
            if style.line_type_scale != 1.0 || style.transparency != Inherit::Layer {
                return None;
            }
            let (color, read) = tables.color(style.color)?;
            element.style.color = Inherit::Own(read);
            let codes = if args.name == MARKER || args.name == text::NAME {
                // 点マーカーは線種と線幅を持たず、読み直すとレイヤに従う線種と線幅なしになる
                if !self.writer.markers()
                    || style.line_type != Inherit::Layer
                    || !matches!(style.line_weight, None | Some(Inherit::Layer))
                {
                    return None;
                }
                element.style.line_weight = None;
                vec![tables.layer(element.layer)?, color]
            } else {
                vec![
                    tables.layer(element.layer)?,
                    color,
                    tables.line_type(style.line_type)?,
                    tables.width(style.line_weight?)?,
                ]
            };
            for (at, code) in codes.iter().enumerate() {
                *args.values.get_mut(at)? = code.to_string();
            }
            let instance = match self.writer.add(&args, cx) {
                Ok(value) => value?,
                Err(stop) => {
                    error = Some(stop);
                    return None;
                }
            };
            if args.name == text::NAME {
                self.estimated_texts.push(instance);
            }
            Some((instance, element))
        })();
        match error {
            Some(error) => Err(error),
            None => Ok(computed),
        }
    }

    fn shape_arguments(
        &mut self,
        element: &mut Placed,
        original: Option<usize>,
        cx: &mut Cx,
    ) -> Result<Option<Arguments>, Error> {
        let original = original.and_then(|record| self.record(record));
        macro_rules! prepare {
            ($element:expr) => {{
                let element = $element;
                if matches!(element.shape, Shape::Text(_) | Shape::Formatted(_)) {
                    text::count(&element.shape, cx)?;
                    let Some(mut prepared) = text::prepare(&element.shape, original) else {
                        return Ok(None);
                    };
                    let Some(code) = self.font(prepared.font.as_deref(), original, cx)? else {
                        return Ok(None);
                    };
                    let Some(value) = prepared.args.values.get_mut(2) else {
                        return Ok(None);
                    };
                    *value = code.to_string();
                    if prepared.font.is_none()
                        && let Some(name) = usize::try_from(code)
                            .ok()
                            .and_then(|n| n.checked_sub(1))
                            .and_then(|n| self.fonts.as_ref()?.get(n))
                        && let Shape::Formatted(text) = &mut prepared.shape
                        && let Some(span) = text.spans.first_mut()
                    {
                        span.format.font = Some(text::font(name));
                    }
                    if original.is_some() {
                        let Some(layer) = prepared.args.values.get_mut(0) else {
                            return Ok(None);
                        };
                        *layer = element.layer.0.to_string();
                        let Inherit::Own(Color {
                            code:
                                Some(ColorCode {
                                    system: ColorSystem::Sxf,
                                    value,
                                }),
                            ..
                        }) = element.style.color
                        else {
                            return Ok(None);
                        };
                        let Some(color) = prepared.args.values.get_mut(1) else {
                            return Ok(None);
                        };
                        *color = value.to_string();
                    }
                    element.shape = prepared.shape;
                    Some(prepared.args)
                } else {
                    arguments(&element.shape)
                }
            }};
        }
        Ok(match element {
            Placed::Model(element) => prepare!(element),
            Placed::Paper(element) => prepare!(element),
        })
    }

    fn font(
        &mut self,
        name: Option<&str>,
        original: Option<&FeatureRecord>,
        cx: &mut Cx,
    ) -> Result<Option<i64>, Error> {
        if name.is_some_and(|name| name.is_empty() || !text::valid_string(name)) {
            return Ok(None);
        }
        if self.fonts.is_none() {
            let mut fonts = Vec::new();
            for record in self.features {
                work(cx, 1, &Ref::default())?;
                if record.name.eq_ignore_ascii_case("text_font") {
                    let Some(crate::drawing::Arg::Text(name)) = record.args.first() else {
                        return Ok(None);
                    };
                    work(cx, (name.len() as u64).div_ceil(1024), &Ref::default())?;
                    fonts.push(name.clone());
                }
            }
            self.fonts = Some(fonts);
        }
        let Some(fonts) = &mut self.fonts else {
            return Ok(None);
        };
        let Some(name) = name else {
            return Ok(original.and_then(|r| Args { values: &r.args }.int(2)));
        };
        for (at, known) in fonts.iter().enumerate() {
            work(
                cx,
                1 + (known.len().min(name.len()) as u64) / 1024,
                &Ref::default(),
            )?;
            if known == name {
                return Ok(i64::try_from(at + 1).ok());
            }
        }
        let code = i64::try_from(fonts.len())
            .ok()
            .and_then(|n| n.checked_add(1));
        let args = Arguments {
            name: "text_font",
            values: vec![name.into()],
        };
        if self.writer.add(&args, cx)?.is_none() {
            return Ok(None);
        }
        fonts.push(name.into());
        Ok(code)
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
    /// 足す所と、そのマーカー。用紙のフィーチャの前か、最後のフィーチャの後
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

    fn string_argument(&mut self, record: usize, at: usize, value: &str) -> Option<()> {
        let part = self.file.records().get(record)?.parts().first()?;
        let range = match part.args().get(at)? {
            Value::Text { raw, quote } => {
                let width = if *quote == Quote::Backslash { 2 } else { 1 };
                raw.start.checked_sub(width)?..raw.end.checked_add(width)?
            }
            Value::Token(raw) => raw.clone(),
            _ => return None,
        };
        self.patches.push((range, text::sfc_string(value)?));
        Some(())
    }
}

impl Writer for Sfc<'_> {
    fn layer_change(
        &mut self,
        record: Option<usize>,
        layer: &Layer,
        cx: &mut Cx,
    ) -> Result<Option<u64>, Error> {
        let Some(state) = layer.state else {
            return Ok(None);
        };
        let visible = if state.visible { "1" } else { "0" };
        let Some(record) = record else {
            return self.add(
                &Arguments {
                    name: "layer",
                    values: vec![layer.name.clone(), visible.into()],
                },
                cx,
            );
        };
        Ok((|| {
            let entry = self.file.records().get(record)?;
            let part = entry.parts().first()?;
            let id = entry.id()?;
            self.string_argument(record, 0, &layer.name)?;
            if part.args().len() > 1 {
                self.argument(record, 1, visible)?;
            } else if !state.visible {
                let at = match part.args().first()? {
                    Value::Text { raw, quote } => raw
                        .end
                        .checked_add(if *quote == Quote::Backslash { 2 } else { 1 })?,
                    Value::Token(raw) => raw.end,
                    _ => return None,
                };
                self.patches.push((at..at, b",'0'".to_vec()));
            }
            Some(id)
        })())
    }
    fn codes(
        &mut self,
        record: usize,
        codes: &[(usize, i64)],
        _cx: &mut Cx,
    ) -> Result<Option<()>, Error> {
        Ok((|| {
            for (at, code) in codes {
                self.argument(record, *at, &code.to_string())?;
            }
            Some(())
        })())
    }

    fn shape(
        &mut self,
        record: usize,
        args: &Arguments,
        _cx: &mut Cx,
    ) -> Result<Option<()>, Error> {
        Ok((|| {
            let first = if args.name == text::NAME {
                0
            } else {
                args.first()
            };
            for (at, text) in args.values.iter().enumerate().skip(first) {
                if text::is_string(args, at) {
                    self.string_argument(record, at, text)?;
                } else if args.name == text::NAME
                    && at == 12
                    && text == "1"
                    && self
                        .file
                        .records()
                        .get(record)?
                        .parts()
                        .first()?
                        .args()
                        .len()
                        == 12
                {
                    // 旧版の横書きは省略できる。
                } else {
                    self.argument(record, at, text)?;
                }
            }
            Some(())
        })())
    }

    /// フィーチャのコメントを、後の改行とともに除く
    fn remove(&mut self, record: usize, _cx: &mut Cx) -> Result<Option<()>, Error> {
        let Some(span) = self.file.records().get(record).map(crate::Record::span) else {
            return Ok(None);
        };
        self.patches
            .push((span.start..line_end(&self.file.src, span.end), Vec::new()));
        Ok(Some(()))
    }

    fn add(&mut self, args: &Arguments, _cx: &mut Cx) -> Result<Option<u64>, Error> {
        Ok((|| {
            let (at, marker, before) = self.insert?;
            let (_, open, close) = Marker::ALL
                .iter()
                .find(|(found, _, _)| *found == marker)
                .copied()?;
            let instance = self.next;
            self.next = self.next.checked_add(1)?;
            let nl = self.newline;
            let mut out = Vec::new();
            if !before {
                out.extend_from_slice(nl.as_bytes());
            }
            out.extend_from_slice(open);
            out.extend_from_slice(format!("{nl}#{instance} = {}_feature(", args.name).as_bytes());
            for (at, value) in args.values.iter().enumerate() {
                if at > 0 {
                    out.push(b',');
                }
                if text::is_string(args, at) {
                    out.extend(text::sfc_string(value)?);
                } else {
                    out.extend_from_slice(format!("'{value}'").as_bytes());
                }
            }
            out.push(b')');
            out.extend_from_slice(nl.as_bytes());
            out.extend_from_slice(close);
            if before {
                out.extend_from_slice(nl.as_bytes());
            }
            self.patches.push((at..at, out));
            Some(instance)
        })())
    }

    fn bytes(self: Box<Self>, src: &[u8], cx: &mut Cx) -> Result<Vec<u8>, Error> {
        patched(self.patches, src, cx)
    }

    fn markers(&self) -> bool {
        true
    }
}

/// ファイルの改行
pub(super) fn newline(src: &[u8]) -> &'static str {
    if src.windows(2).any(|pair| pair == b"\r\n") {
        "\r\n"
    } else {
        "\n"
    }
}

/// 範囲の終わりの後に改行があれば、その後の位置
pub(super) fn line_end(src: &[u8], end: usize) -> usize {
    let rest = src.get(end..).unwrap_or_default();
    end + if rest.starts_with(b"\r\n") {
        2
    } else {
        usize::from(rest.starts_with(b"\n"))
    }
}

/// 圧縮ページ保持された [`zumen_live::SourceBytes`] 上で `end` 直後の改行位置を返す
pub(super) fn line_end_in_source(src: &zumen_live::SourceBytes, end: usize) -> usize {
    let check_end = end.saturating_add(2).min(src.len());
    src.with_range(end..check_end, |rs| {
        end + if rs.bytes.starts_with(b"\r\n") {
            2
        } else {
            usize::from(rs.bytes.starts_with(b"\n"))
        }
    })
}

/// 範囲を書き直したバイト列。範囲は重ならない
pub(super) fn patched(
    mut patches: Vec<(Range<usize>, Vec<u8>)>,
    src: &[u8],
    cx: &mut Cx,
) -> Result<Vec<u8>, Error> {
    work(cx, 1 + src.len() as u64 / 1024, &Ref::default())?;
    for (_, bytes) in &patches {
        work(cx, 1 + bytes.len() as u64 / 1024, &Ref::default())?;
    }
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

/// [`zumen_live::SourceBytes`] から未変更ページを直接展開しながら範囲書き換えを行う（[`patched`] とバイト列・作業量計上が完全一致）
pub(super) fn patched_source(
    mut patches: Vec<(Range<usize>, Vec<u8>)>,
    src: &zumen_live::SourceBytes,
    cx: &mut Cx,
) -> Result<Vec<u8>, Error> {
    work(cx, 1 + src.len() as u64 / 1024, &Ref::default())?;
    for (_, bytes) in &patches {
        work(cx, 1 + bytes.len() as u64 / 1024, &Ref::default())?;
    }
    patches.sort_by_key(|(range, _)| range.start);
    let mut out = Vec::with_capacity(src.len());
    let mut end = 0;
    for (range, bytes) in &patches {
        work(cx, 1, &Ref::default())?;
        if end < range.start {
            src.append_range(&mut out, end..range.start);
        }
        out.extend_from_slice(bytes);
        end = range.end;
    }
    if end < src.len() {
        src.append_range(&mut out, end..src.len());
    }
    Ok(out)
}

/// フィーチャの名前と引数。レイヤ・色・線種・線幅（0〜3。点マーカーはレイヤと色の 0〜1）は空にしておく
pub(super) struct Arguments {
    pub(super) name: &'static str,
    pub(super) values: Vec<String>,
}

impl Arguments {
    /// 形の初めの引数
    pub(super) fn first(&self) -> usize {
        if self.name == MARKER || self.name == text::NAME {
            2
        } else {
            4
        }
    }
}

/// 全種類の形を予算付きでたどり、その量に応じて複製・変換の作業も先に数える。
pub(super) fn count_element_copy<P: Encode>(
    element: &Element<P>,
    cx: &mut Cx,
    copies: u64,
) -> Result<(), Error> {
    let before = cx.budget.used(Kind::Work);
    Hash::element_checked(element, cx).map_err(|stop| {
        cx.report(stop.to_diagnostic().at(element.source.clone()));
        Error::Stopped(stop)
    })?;
    let traversed = cx.budget.used(Kind::Work).saturating_sub(before);
    let source = element.source.id().map_or(0, str::len) as u64;
    work(
        cx,
        traversed
            .saturating_mul(copies)
            .saturating_add(source / 1024),
        &element.source,
    )
}

/// 複製を作らずに要素を比べるとき（`same_any`）の作業を数える。入れ子のない形は頂点や文字列の長さから直接数え、
/// 入れ子を持つ形は `count_element_copy` で全体をたどる。
pub(super) fn count_element_compare<P: Encode>(
    element: &Element<P>,
    cx: &mut Cx,
) -> Result<(), Error> {
    let simple = match &element.shape {
        Shape::Line { .. }
        | Shape::Circle { .. }
        | Shape::Curve(
            Edge::Line { .. } | Edge::Arc { .. } | Edge::Elliptic { .. } | Edge::Clothoid { .. },
        ) => Some(1_u64),
        Shape::Polyline(line) => Some(
            1_u64
                .saturating_add((line.vertices.len() as u64).saturating_mul(4))
                .saturating_add((line.vertex_widths.len() as u64).saturating_mul(2)),
        ),
        Shape::Point(point) => {
            let extra = match &point.look {
                PointLook::Marker { key, .. } => {
                    (key.namespace.len().saturating_add(key.code.len()) as u64) / 1024
                }
                _ => 0,
            };
            Some(1_u64.saturating_add(extra))
        }
        _ => None,
    };
    match simple {
        Some(amount) => work(cx, amount, &element.source),
        None => count_element_copy(element, cx, 1),
    }
}

/// 折線を複製し、座標を文字列にする前に点の走査を数える。
pub(super) fn count_shape<P>(shape: &Shape<P>, cx: &mut Cx) -> Result<(), Error> {
    if let Shape::Polyline(line) = shape {
        work(
            cx,
            (line.vertices.len() as u64).saturating_mul(3),
            &Ref::default(),
        )?;
    }
    Ok(())
}

/// 形を書く引数。数は、読み直すと同じ値になる最短の 10 進で書く
pub(super) fn arguments<P: transform::Point>(shape: &Shape<P>) -> Option<Arguments> {
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
            // SXF の折れ線は、線種の生成を持たない
            line_type_generation: false,
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
        // 点マーカーは、点・マーカーのコード・回転（度）・尺度。名前空間 `sxf.marker` の既定義のマーカーで、読み直すと同じ
        // コードになるものだけを書く。作図の補助の点はない（決定 0183）
        Shape::Point(Point {
            position,
            rotation,
            look: PointLook::Marker { key, scale },
            construction: false,
        }) if key.namespace == "sxf.marker" => {
            let code = key.code.parse::<i64>().ok()?;
            if code.to_string() != key.code {
                return None;
            }
            values.truncate(2);
            values.extend(point(position)?);
            values.push(key.code.clone());
            values.push(number(*rotation)?);
            values.push(number(*scale)?);
            MARKER
        }
        _ => return None,
    };
    Some(Arguments { name, values })
}

#[cfg(test)]
mod tests {
    use super::*;
    use zumen_base::{Budget, CancelToken, Limit, Limits};
    use zumen_drawing::{SheetId, Vertex};

    #[test]
    fn copying_large_shapes_is_checked_before_copying() {
        let point = PaperPoint {
            sheet: SheetId(0),
            x: 1.0,
            y: 2.0,
        };
        let element = Element {
            layer: LayerId(0),
            style: Style {
                color: Inherit::Layer,
                line_type: Inherit::Layer,
                line_type_scale: 1.0,
                line_weight: None,
                transparency: Inherit::Layer,
            },
            shape: Shape::Polyline(Polyline {
                vertices: vec![Vertex { point, bulge: 0.0 }; 100],
                closed: false,
                width: None,
                vertex_widths: Vec::new(),
                line_type_generation: false,
            }),
            source: Ref::default(),
        };
        let cancel = CancelToken::new();
        let mut budget = Budget::default();
        let mut diagnostics = Diagnostics::new();
        count_element_copy(
            &element,
            &mut Cx::new(&mut budget, &mut diagnostics, &cancel),
            2,
        )
        .unwrap();
        let amount = budget.used(Kind::Work);
        let mut budget = Budget::new(Limits::default().with(Kind::Work, Limit::Finite(amount - 1)));
        let mut diagnostics = Diagnostics::new();
        assert!(matches!(
            count_element_copy(
                &element,
                &mut Cx::new(&mut budget, &mut diagnostics, &cancel),
                2
            ),
            Err(Error::Stopped(_))
        ));
    }

    #[test]
    fn copying_a_large_unmodified_range_is_checked() {
        let src = vec![b'x'; 8192];
        let mut budget = Budget::new(Limits::default().with(Kind::Work, Limit::Finite(1)));
        let mut diagnostics = Diagnostics::new();
        let cancel = CancelToken::new();
        assert!(matches!(
            patched(
                Vec::new(),
                &src,
                &mut Cx::new(&mut budget, &mut diagnostics, &cancel)
            ),
            Err(Error::Stopped(_))
        ));
    }
}
