//! apply の op の変更: 要求の変更を、図面モデルの変更の一覧にする（決定 0162・0163・0164・0165）。
//!
//! 書けるかどうか（フォントの種類・ブロックがあるかなど）は、apply が形式ごとに確かめる。

use std::borrow::Cow;
use std::collections::BTreeMap;

use zumen_base::{Cx, Diagnostic, Kind};
use zumen_base::{Ref, codes};
use zumen_drawing::{
    Affine, Arrowhead, Changes, Color, ColorCode, ColorSystem, DimensionStyle, Drawing2D, Edit,
    Element, Hash, Inherit, Layer, LayerId, LayerState, LayerStyle, LineType, LineTypeDefinition,
    LineTypeId, LineWeight, ModelPoint, PaperLength, PaperPoint, PatternBasis, Placed, Restyle,
    Seen, Shape, Style, SymbolKey, TextStyle, TextStyleDefinition, id,
};

use crate::geometry;
use crate::messages::{self, ApplyRequest};
use crate::session::{Failed, fail, invalid_value, stopped};
use crate::tables::table_id;

/// 要求の変更を、図面モデルの変更の一覧にする。レイヤと線種は、ID か、名前が 1 つに決まるものを図面から引く
pub(crate) fn changes(
    request: &ApplyRequest,
    drawing: &Drawing2D,
    cx: &mut Cx<'_>,
) -> Result<Changes, Failed> {
    let hash = |text: &str, pointer: String, cx: &mut Cx<'_>| {
        text.parse::<Hash>()
            .map_err(|()| fail(cx, invalid_value(pointer)))
    };
    let document = match &request.hash {
        Some(text) => Some(hash(text, "/hash".to_string(), cx)?),
        None => None,
    };
    let mut edits = Vec::with_capacity(request.edits.len());
    let mut tables = None;
    let mut elements = None;
    let mut places = None;
    let mut definitions: Option<BTreeMap<String, &zumen_drawing::SymbolDefinition>> = None;
    for (index, edit) in request.edits.iter().enumerate() {
        cx.check(Kind::Work, 1).map_err(|stop| stopped(cx, stop))?;
        let at = |field: &str| format!("/edits/{index}/{field}");
        let seen = |id: &str, text: &str, cx: &mut Cx<'_>| {
            cx.check(Kind::Work, u64::try_from(id.len()).unwrap_or(u64::MAX))
                .map_err(|stop| stopped(cx, stop))?;
            Ok(Seen {
                id: id.to_string(),
                hash: hash(text, at("hash"), cx)?,
            })
        };
        let value = match edit {
            messages::Edit::Replace(edit) => Some(&edit.geometry),
            messages::Edit::Add(edit) => Some(&edit.geometry),
            _ => None,
        };
        let size = match value {
            Some(messages::Geometry::Polyline(line)) => line.points.len(),
            Some(messages::Geometry::Spline(spline)) => spline
                .control
                .len()
                .saturating_add(spline.knots.len())
                .saturating_add(spline.weights.as_ref().map_or(0, Vec::len))
                .saturating_add(spline.fit.as_ref().map_or(0, Vec::len)),
            Some(messages::Geometry::Text(text)) => text
                .content
                .len()
                .saturating_add(text.style.as_ref().map_or(0, String::len))
                .saturating_add(text.style_id.as_ref().map_or(0, String::len)),
            Some(messages::Geometry::FormattedText(text)) => text
                .style
                .as_ref()
                .map_or(0, String::len)
                .saturating_add(text.style_id.as_ref().map_or(0, String::len)),
            Some(messages::Geometry::Symbol(symbol)) => symbol.name.len(),
            Some(messages::Geometry::Leader(leader)) => leader
                .path
                .len()
                .saturating_add(leader.style.len())
                .saturating_add(leader.style_id.as_ref().map_or(0, String::len))
                .saturating_add(leader.annotation.as_ref().map_or(0, String::len)),
            Some(messages::Geometry::Callout(callout)) => callout
                .branches
                .iter()
                .map(|branch| {
                    branch
                        .lines
                        .iter()
                        .map(Vec::len)
                        .fold(branch.lines.len(), usize::saturating_add)
                })
                .fold(callout.branches.len(), usize::saturating_add)
                .saturating_add(callout.text.as_ref().map_or(0, String::len))
                .saturating_add(callout.symbol.as_ref().map_or(0, String::len)),
            _ => 0,
        };
        cx.check(Kind::Work, u64::try_from(size).unwrap_or(u64::MAX))
            .map_err(|stop| stopped(cx, stop))?;
        if let Some(messages::Geometry::Spline(spline)) = value {
            for point in spline.control.iter().chain(spline.fit.iter().flatten()) {
                cx.check(Kind::Work, u64::try_from(point.len()).unwrap_or(u64::MAX))
                    .map_err(|stop| stopped(cx, stop))?;
            }
        }

        if let Some(messages::Geometry::Symbol(symbol)) = value {
            for item in symbol.attributes.iter().flatten() {
                let size = 1_usize
                    .saturating_add(item.tag.len())
                    .saturating_add(item.value.len());
                cx.check(Kind::Work, u64::try_from(size).unwrap_or(u64::MAX))
                    .map_err(|stop| stopped(cx, stop))?;
            }
        }
        edits.push(match edit {
            messages::Edit::Restyle(edit) => {
                let target = seen(&edit.id, &edit.hash, cx)?;
                if tables.is_none() {
                    tables = Some(TableLookup::new(drawing, cx)?);
                }
                let Some(tables) = &tables else {
                    return Err(Failed);
                };
                Edit::Restyle {
                    target,
                    style: restyle(edit, tables)
                        .map_err(|field| fail(cx, invalid_value(at(field))))?,
                }
            }
            messages::Edit::Transform(edit) => {
                let target = seen(&edit.id, &edit.hash, cx)?;
                let m = match edit.matrix.as_slice() {
                    &[a, b, c, d, e, f] if edit.matrix.iter().all(|v| v.is_finite()) => {
                        [a, b, c, d, e, f]
                    }
                    _ => return Err(fail(cx, invalid_value(at("matrix")))),
                };
                Edit::Transform {
                    target,
                    transform: Affine { m },
                }
            }
            messages::Edit::Remove(edit) => Edit::Remove {
                target: seen(&edit.id, &edit.hash, cx)?,
            },
            messages::Edit::RemoveLayer(edit) => Edit::RemoveLayer {
                target: seen(&edit.id, &edit.hash, cx)?,
            },
            messages::Edit::RemoveTextStyle(edit) => Edit::RemoveTextStyle {
                target: seen(&edit.id, &edit.hash, cx)?,
            },
            messages::Edit::TextStyle(edit) => {
                let target = seen(&edit.id, &edit.hash, cx)?;
                if tables.is_none() {
                    tables = Some(TableLookup::new(drawing, cx)?);
                }
                let Some(tables) = &tables else {
                    return Err(Failed);
                };
                cx.check(
                    Kind::Work,
                    text_style_work(edit.name.as_deref(), &edit.font, &edit.big_font),
                )
                .map_err(|stop| stopped(cx, stop))?;
                // 指した文字スタイルがなければ、要素と同じく `change.element.missing` にする
                let Some(base) = tables.text_styles.get(&edit.id).copied() else {
                    let missing =
                        Diagnostic::new(codes::CHANGE_ELEMENT_MISSING).with("id", edit.id.as_str());
                    return Err(fail(cx, missing));
                };
                let fields = TextStyleFields {
                    name: edit.name.as_deref(),
                    font: edit.font.as_ref(),
                    big_font: edit.big_font.as_ref(),
                    height: edit.height,
                    width: edit.width,
                    oblique: edit.oblique,
                    backward: edit.backward,
                    upside_down: edit.upside_down,
                    vertical: edit.vertical,
                };
                let style = text_style_change(base, &fields)
                    .map_err(|field| fail(cx, invalid_value(at(field))))?;
                Edit::ReplaceTextStyle {
                    target,
                    style: Box::new(style),
                }
            }
            messages::Edit::AddTextStyle(edit) => {
                cx.check(
                    Kind::Work,
                    text_style_work(Some(&edit.name), &edit.font, &edit.big_font),
                )
                .map_err(|stop| stopped(cx, stop))?;
                // 書かない欄は、Standard と同じにする
                let standard = drawing.text_styles.iter().find(|style| {
                    style.name.eq_ignore_ascii_case("Standard") && style.definition.is_some()
                });
                let base = TextStyle {
                    name: String::new(),
                    definition: standard.and_then(|style| style.definition.clone()),
                    source: Ref::default(),
                };
                let fields = TextStyleFields {
                    name: Some(&edit.name),
                    font: edit.font.as_ref(),
                    big_font: edit.big_font.as_ref(),
                    height: edit.height,
                    width: edit.width,
                    oblique: edit.oblique,
                    backward: edit.backward,
                    upside_down: edit.upside_down,
                    vertical: edit.vertical,
                };
                let style = text_style_change(&base, &fields)
                    .map_err(|field| fail(cx, invalid_value(at(field))))?;
                Edit::AddTextStyle {
                    style: Box::new(style),
                }
            }
            messages::Edit::RemoveDimensionStyle(edit) => Edit::RemoveDimensionStyle {
                target: seen(&edit.id, &edit.hash, cx)?,
            },
            messages::Edit::DimensionStyle(edit) => {
                let target = seen(&edit.id, &edit.hash, cx)?;
                if tables.is_none() {
                    tables = Some(TableLookup::new(drawing, cx)?);
                }
                let Some(tables) = &tables else {
                    return Err(Failed);
                };
                let fields = DimensionStyleFields {
                    name: edit.name.as_deref(),
                    ends: edit.ends.as_deref(),
                    leader: edit.leader.as_deref(),
                    size: edit.size,
                    tick: edit.tick,
                    scale: edit.scale,
                };
                cx.check(Kind::Work, fields.work())
                    .map_err(|stop| stopped(cx, stop))?;
                // 指した寸法スタイルがなければ、要素と同じく `change.element.missing` にする
                let Some(base) = tables.dimension_styles.get(&edit.id).copied() else {
                    let missing =
                        Diagnostic::new(codes::CHANGE_ELEMENT_MISSING).with("id", edit.id.as_str());
                    return Err(fail(cx, missing));
                };
                let style = dimension_style_edit(base, &fields)
                    .map_err(|field| fail(cx, invalid_value(at(field))))?;
                Edit::ReplaceDimensionStyle {
                    target,
                    style: Box::new(style),
                }
            }
            messages::Edit::AddDimensionStyle(edit) => {
                let fields = DimensionStyleFields {
                    name: Some(&edit.name),
                    ends: edit.ends.as_deref(),
                    leader: edit.leader.as_deref(),
                    size: edit.size,
                    tick: edit.tick,
                    scale: edit.scale,
                };
                cx.check(Kind::Work, fields.work())
                    .map_err(|stop| stopped(cx, stop))?;
                // 書かない欄は、Standard と同じにする。なければ初めの子でない寸法スタイル、それもなければ DXF の
                // 既定の値。apply が写すレコードと同じ
                let styles = &drawing.dimension_styles;
                let standard = styles
                    .iter()
                    .find(|style| style.name.eq_ignore_ascii_case("Standard"))
                    .or_else(|| {
                        styles
                            .iter()
                            .find(|style| !style.name.is_empty() && !style.name.contains('$'))
                    });
                let base = standard.cloned().unwrap_or(DimensionStyle {
                    name: String::new(),
                    ends: [Arrowhead::Default, Arrowhead::Default],
                    leader: Arrowhead::Default,
                    size: 0.18,
                    tick: None,
                    scale: 1.0,
                    source: Ref::default(),
                });
                let style = dimension_style_edit(&base, &fields)
                    .map_err(|field| fail(cx, invalid_value(at(field))))?;
                Edit::AddDimensionStyle {
                    style: Box::new(style),
                }
            }
            messages::Edit::RemoveLineType(edit) => Edit::RemoveLineType {
                target: seen(&edit.id, &edit.hash, cx)?,
            },
            messages::Edit::LineType(edit) => {
                let target = seen(&edit.id, &edit.hash, cx)?;
                if tables.is_none() {
                    tables = Some(TableLookup::new(drawing, cx)?);
                }
                let Some(tables) = &tables else {
                    return Err(Failed);
                };
                cx.check(
                    Kind::Work,
                    line_type_work(
                        edit.name.as_deref(),
                        edit.description.as_deref(),
                        edit.pattern.as_deref(),
                    ),
                )
                .map_err(|stop| stopped(cx, stop))?;
                // 指した線種がなければ、要素と同じく `change.element.missing` にする
                let Some(base) = tables.line_type_items.get(&edit.id).copied() else {
                    let missing =
                        Diagnostic::new(codes::CHANGE_ELEMENT_MISSING).with("id", edit.id.as_str());
                    return Err(fail(cx, missing));
                };
                let line_type = line_type_edit(
                    base,
                    edit.name.as_deref(),
                    edit.description.as_deref(),
                    edit.pattern.as_deref(),
                )
                .map_err(|field| fail(cx, invalid_value(at(field))))?;
                Edit::ReplaceLineType {
                    target,
                    line_type: Box::new(line_type),
                }
            }
            messages::Edit::AddLineType(edit) => {
                cx.check(
                    Kind::Work,
                    line_type_work(
                        Some(&edit.name),
                        edit.description.as_deref(),
                        edit.pattern.as_deref(),
                    ),
                )
                .map_err(|stop| stopped(cx, stop))?;
                // 書かない欄は、Continuous と同じにする
                let continuous = drawing.line_types.iter().find(|line_type| {
                    line_type.name.eq_ignore_ascii_case("Continuous")
                        && line_type.definition.is_some()
                });
                let base = LineType {
                    id: LineTypeId(0),
                    name: String::new(),
                    definition: continuous.and_then(|line_type| line_type.definition.clone()),
                    source: Ref::default(),
                };
                let line_type = line_type_edit(
                    &base,
                    Some(&edit.name),
                    edit.description.as_deref(),
                    edit.pattern.as_deref(),
                )
                .map_err(|field| fail(cx, invalid_value(at(field))))?;
                Edit::AddLineType {
                    line_type: Box::new(line_type),
                }
            }
            messages::Edit::Replace(edit) => {
                let target = seen(&edit.id, &edit.hash, cx)?;
                count_formatted_geometry(&edit.geometry, cx)?;
                if elements.is_none() {
                    elements = Some(ElementLookup::new(drawing, cx)?);
                }
                if tables.is_none() && has_style_id(&edit.geometry) {
                    tables = Some(TableLookup::new(drawing, cx)?);
                }
                let geometry = named_style(&edit.geometry, tables.as_ref())
                    .map_err(|field| fail(cx, invalid_value(at(field))))?;
                // 指した要素がなければ、除くときと同じく `change.element.missing` にする
                // 引出線の注記は元のままにする。注記の ID は、元の注記の要素を指すものだけを受ける
                if let messages::Geometry::Leader(leader) = &*geometry {
                    let was = match elements.as_ref().and_then(|index| index.get(&edit.id)) {
                        Some(FoundElement::Model(Element {
                            shape: Shape::Leader(was),
                            ..
                        })) => Some(&was.annotation),
                        Some(FoundElement::Paper(Element {
                            shape: Shape::Leader(was),
                            ..
                        })) => Some(&was.annotation),
                        _ => None,
                    };
                    if let (Some(was), Some(index)) = (was, elements.as_ref()) {
                        let kept = match (&leader.annotation, was) {
                            (Some(id), Some(was)) => index.get(id).is_some_and(|found| {
                                let source = match found {
                                    FoundElement::Model(found) => &found.source,
                                    FoundElement::Paper(found) => &found.source,
                                };
                                source == was
                            }),
                            (Some(_), None) => false,
                            (None, None) => true,
                            // entities は、注記の要素がなければ ID を書かない
                            (None, Some(was)) => !index.has_source(was, cx)?,
                        };
                        if !kept {
                            return Err(fail(cx, invalid_value(at("geometry/annotation"))));
                        }
                    }
                }
                let element = match elements.as_ref().and_then(|index| index.get(&edit.id)) {
                    Some(FoundElement::Model(element)) => {
                        count_old_element(element, cx)?;
                        reshaped(element, &geometry).map(Placed::Model)
                    }
                    Some(FoundElement::Paper(element)) => {
                        count_old_element(element, cx)?;
                        reshaped(element, &geometry).map(Placed::Paper)
                    }
                    None => {
                        let missing = Diagnostic::new(codes::CHANGE_ELEMENT_MISSING)
                            .with("id", edit.id.as_str());
                        return Err(fail(cx, missing));
                    }
                }
                .map_err(|field| fail(cx, invalid_value(at(&field))))?;
                Edit::Replace {
                    target,
                    element: Box::new(element),
                }
            }
            messages::Edit::Add(edit) => {
                cx.check(
                    Kind::Work,
                    u64::try_from(edit.place.len()).unwrap_or(u64::MAX),
                )
                .map_err(|stop| stopped(cx, stop))?;
                count_formatted_geometry(&edit.geometry, cx)?;
                if tables.is_none() {
                    tables = Some(TableLookup::new(drawing, cx)?);
                }
                let Some(tables) = &tables else {
                    return Err(Failed);
                };
                if places.is_none() {
                    places = Some(PlaceLookup::new(drawing, cx)?);
                }
                let Some(places) = &places else {
                    return Err(Failed);
                };
                let definition = if let messages::Geometry::Symbol(symbol) = &edit.geometry {
                    if definitions.is_none() {
                        let mut found = BTreeMap::new();
                        for definition in &drawing.symbols {
                            let work = definition.key.name.len().saturating_add(1);
                            cx.check(Kind::Work, u64::try_from(work).unwrap_or(u64::MAX))
                                .map_err(|stop| stopped(cx, stop))?;
                            if definition.key.namespace == "dxf.block" {
                                found
                                    .entry(definition.key.name.to_ascii_lowercase())
                                    .or_insert(definition);
                            }
                        }
                        definitions = Some(found);
                    }
                    let work = symbol.name.len();
                    cx.check(Kind::Work, u64::try_from(work).unwrap_or(u64::MAX))
                        .map_err(|stop| stopped(cx, stop))?;
                    let found = definitions
                        .as_ref()
                        .and_then(|items| items.get(&symbol.name.to_ascii_lowercase()).copied());
                    if let Some(definition) = found {
                        for item in &definition.attributes {
                            let attr = &item.attribute;
                            let size = 1_usize
                                .saturating_add(attr.tag.len())
                                .saturating_add(attr.text.content.len())
                                .saturating_add(attr.text.style.len());
                            cx.check(Kind::Work, u64::try_from(size).unwrap_or(u64::MAX))
                                .map_err(|stop| stopped(cx, stop))?;
                        }
                    }
                    found
                } else {
                    None
                };
                let geometry = named_style(&edit.geometry, Some(tables))
                    .map_err(|field| fail(cx, invalid_value(at(field))))?;
                // 足す引出線の注記は、図面にある要素の ID で指し、その出所を引出線に持たせる（決定 0185）。
                // 注記の種類と置き場所は、書き戻す側が確かめる
                let (geometry, annotation) = match &*geometry {
                    messages::Geometry::Leader(leader) if leader.annotation.is_some() => {
                        if elements.is_none() {
                            elements = Some(ElementLookup::new(drawing, cx)?);
                        }
                        let source = leader
                            .annotation
                            .as_ref()
                            .and_then(|id| elements.as_ref()?.get(id))
                            .map(|found| match found {
                                FoundElement::Model(found) => found.source.clone(),
                                FoundElement::Paper(found) => found.source.clone(),
                            })
                            .ok_or_else(|| fail(cx, invalid_value(at("geometry/annotation"))))?;
                        let mut bare = leader.clone();
                        bare.annotation = None;
                        (Cow::Owned(messages::Geometry::Leader(bare)), Some(source))
                    }
                    _ => (geometry, None),
                };
                let mut element = added(edit, &geometry, places, tables, definition)
                    .map_err(|field| fail(cx, invalid_value(at(&field))))?;
                if let Some(source) = annotation {
                    match &mut element {
                        Placed::Model(Element {
                            shape: Shape::Leader(leader),
                            ..
                        }) => leader.annotation = Some(source),
                        Placed::Paper(Element {
                            shape: Shape::Leader(leader),
                            ..
                        }) => leader.annotation = Some(source),
                        _ => return Err(fail(cx, invalid_value(at("geometry")))),
                    }
                }
                Edit::Add {
                    place: edit.place.clone(),
                    element: Box::new(element),
                }
            }
            messages::Edit::Layer(edit) => {
                let target = seen(&edit.id, &edit.hash, cx)?;
                if tables.is_none() {
                    tables = Some(TableLookup::new(drawing, cx)?);
                }
                let Some(tables) = &tables else {
                    return Err(Failed);
                };
                cx.check(
                    Kind::Work,
                    edit.name.as_ref().map_or(0, |name| name.len() as u64),
                )
                .map_err(|stop| stopped(cx, stop))?;
                // 指したレイヤがなければ、要素と同じく `change.element.missing` にする
                let Some(base) = tables
                    .layer_ids
                    .get(&edit.id)
                    .copied()
                    .flatten()
                    .and_then(|(_, layer)| drawing.layer(layer))
                else {
                    let missing =
                        Diagnostic::new(codes::CHANGE_ELEMENT_MISSING).with("id", edit.id.as_str());
                    return Err(fail(cx, missing));
                };
                let fields = LayerFields {
                    name: edit.name.as_deref(),
                    visible: edit.visible,
                    printable: edit.printable,
                    editable: edit.editable,
                    frozen: edit.frozen,
                    color: edit.color.as_ref(),
                    line_type: edit.line_type.as_ref(),
                    line_weight: edit.line_weight.as_ref(),
                };
                let layer = layer_change(base, &fields, tables)
                    .map_err(|field| fail(cx, invalid_value(at(field))))?;
                Edit::ReplaceLayer {
                    target,
                    layer: Box::new(layer),
                }
            }
            messages::Edit::AddLayer(edit) => {
                if tables.is_none() {
                    tables = Some(TableLookup::new(drawing, cx)?);
                }
                let Some(tables) = &tables else {
                    return Err(Failed);
                };
                cx.check(Kind::Work, edit.name.len() as u64)
                    .map_err(|stop| stopped(cx, stop))?;
                // 書かない色・線種・線幅は、レイヤ 0 と同じにする。透過性は、画層 0 のレコードを写すときに
                // 拡張データを除くので写さない（決定 0157）
                let zero = tables
                    .layers
                    .get("0")
                    .copied()
                    .flatten()
                    .and_then(|layer| drawing.layer(layer));
                let base = Layer {
                    id: LayerId(u32::MAX),
                    name: String::new(),
                    state: None,
                    style: zero.and_then(|layer| layer.style).map(|style| LayerStyle {
                        transparency: None,
                        ..style
                    }),
                    follows_placement: false,
                    source: Ref::default(),
                    description: None,
                };
                let fields = LayerFields {
                    name: Some(&edit.name),
                    visible: edit.visible,
                    printable: edit.printable,
                    editable: edit.editable,
                    frozen: edit.frozen,
                    color: edit.color.as_ref(),
                    line_type: edit.line_type.as_ref(),
                    line_weight: edit.line_weight.as_ref(),
                };
                let layer = layer_change(&base, &fields, tables)
                    .map_err(|field| fail(cx, invalid_value(at(field))))?;
                Edit::AddLayer {
                    layer: Box::new(layer),
                }
            }
        });
    }
    Ok(Changes { edits, document })
}

/// 要素の ID を一度だけ作り、複数の置き換えで共有する。
pub(crate) enum FoundElement<'a> {
    Model(&'a Element<ModelPoint>),
    Paper(&'a Element<PaperPoint>),
}

pub(crate) struct ElementLookup<'a>(BTreeMap<String, FoundElement<'a>>);

impl<'a> ElementLookup<'a> {
    pub(crate) fn new(drawing: &'a Drawing2D, cx: &mut Cx<'_>) -> Result<Self, Failed> {
        let mut found = BTreeMap::new();
        for space in &drawing.spaces {
            cx.check(Kind::Work, 1).map_err(|stop| stopped(cx, stop))?;
            let place = id::space(space.id);
            let mut namer = id::Namer::new(&place);
            for (index, element) in space.elements.iter().enumerate() {
                cx.check(Kind::Work, 1).map_err(|stop| stopped(cx, stop))?;
                found.insert(namer.next(index, element), FoundElement::Model(element));
            }
        }
        let mut definitions = id::DefinitionNamer::default();
        for definition in &drawing.symbols {
            cx.check(Kind::Work, 1).map_err(|stop| stopped(cx, stop))?;
            let place = definitions.next(definition);
            let mut namer = id::Namer::new(&place);
            for (index, element) in definition.space.elements.iter().enumerate() {
                cx.check(Kind::Work, 1).map_err(|stop| stopped(cx, stop))?;
                found.insert(namer.next(index, element), FoundElement::Model(element));
            }
        }
        let mut sheets = id::SheetNamer::default();
        for sheet in &drawing.sheets {
            cx.check(Kind::Work, 1).map_err(|stop| stopped(cx, stop))?;
            let place = sheets.next(sheet);
            let mut namer = id::Namer::new(&place);
            for (index, element) in sheet.elements.iter().enumerate() {
                cx.check(Kind::Work, 1).map_err(|stop| stopped(cx, stop))?;
                found.insert(namer.next(index, element), FoundElement::Paper(element));
            }
        }
        Ok(Self(found))
    }

    pub(crate) fn get(&self, id: &str) -> Option<&FoundElement<'a>> {
        self.0.get(id)
    }

    /// 出所が `source` の要素があるか
    fn has_source(&self, source: &Ref, cx: &mut Cx<'_>) -> Result<bool, Failed> {
        for found in self.0.values() {
            cx.check(Kind::Work, 1).map_err(|stop| stopped(cx, stop))?;
            let at = match found {
                FoundElement::Model(found) => &found.source,
                FoundElement::Paper(found) => &found.source,
            };
            if at == source {
                return Ok(true);
            }
        }
        Ok(false)
    }
}

/// 足す先の検索を変更ごとに繰り返さないための索引。
pub(crate) struct PlaceLookup {
    spaces: BTreeMap<String, ModelPoint>,
    sheets: BTreeMap<String, PaperPoint>,
}

impl PlaceLookup {
    pub(crate) fn new(drawing: &Drawing2D, cx: &mut Cx<'_>) -> Result<Self, Failed> {
        let mut spaces = BTreeMap::new();
        for space in &drawing.spaces {
            cx.check(Kind::Work, 1).map_err(|stop| stopped(cx, stop))?;
            spaces.insert(
                id::space(space.id),
                ModelPoint {
                    space: space.id,
                    x: 0.0,
                    y: 0.0,
                },
            );
        }
        // 記号の定義の空間（決定 0177）
        let mut definitions = id::DefinitionNamer::default();
        for definition in &drawing.symbols {
            cx.check(Kind::Work, 1).map_err(|stop| stopped(cx, stop))?;
            spaces
                .entry(definitions.next(definition))
                .or_insert(ModelPoint {
                    space: definition.space.id,
                    x: 0.0,
                    y: 0.0,
                });
        }
        let mut sheets = BTreeMap::new();
        let mut namer = id::SheetNamer::default();
        for sheet in &drawing.sheets {
            cx.check(Kind::Work, 1).map_err(|stop| stopped(cx, stop))?;
            sheets.insert(
                namer.next(sheet),
                PaperPoint {
                    sheet: sheet.id,
                    x: 0.0,
                    y: 0.0,
                },
            );
        }
        Ok(Self { spaces, sheets })
    }
}

/// MTEXT の解析は、入れ子や区間ごとにフォント名を複製する。内容の長さと、複製を起こし得る
/// 符号の数から上限を求め、解析より先に作業量を数える。
fn count_formatted_geometry(value: &messages::Geometry, cx: &mut Cx<'_>) -> Result<(), Failed> {
    if let messages::Geometry::FormattedText(text) = value {
        let bytes = u64::try_from(text.content.len()).unwrap_or(u64::MAX);
        cx.check(Kind::Work, bytes)
            .map_err(|stop| stopped(cx, stop))?;
        if text.content.contains("\\f") || text.content.contains("\\F") {
            let signs = text
                .content
                .bytes()
                .filter(|b| matches!(b, b'{' | b'}' | b'\\'))
                .count();
            let signs = u64::try_from(signs).unwrap_or(u64::MAX);
            cx.check(Kind::Work, bytes.saturating_mul(signs))
                .map_err(|stop| stopped(cx, stop))?;
        }
    }
    Ok(())
}

/// 表の検索を変更ごとに繰り返さないための索引。同じ名前は 1 つに決まらない
pub(crate) struct TableLookup<'a> {
    pub(crate) layers: BTreeMap<&'a str, Option<LayerId>>,
    layer_ids: BTreeMap<String, Option<(&'a str, LayerId)>>,
    line_types: BTreeMap<&'a str, Option<LineTypeId>>,
    line_type_ids: BTreeMap<String, Option<(&'a str, LineTypeId)>>,
    /// 線種の ID から項目（決定 0164）
    line_type_items: BTreeMap<String, &'a LineType>,
    /// 文字スタイルの ID から名前。同じ名前（大文字と小文字を区別しない）の項目が複数あれば `None`
    text_style_ids: BTreeMap<String, Option<&'a str>>,
    /// 文字スタイルの ID から項目（決定 0163）
    text_styles: BTreeMap<String, &'a TextStyle>,
    /// 寸法スタイルの ID から項目（決定 0165）
    dimension_styles: BTreeMap<String, &'a DimensionStyle>,
}

impl<'a> TableLookup<'a> {
    pub(crate) fn new(drawing: &'a Drawing2D, cx: &mut Cx<'_>) -> Result<Self, Failed> {
        let mut result = Self {
            layers: BTreeMap::new(),
            layer_ids: BTreeMap::new(),
            line_types: BTreeMap::new(),
            line_type_ids: BTreeMap::new(),
            line_type_items: BTreeMap::new(),
            text_style_ids: BTreeMap::new(),
            text_styles: BTreeMap::new(),
            dimension_styles: BTreeMap::new(),
        };
        let mut seen = BTreeMap::new();
        for layer in &drawing.layers {
            cx.check(Kind::Work, 1).map_err(|stop| stopped(cx, stop))?;
            result
                .layers
                .entry(&layer.name)
                .and_modify(|v| *v = None)
                .or_insert(Some(layer.id));
            result
                .layer_ids
                .entry(table_id(id::layer(layer), &mut seen))
                .and_modify(|v| *v = None)
                .or_insert(Some((&layer.name, layer.id)));
        }
        seen.clear();
        for line_type in &drawing.line_types {
            cx.check(Kind::Work, 1).map_err(|stop| stopped(cx, stop))?;
            result
                .line_types
                .entry(&line_type.name)
                .and_modify(|v| *v = None)
                .or_insert(Some(line_type.id));
            let id = table_id(id::line_type(line_type), &mut seen);
            result.line_type_items.insert(id.clone(), line_type);
            result
                .line_type_ids
                .entry(id)
                .and_modify(|v| *v = None)
                .or_insert(Some((&line_type.name, line_type.id)));
        }
        seen.clear();
        let mut names: BTreeMap<String, usize> = BTreeMap::new();
        for style in &drawing.text_styles {
            let work = style.name.len().saturating_add(1);
            cx.check(Kind::Work, u64::try_from(work).unwrap_or(u64::MAX))
                .map_err(|stop| stopped(cx, stop))?;
            *names.entry(style.name.to_ascii_lowercase()).or_default() += 1;
        }
        for (index, style) in drawing.text_styles.iter().enumerate() {
            cx.check(Kind::Work, 1).map_err(|stop| stopped(cx, stop))?;
            let unique = names.get(&style.name.to_ascii_lowercase()) == Some(&1);
            let id = table_id(id::text_style(style, index), &mut seen);
            result.text_styles.insert(id.clone(), style);
            result
                .text_style_ids
                .insert(id, unique.then_some(style.name.as_str()));
        }
        seen.clear();
        for (index, style) in drawing.dimension_styles.iter().enumerate() {
            cx.check(Kind::Work, 1).map_err(|stop| stopped(cx, stop))?;
            let id = table_id(id::dimension_style(style, index), &mut seen);
            result.dimension_styles.insert(id, style);
        }
        Ok(result)
    }
}

/// 文字スタイルを ID で指した形の値か
fn has_style_id(geometry: &messages::Geometry) -> bool {
    match geometry {
        messages::Geometry::Text(text) => text.style_id.is_some(),
        messages::Geometry::FormattedText(text) => text.style_id.is_some(),
        messages::Geometry::Leader(leader) => leader.style_id.is_some(),
        _ => false,
    }
}

/// 文字スタイルを ID で指した形の値を、名前で指す形の値にする。合わなければ、その欄の名前を返す
fn named_style<'g>(
    geometry: &'g messages::Geometry,
    tables: Option<&TableLookup<'_>>,
) -> Result<Cow<'g, messages::Geometry>, &'static str> {
    let (style, wanted) = match geometry {
        messages::Geometry::Text(text) => (&text.style, &text.style_id),
        messages::Geometry::FormattedText(text) => (&text.style, &text.style_id),
        // 引出線は寸法スタイルを指す。ID は、名前と同じ寸法スタイルを指すものだけを受ける
        messages::Geometry::Leader(leader) => {
            let Some(wanted) = &leader.style_id else {
                return Ok(Cow::Borrowed(geometry));
            };
            let found = tables
                .and_then(|tables| tables.dimension_styles.get(wanted.as_str()))
                .ok_or("geometry/style_id")?;
            if found.name != leader.style {
                return Err("geometry/style_id");
            }
            let mut geometry = geometry.clone();
            if let messages::Geometry::Leader(leader) = &mut geometry {
                leader.style_id = None;
            }
            return Ok(Cow::Owned(geometry));
        }
        _ => return Ok(Cow::Borrowed(geometry)),
    };
    let Some(wanted) = wanted else {
        return Ok(Cow::Borrowed(geometry));
    };
    let name = tables
        .and_then(|tables| {
            tables
                .text_style_ids
                .get(wanted.as_str())
                .copied()
                .flatten()
        })
        .ok_or("geometry/style_id")?
        .to_string();
    // 名前も書けば、同じ文字スタイルを指す（entities が返した形の値を、そのまま戻せる）
    if style
        .as_ref()
        .is_some_and(|style| !style.eq_ignore_ascii_case(&name))
    {
        return Err("geometry/style_id");
    }
    let mut geometry = geometry.clone();
    match &mut geometry {
        messages::Geometry::Text(text) => (text.style, text.style_id) = (Some(name), None),
        messages::Geometry::FormattedText(text) => {
            (text.style, text.style_id) = (Some(name), None);
        }
        _ => {}
    }
    Ok(Cow::Owned(geometry))
}

/// 名前か ID で指したレイヤ。どちらもなければ `None`。合わなければ、その欄の名前を返す
fn layer_of(
    name: Option<&str>,
    wanted: Option<&str>,
    tables: &TableLookup<'_>,
) -> Result<Option<LayerId>, &'static str> {
    Ok(match (name, wanted) {
        (None, None) => None,
        (Some(name), None) => Some(tables.layers.get(name).copied().flatten().ok_or("layer")?),
        (None, Some(wanted)) => {
            let (name, id) = tables
                .layer_ids
                .get(wanted)
                .copied()
                .flatten()
                .ok_or("layer_id")?;
            // DXF は名前で書くので、名前が 1 つに決まらないレイヤは ID でも書き戻さない
            if tables.layers.get(name).copied().flatten() != Some(id) {
                return Err("layer_id");
            }
            Some(id)
        }
        (Some(_), Some(_)) => return Err("layer_id"),
    })
}

/// 形を形の値で置き換えた要素。合わない欄があれば、その欄の名前を返す
pub(crate) fn count_old_element<P>(element: &Element<P>, cx: &mut Cx<'_>) -> Result<(), Failed> {
    let source = element.source.id().map_or(0, str::len);
    let size = match &element.shape {
        Shape::Text(text) => text.style.len(),
        Shape::Formatted(text) => text.style.len(),
        Shape::Symbol(symbol) => {
            // 属性と切り抜きは元のまま複製する。内容と区間は、たどる前に予算を確かめる。
            for attribute in &symbol.attributes {
                let size = 1usize
                    .saturating_add(attribute.tag.len())
                    .saturating_add(attribute.text.content.len())
                    .saturating_add(attribute.text.style.len())
                    .saturating_add(attribute.source.id().map_or(0, str::len));
                cx.check(Kind::Work, u64::try_from(size).unwrap_or(u64::MAX))
                    .map_err(|stop| stopped(cx, stop))?;
                if let Some(text) = &attribute.formatted {
                    let size = text.content.len().saturating_add(text.style.len());
                    cx.check(Kind::Work, u64::try_from(size).unwrap_or(u64::MAX))
                        .map_err(|stop| stopped(cx, stop))?;
                    for span in &text.spans {
                        let text = match &span.piece {
                            zumen_drawing::Piece::Text(text)
                            | zumen_drawing::Piece::Unparsed(text) => text.len(),
                            zumen_drawing::Piece::Stack { upper, lower, .. } => {
                                upper.len().saturating_add(lower.len())
                            }
                            _ => 0,
                        };
                        let font = span.format.font.as_ref().map_or(0, |font| font.name.len());
                        let size = 1usize.saturating_add(text).saturating_add(font);
                        cx.check(Kind::Work, u64::try_from(size).unwrap_or(u64::MAX))
                            .map_err(|stop| stopped(cx, stop))?;
                    }
                }
            }
            symbol
                .key
                .name
                .len()
                .saturating_add(symbol.clip.as_ref().map_or(0, |clip| clip.vertices.len()))
        }
        _ => 0,
    };
    cx.check(
        Kind::Work,
        u64::try_from(source.saturating_add(size)).unwrap_or(u64::MAX),
    )
    .map_err(|stop| stopped(cx, stop))?;
    Ok(())
}

fn reshaped<P: geometry::Spot>(
    element: &Element<P>,
    value: &messages::Geometry,
) -> Result<Element<P>, String> {
    let origin = geometry::origin_of(&element.shape).ok_or("geometry")?;
    let shape = geometry::shape_of(value, Some(&element.shape), origin)?;
    Ok(Element {
        source: element.source.clone(),
        layer: element.layer,
        style: element.style,
        shape,
    })
}

/// 足す要素。レイヤを指さなければ「0」に置き、色・線種・線幅はレイヤに従う。合わない欄があれば、その欄の名前を返す
fn added(
    edit: &messages::AddEdit,
    geometry: &messages::Geometry,
    places: &PlaceLookup,
    tables: &TableLookup<'_>,
    definition: Option<&zumen_drawing::SymbolDefinition>,
) -> Result<Placed, String> {
    let layer = match layer_of(edit.layer.as_deref(), edit.layer_id.as_deref(), tables)? {
        Some(layer) => layer,
        None => layer_of(Some("0"), None, tables)?.ok_or("layer")?,
    };
    // 書かない色・線種・線幅は、レイヤに従う
    let style = Style {
        color: match &edit.color {
            Some(change) => color_change(change).map_err(|()| "color")?,
            None => Inherit::Layer,
        },
        line_type: match &edit.line_type {
            Some(change) => line_type_change(change, tables).map_err(|()| "line_type")?,
            None => Inherit::Layer,
        },
        line_type_scale: 1.0,
        line_weight: Some(match &edit.line_weight {
            Some(change) => line_weight_change(change).map_err(|()| "line_weight")?,
            None => Inherit::Layer,
        }),
        transparency: Inherit::Layer,
    };
    if let Some(&origin) = places.spaces.get(&edit.place) {
        return Ok(Placed::Model(Element {
            source: Ref::default(),
            layer,
            style,
            shape: attributed(
                geometry::shape_of(geometry, None, origin)?,
                edit,
                definition,
            )?,
        }));
    }
    if let Some(&origin) = places.sheets.get(&edit.place) {
        return Ok(Placed::Paper(Element {
            source: Ref::default(),
            layer,
            style,
            shape: attributed(
                geometry::shape_of(geometry, None, origin)?,
                edit,
                definition,
            )?,
        }));
    }
    Err("place".to_string())
}

/// 足す記号に、定義の属性の定義から属性を付ける
fn attributed<P: geometry::Spot>(
    shape: Shape<P>,
    edit: &messages::AddEdit,
    definition: Option<&zumen_drawing::SymbolDefinition>,
) -> Result<Shape<P>, String> {
    match (shape, &edit.geometry) {
        (Shape::Symbol(mut symbol), messages::Geometry::Symbol(given)) => {
            symbol.attributes =
                geometry::attributes_of(definition, &symbol, given.attributes.as_deref())?;
            Ok(Shape::Symbol(symbol))
        }
        (shape, _) => Ok(shape),
    }
}

/// 要求のスタイルの変更を、図面モデルの変更にする。合わない欄があれば、その欄の名前を返す
fn restyle(
    edit: &messages::RestyleEdit,
    tables: &TableLookup<'_>,
) -> Result<Restyle, &'static str> {
    let layer = layer_of(edit.layer.as_deref(), edit.layer_id.as_deref(), tables)?;
    let color = edit
        .color
        .as_ref()
        .map(color_change)
        .transpose()
        .map_err(|()| "color")?;
    let line_type = edit
        .line_type
        .as_ref()
        .map(|change| line_type_change(change, tables))
        .transpose()
        .map_err(|()| "line_type")?;
    let line_type_scale = match edit.line_type_scale {
        Some(scale) if !(scale.is_finite() && scale > 0.0) => return Err("line_type_scale"),
        scale => scale,
    };
    let line_weight = edit
        .line_weight
        .as_ref()
        .map(line_weight_change)
        .transpose()
        .map_err(|()| "line_weight")?;
    Ok(Restyle {
        layer,
        color,
        line_type,
        line_type_scale,
        line_weight,
    })
}

/// 新しい色。合わなければ `Err`
fn color_change(change: &messages::ColorChange) -> Result<Inherit<Color>, ()> {
    let code = match (change.aci, change.sxf, change.jww) {
        (Some(value), None, None) => Some((ColorSystem::Aci, value)),
        (None, Some(value), None) => Some((ColorSystem::Sxf, value)),
        (None, None, Some(value)) => Some((ColorSystem::Jww, value)),
        (None, None, None) => None,
        _ => return Err(()),
    };
    Ok(match (change.inherit, code, change.rgb.as_deref()) {
        (Some(to), None, None) => inherit(to),
        (None, None, None) | (Some(_), _, _) => return Err(()),
        (None, code, rgb) => {
            let rgb = match rgb {
                None => None,
                Some(&[r, g, b]) => {
                    Some([r, g, b].map(|value| u8::try_from(value).unwrap_or(u8::MAX)))
                }
                Some(_) => return Err(()),
            };
            let code = match code {
                None => None,
                Some((system, value)) => Some(ColorCode {
                    system,
                    value: u32::try_from(value).map_err(|_| ())?,
                }),
            };
            Inherit::Own(Color { code, rgb })
        }
    })
}

/// 新しい線種。名前か ID が 1 つに決まらなければ `Err`
fn line_type_change(
    change: &messages::LineTypeChange,
    tables: &TableLookup<'_>,
) -> Result<Inherit<LineTypeId>, ()> {
    Ok(match (change.inherit, &change.name, &change.id) {
        (Some(to), None, None) => inherit(to),
        (None, None, Some(wanted)) => {
            let (name, id) = tables
                .line_type_ids
                .get(wanted)
                .copied()
                .flatten()
                .ok_or(())?;
            if tables.line_types.get(name).copied().flatten() != Some(id) {
                return Err(());
            }
            Inherit::Own(id)
        }
        (None, Some(name), None) => Inherit::Own(
            tables
                .line_types
                .get(name.as_str())
                .copied()
                .flatten()
                .ok_or(())?,
        ),
        _ => return Err(()),
    })
}

/// 新しい線幅
fn line_weight_change(change: &messages::LineWeightChange) -> Result<Inherit<LineWeight>, ()> {
    Ok(match (change.inherit, change.mm, change.default) {
        (Some(to), None, None) => inherit(to),
        (None, Some(mm), None) if mm.is_finite() && mm >= 0.0 => {
            Inherit::Own(LineWeight::Width(PaperLength(mm)))
        }
        (None, None, Some(true)) => Inherit::Own(LineWeight::Default),
        _ => return Err(()),
    })
}

/// レイヤの変更と追加の、書いた欄（決定 0162）
/// 文字スタイルの変更で書いた欄（決定 0163）
struct TextStyleFields<'e> {
    name: Option<&'e str>,
    font: Option<&'e messages::Font>,
    big_font: Option<&'e messages::Font>,
    height: Option<f64>,
    width: Option<f64>,
    oblique: Option<f64>,
    backward: Option<bool>,
    upside_down: Option<bool>,
    vertical: Option<bool>,
}

/// 文字スタイルの変更の名前とフォントの名前の長さ
fn text_style_work(
    name: Option<&str>,
    font: &Option<messages::Font>,
    big_font: &Option<messages::Font>,
) -> u64 {
    [
        name,
        font.as_ref().map(|f| f.name.as_str()),
        big_font.as_ref().map(|f| f.name.as_str()),
    ]
    .iter()
    .flatten()
    .map(|text| text.len() as u64)
    .sum()
}

/// フォントの指定。名前が空なら `None`。形のファイルは太字などを持てない。合わなければ `Err`
fn font_change(font: &messages::Font) -> Result<Option<zumen_drawing::Font>, ()> {
    if font.name.is_empty() {
        return Ok(None);
    }
    let kind = match font.kind {
        messages::FontKind::Shape => {
            let plain = font.bold.is_none()
                && font.italic.is_none()
                && font.charset.is_none()
                && font.pitch.is_none();
            plain.then_some(zumen_drawing::FontKind::Shape).ok_or(())?
        }
        messages::FontKind::Named => zumen_drawing::FontKind::Named {
            bold: font.bold.unwrap_or(false),
            italic: font.italic.unwrap_or(false),
            charset: font
                .charset
                .map(u32::try_from)
                .transpose()
                .map_err(|_| ())?,
            pitch: font.pitch.map(u32::try_from).transpose().map_err(|_| ())?,
        },
    };
    Ok(Some(zumen_drawing::Font {
        name: font.name.clone(),
        kind,
    }))
}

/// `base` に、書いた欄を重ねた文字スタイル。定義がなければ、Standard と同じ既定の値から始める。合わなければ、
/// その欄の名前を返す。書けるかどうか（フォントの種類・傾きの範囲など）は、apply が形式ごとに確かめる
fn text_style_change(
    base: &TextStyle,
    fields: &TextStyleFields<'_>,
) -> Result<TextStyle, &'static str> {
    let mut definition = base.definition.clone().unwrap_or(TextStyleDefinition {
        font: None,
        big_font: None,
        height: None,
        width: 1.0,
        oblique: 0.0,
        backward: false,
        upside_down: false,
        vertical: false,
    });
    if let Some(font) = fields.font {
        definition.font = font_change(font).map_err(|()| "font")?;
    }
    if let Some(font) = fields.big_font {
        definition.big_font = font_change(font).map_err(|()| "big_font")?;
    }
    if let Some(height) = fields.height {
        definition.height = match height {
            0.0 => None,
            height if height > 0.0 => Some(height),
            _ => return Err("height"),
        };
    }
    if let Some(width) = fields.width {
        (width > 0.0).then_some(()).ok_or("width")?;
        definition.width = width;
    }
    if let Some(oblique) = fields.oblique {
        definition.oblique = oblique;
    }
    definition.backward = fields.backward.unwrap_or(definition.backward);
    definition.upside_down = fields.upside_down.unwrap_or(definition.upside_down);
    definition.vertical = fields.vertical.unwrap_or(definition.vertical);
    Ok(TextStyle {
        name: fields.name.unwrap_or(&base.name).to_owned(),
        definition: Some(definition),
        source: base.source.clone(),
    })
}

/// 寸法スタイルの変更で書いた欄（決定 0165）
struct DimensionStyleFields<'e> {
    name: Option<&'e str>,
    ends: Option<&'e [String]>,
    leader: Option<&'e str>,
    size: Option<f64>,
    tick: Option<f64>,
    scale: Option<f64>,
}

impl DimensionStyleFields<'_> {
    /// 名前と端部記号の名前の長さと、端部記号の数
    fn work(&self) -> u64 {
        let ends = self.ends.unwrap_or_default();
        [self.name, self.leader]
            .iter()
            .flatten()
            .map(|text| text.len() as u64)
            .chain(ends.iter().map(|end| end.len() as u64))
            .fold(ends.len() as u64, u64::saturating_add)
    }
}

/// `base` に、書いた欄を重ねた寸法スタイル（決定 0165）。端部記号は DXF のブロックの名前で、空の名前は既定の矢印。
/// 合わなければ、その欄の名前を返す。書けるかどうか（ブロックがあるか・名前の決まり）は、apply が形式ごとに確かめる
fn dimension_style_edit(
    base: &DimensionStyle,
    fields: &DimensionStyleFields<'_>,
) -> Result<DimensionStyle, &'static str> {
    let arrow = |name: &str| {
        if name.is_empty() {
            Arrowhead::Default
        } else {
            Arrowhead::Symbol(SymbolKey {
                namespace: "dxf.block",
                name: name.to_owned(),
            })
        }
    };
    let length = |value: Option<f64>, field| match value {
        Some(value) if !(value.is_finite() && value >= 0.0) => Err(field),
        _ => Ok(value),
    };
    let mut style = base.clone();
    if let Some(name) = fields.name {
        name.clone_into(&mut style.name);
    }
    if let Some(ends) = fields.ends {
        let [first, second] = ends else {
            return Err("ends");
        };
        style.ends = [arrow(first), arrow(second)];
    }
    if let Some(leader) = fields.leader {
        style.leader = arrow(leader);
    }
    if let Some(size) = length(fields.size, "size")? {
        style.size = size;
    }
    if let Some(tick) = length(fields.tick, "tick")? {
        style.tick = (tick > 0.0).then_some(tick);
    }
    if let Some(scale) = length(fields.scale, "scale")? {
        style.scale = scale;
    }
    Ok(style)
}

/// 線種の変更の名前・説明の長さと列の要素の数
fn line_type_work(name: Option<&str>, description: Option<&str>, pattern: Option<&[f64]>) -> u64 {
    let text = [name, description]
        .iter()
        .flatten()
        .map(|text| text.len() as u64)
        .sum::<u64>();
    text.saturating_add(pattern.map_or(0, |pattern| pattern.len() as u64))
}

/// `base` に、書いた欄を重ねた線種（決定 0164）。定義がなければ、説明も列もない実線から始める。列を書けば、
/// 文書の単位の列にする。合わなければ、その欄の名前を返す。書けるかどうか（添える形と文字・要素の数）は、
/// apply が形式ごとに確かめる
fn line_type_edit(
    base: &LineType,
    name: Option<&str>,
    description: Option<&str>,
    pattern: Option<&[f64]>,
) -> Result<LineType, &'static str> {
    let mut definition = base.definition.clone().unwrap_or(LineTypeDefinition {
        description: String::new(),
        pattern: Vec::new(),
        basis: PatternBasis::Element,
        marks: Vec::new(),
    });
    if let Some(description) = description {
        definition.description = description.to_owned();
    }
    if let Some(pattern) = pattern {
        pattern
            .iter()
            .all(|length| length.is_finite())
            .then_some(())
            .ok_or("pattern")?;
        definition.pattern = pattern.to_vec();
        definition.basis = PatternBasis::Element;
    }
    Ok(LineType {
        id: base.id,
        name: name.unwrap_or(&base.name).to_owned(),
        definition: Some(definition),
        source: base.source.clone(),
    })
}

struct LayerFields<'e> {
    name: Option<&'e str>,
    visible: Option<bool>,
    printable: Option<bool>,
    editable: Option<bool>,
    frozen: Option<bool>,
    color: Option<&'e messages::ColorChange>,
    line_type: Option<&'e messages::LineTypeChange>,
    line_weight: Option<&'e messages::LineWeightChange>,
}

/// `base` に、書いた欄を重ねたレイヤ。凍結するだけなら表示もしない。表示するだけか、凍結を解くだけなら、
/// 凍結を解いて表示する。継承は書けない。合わなければ、その欄の名前を返す
fn layer_change(
    base: &Layer,
    fields: &LayerFields<'_>,
    tables: &TableLookup<'_>,
) -> Result<Layer, &'static str> {
    let was = base.state.unwrap_or(LayerState {
        visible: true,
        printable: true,
        editable: true,
        frozen: false,
    });
    let frozen = fields
        .frozen
        .unwrap_or(was.frozen && fields.visible != Some(true));
    // 凍結を解いた画層は、表示する
    let visible = fields
        .visible
        .unwrap_or(!frozen && (was.visible || was.frozen));
    let state = LayerState {
        visible,
        printable: fields.printable.unwrap_or(was.printable),
        editable: fields.editable.unwrap_or(was.editable),
        frozen,
    };
    let mut style = base.style.unwrap_or(LayerStyle {
        color: None,
        line_type: None,
        line_weight: None,
        transparency: None,
    });
    if let Some(change) = fields.color {
        match color_change(change) {
            Ok(Inherit::Own(color)) => style.color = Some(color),
            _ => return Err("color"),
        }
    }
    if let Some(change) = fields.line_type {
        match line_type_change(change, tables) {
            Ok(Inherit::Own(line_type)) => style.line_type = Some(line_type),
            _ => return Err("line_type"),
        }
    }
    if let Some(change) = fields.line_weight {
        match line_weight_change(change) {
            Ok(Inherit::Own(weight)) => style.line_weight = Some(weight),
            _ => return Err("line_weight"),
        }
    }
    let mut layer = base.clone();
    if let Some(name) = fields.name {
        layer.name = name.to_owned();
    }
    layer.state = Some(state);
    layer.style = Some(style);
    Ok(layer)
}

/// 継承する先
fn inherit<T>(inherit: messages::Inherit) -> Inherit<T> {
    match inherit {
        messages::Inherit::Layer => Inherit::Layer,
        messages::Inherit::Block => Inherit::Block,
    }
}
