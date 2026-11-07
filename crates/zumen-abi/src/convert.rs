//! 開いた図面を新しい DXF に書く（決定 0139・0206・0281）。対応する R2004 DWG では共有の表を再符号化する。
//!
//! 図面モデルから書くときは、形式ごとのひな形（表の既定の項目と記号の定義の空の部品を持つファイル。DXF のひな形は
//! `zumen-acad` の `NewDxfTemplate` が作り、レイアウトと用紙のビューポートも持つ）を作り、図面モデルの線種・
//! 文字スタイル・寸法スタイル・レイヤ・要素を、書き戻し（apply）の追加で足す。書けない要素は落とし、種類ごとの
//! 件数を診断にする。

use std::collections::{BTreeMap, BTreeSet};

use zumen_acad::{Dxf, DxfFile, NewDxfTemplate};
use zumen_base::{Cx, Diagnostic, Diagnostics, Kind, Ref, Value, codes};
use zumen_drawing::{
    Changes, Color, ColorCode, ColorSystem, DimensionStyle, Drawing2D, Edit, Element, Inherit,
    Layer, LayerId, LayerState, LayerStyle, LineType, LineTypeId, ModelPoint, PaperPoint, Placed,
    Shape, SheetItem, SpaceId, TextStyle, id,
};
use zumen_sdk::{Apply, Codec, Error, Lift};

use crate::bake;
use crate::geometry;
use crate::messages::Geometry;
use crate::session::{Failed, fail, stopped};

/// 書いたファイルと、足した要素の数
pub(crate) struct Converted {
    pub(crate) file: DxfFile,
    pub(crate) bytes: Vec<u8>,
    pub(crate) elements: u64,
}

/// R2004 DWG は共有の表を一度だけ読む。再符号化の範囲外なら同じ表から図面モデルを作る。
pub(crate) fn from_r2004(
    source: &zumen_acad::DwgFile,
    cx: &mut Cx<'_>,
) -> Result<Converted, Failed> {
    match source.reencode_r2004_dxf(cx).map_err(|_| Failed)? {
        zumen_acad::dxf::R2004Conversion::Dxf(encoded) => {
            let file = Dxf::read(&encoded.bytes, cx).map_err(|_| Failed)?;
            Ok(Converted {
                file,
                bytes: encoded.bytes,
                elements: encoded.elements,
            })
        }
        zumen_acad::dxf::R2004Conversion::DrawingSource(db) => {
            let drawing = zumen_acad::lift_drawing(&db, cx).map_err(|_| Failed)?;
            to_dxf(&drawing, cx)
        }
    }
}

/// 図面モデルを、新しい DXF（R2004）に書く
pub(crate) fn to_dxf(drawing: &Drawing2D, cx: &mut Cx<'_>) -> Result<Converted, Failed> {
    // 外部参照の記号の定義は、ひな形に書かない（`NewDxfTemplate::new` と同じ条件。キーが重ならない定義のとき。
    // 重なるときの扱いは #656）。それを置いた記号は、下の要素で報告する
    for definition in drawing
        .symbols
        .iter()
        .filter(|definition| definition.external.is_some())
    {
        report_dropped(cx, "external_symbol_definition", &definition.source);
    }
    let template = NewDxfTemplate::new(drawing);
    // ひな形を符号化できなかったとき（不具合）は空。空のバイト列も読めてしまうので、ここで内部の失敗にする
    if template.bytes().is_empty() {
        return Err(fail(cx, Diagnostic::new(codes::INTERNAL_PANIC_CAUGHT)));
    }
    cx.check(Kind::Work, template.bytes().len() as u64)
        .map_err(|stop| stopped(cx, stop))?;
    let mut file =
        quiet(cx, |cx| Dxf::read(template.bytes(), cx)).map_err(|error| failed(cx, error))?;

    // 1. 線種・文字スタイル・寸法スタイル。レイヤは足した線種を同じ一覧で指せないので、先に足す
    let target = lift(&file, cx)?;
    let mut edits = Vec::new();
    for line_type in &drawing.line_types {
        cx.check(Kind::Work, 1).map_err(|stop| stopped(cx, stop))?;
        if line_type.definition.is_some() && find_line_type(&target, &line_type.name).is_none() {
            edits.push(Addition::new(
                Edit::AddLineType {
                    line_type: Box::new(line_type.clone()),
                },
                "line_type",
                &line_type.source,
            ));
        }
    }
    for style in &drawing.text_styles {
        cx.check(Kind::Work, 1).map_err(|stop| stopped(cx, stop))?;
        if style.definition.is_some() && target.text_style(&style.name).is_none() {
            edits.push(Addition::new(
                Edit::AddTextStyle {
                    style: Box::new(TextStyle {
                        name: style.name.clone(),
                        definition: style.definition.clone(),
                        source: zumen_base::Ref::default(),
                    }),
                },
                "text_style",
                &style.source,
            ));
        }
    }
    for style in &drawing.dimension_styles {
        cx.check(Kind::Work, 1).map_err(|stop| stopped(cx, stop))?;
        if !target
            .dimension_styles
            .iter()
            .any(|known| known.name.eq_ignore_ascii_case(&style.name))
        {
            edits.push(Addition::new(
                Edit::AddDimensionStyle {
                    style: Box::new(DimensionStyle {
                        source: zumen_base::Ref::default(),
                        ..style.clone()
                    }),
                },
                "dimension_style",
                &style.source,
            ));
        }
    }
    add_all(&mut file, edits, cx)?;

    // 2. レイヤ。線種は、書いた先の ID に直す
    let target = lift(&file, cx)?;
    let mut edits = Vec::new();
    let mut fields = Vec::new();
    let mut reused = BTreeSet::new();
    for layer in &drawing.layers {
        cx.check(Kind::Work, 1).map_err(|stop| stopped(cx, stop))?;
        if let Some(existing) = find_layer(&target, &layer.name) {
            // 同じひな形の画層を指す入力が複数あれば先の画層を使う。
            if !reused.insert(existing.id) {
                report_layer_fields(layer, cx);
            } else if has_layer_fields(layer) {
                fields.push(layer);
            }
            continue;
        }
        // 画層の通常の値を先に書き、追加できた画層の拡張データを確定時に書く。
        let style = layer.style.unwrap_or(LayerStyle {
            color: None,
            line_type: None,
            line_weight: None,
            transparency: None,
        });
        let line_type = style
            .line_type
            .and_then(|line_type| map_line_type(drawing, &target, line_type))
            .or_else(|| find_line_type(&target, "Continuous").map(|line_type| line_type.id));
        let color = style.color.map_or(
            Color {
                code: Some(ACI_WHITE),
                rgb: None,
            },
            |color| Color {
                code: Some(color.code.filter(is_aci).unwrap_or(ACI_WHITE)),
                ..color
            },
        );
        let state = layer.state.map_or(
            LayerState {
                visible: true,
                printable: true,
                editable: true,
                frozen: false,
            },
            |state| LayerState {
                visible: state.visible && !state.frozen,
                ..state
            },
        );
        let mut addition = Addition::new(
            Edit::AddLayer {
                layer: Box::new(Layer {
                    id: layer.id,
                    name: layer.name.clone(),
                    follows_placement: layer.follows_placement,
                    state: Some(state),
                    style: Some(LayerStyle {
                        color: Some(color),
                        line_type,
                        transparency: None,
                        ..style
                    }),
                    description: None,
                    source: zumen_base::Ref::default(),
                }),
            },
            "layer",
            &layer.source,
        );
        addition.layer_fields = has_layer_fields(layer).then_some(layer);
        edits.push(addition);
    }
    fields.extend(add_all(&mut file, edits, cx)?.layer_fields);

    // 3. 要素。記号の定義の中身を先に、実寸の空間の要素を後に足す
    let target = lift(&file, cx)?;
    let places = definition_places(&target);
    let Some(model) = target
        .spaces
        .first()
        .map(|space| model_place(id::space(space.id), space.id))
    else {
        return Err(fail(cx, Diagnostic::new(codes::INTERNAL_PANIC_CAUGHT)));
    };
    let maps = Maps {
        source: drawing,
        target: &target,
        blocks: &template,
    };
    let mut edits = Vec::new();
    for definition in &drawing.symbols {
        let Some(name) = template.block(&definition.key) else {
            continue;
        };
        let Some(place) = places.get(&name.to_lowercase()) else {
            continue;
        };
        for element in &definition.space.elements {
            cx.check(Kind::Work, 1).map_err(|stop| stopped(cx, stop))?;
            maps.element(element, place, &mut edits, cx);
        }
        for attribute in &definition.attributes {
            cx.check(Kind::Work, 1).map_err(|stop| stopped(cx, stop))?;
            if attribute.constant && !attribute.attribute.hidden {
                let text = Element {
                    source: attribute.attribute.source.clone(),
                    layer: attribute.attribute.layer,
                    style: attribute.attribute.style,
                    shape: Shape::Text(attribute.attribute.text.clone()),
                };
                maps.element(&text, place, &mut edits, cx);
            } else {
                report_dropped(cx, "attribute_definition", &attribute.attribute.source);
            }
        }
    }
    for space in drawing
        .spaces
        .iter()
        .filter(|space| modelled(drawing, space.id))
    {
        for element in &space.elements {
            cx.check(Kind::Work, 1).map_err(|stop| stopped(cx, stop))?;
            maps.element(element, &model, &mut edits, cx);
        }
    }
    // 用紙は 1 つだけ書ける（DXF の Paper_Space）。ほかの用紙は落とす
    let paper = target.sheets.first().map(|sheet| Place {
        id: id::sheet(sheet),
        origin: PaperPoint {
            sheet: sheet.id,
            x: 0.0,
            y: 0.0,
        },
        wrap: Placed::Paper,
    });
    for (index, sheet) in drawing.sheets.iter().enumerate() {
        cx.check(Kind::Work, 1).map_err(|stop| stopped(cx, stop))?;
        let Some(paper) = paper.as_ref().filter(|_| index == 0) else {
            report_dropped(cx, "sheet", &sheet.source);
            for view in &sheet.views {
                report_dropped(cx, "view", &view.source);
            }
            continue;
        };
        // 要素とビューを描く順に書く。横と縦の尺度の違うビューは、用紙の要素に焼き込む（決定 0238）。
        // ほかのビューは書けない
        let items: Vec<SheetItem> = if sheet.order.len() == sheet.elements.len() + sheet.views.len()
        {
            sheet.order.clone()
        } else {
            (0..sheet.elements.len())
                .map(SheetItem::Element)
                .chain((0..sheet.views.len()).map(SheetItem::View))
                .collect()
        };
        for item in items {
            cx.check(Kind::Work, 1).map_err(|stop| stopped(cx, stop))?;
            match item {
                SheetItem::Element(index) => {
                    if let Some(element) = sheet.elements.get(index) {
                        maps.element(element, paper, &mut edits, cx);
                    }
                }
                SheetItem::View(index) => {
                    let Some(view) = sheet.views.get(index) else {
                        continue;
                    };
                    if NewDxfTemplate::writes_viewport(view, sheet) {
                        continue;
                    }
                    let space = drawing.spaces.iter().find(|space| space.id == view.space);
                    let Some(space) = space.filter(|_| bake::bakes(view)) else {
                        report_dropped(cx, "view", &view.source);
                        continue;
                    };
                    cx.report(
                        Diagnostic::new(codes::CONVERT_VIEW_APPROXIMATED)
                            .with("format", "DXF")
                            .at(view.source.clone()),
                    );
                    for element in bake::bake(space, view) {
                        cx.check(Kind::Work, 1).map_err(|stop| stopped(cx, stop))?;
                        match element {
                            Ok(element) => maps.element(&element, paper, &mut edits, cx),
                            Err(element) => {
                                report_dropped(cx, kind_of(&element.shape), &element.source)
                            }
                        }
                    }
                }
            }
        }
    }
    let elements = add_all(&mut file, edits, cx)?.elements;
    let db = quiet(cx, |cx| file.objects(cx)).map_err(|error| failed(cx, error))?;
    let db = db
        .finish_new_dxf_tables(&fields, cx)
        .map_err(|error| failed(cx, error))?;
    quiet(cx, |cx| file.write_back(&db, cx)).map_err(|error| failed(cx, error))?;
    let mut bytes = Vec::new();
    Dxf::write(&file, &mut bytes, cx).map_err(|_| Failed)?;
    Ok(Converted {
        file,
        bytes,
        elements,
    })
}

fn report_dropped(cx: &mut Cx<'_>, kind: &'static str, source: &Ref) {
    cx.report(
        Diagnostic::new(codes::CONVERT_ELEMENT_UNSUPPORTED)
            .with("kind", Value::Key(kind))
            .with("format", "DXF")
            .at(source.clone()),
    );
}

fn has_layer_fields(layer: &Layer) -> bool {
    layer.description.is_some() || layer.style.and_then(|style| style.transparency).is_some()
}

fn report_layer_fields(layer: &Layer, cx: &mut Cx<'_>) {
    let transparency = layer.style.and_then(|style| style.transparency);
    for (field, present) in [
        ("description", layer.description.is_some()),
        ("transparency", transparency.is_some()),
    ] {
        if present {
            cx.report(
                Diagnostic::new(codes::CONVERT_LAYER_FIELD_DROPPED)
                    .with("field", Value::Key(field))
                    .with("format", "DXF")
                    .at(layer.source.clone()),
            );
        }
    }
}

/// 追加と、失敗したときに返す元の種類・場所。属性の描画は、追加が通ってから数える。
struct Addition<'a> {
    edit: Edit,
    kind: &'static str,
    source: Ref,
    detached: bool,
    layer_fields: Option<&'a Layer>,
}

impl Addition<'_> {
    fn new(edit: Edit, kind: &'static str, source: &Ref) -> Self {
        Self {
            edit,
            kind,
            source: source.clone(),
            detached: false,
            layer_fields: None,
        }
    }
}

/// 書いた先の表と記号の定義の名前への対応
struct Maps<'a> {
    source: &'a Drawing2D,
    target: &'a Drawing2D,
    blocks: &'a NewDxfTemplate,
}

impl Maps<'_> {
    /// 要素を書いた先の表と空間に合わせて直し、足す変更にする。形は API の形の値（`add` が受けるもの）を
    /// 通して作り直す。記号の属性は、記号から離した文字にする
    fn element<P: geometry::Spot + Clone>(
        &self,
        element: &Element<P>,
        place: &Place<P>,
        edits: &mut Vec<Addition<'_>>,
        cx: &mut Cx<'_>,
    ) {
        let kind = kind_of(&element.shape);
        let Some(mut geometry) = geometry::geometry_of(&element.shape) else {
            report_dropped(cx, kind, &element.source);
            return;
        };
        let mut detached = Vec::new();
        match &mut geometry {
            Geometry::Text(text) => self.text_style(&mut text.style, &mut text.style_id),
            Geometry::FormattedText(text) => {
                self.text_style(&mut text.style, &mut text.style_id);
            }
            Geometry::Symbol(symbol) => {
                let Shape::Symbol(source) = &element.shape else {
                    report_dropped(cx, kind, &element.source);
                    return;
                };
                let Some(name) = self.blocks.block(&source.key) else {
                    report_dropped(cx, "external_symbol", &element.source);
                    return;
                };
                symbol.name = name.to_owned();
                symbol.attributes = None;
                for attribute in &source.attributes {
                    if !attribute.hidden {
                        detached.push(Element {
                            source: attribute.source.clone(),
                            layer: attribute.layer,
                            style: attribute.style,
                            shape: Shape::Text(attribute.text.clone()),
                        });
                    }
                }
            }
            _ => {}
        }
        let Ok(shape) = geometry::shape_of(&geometry, None, place.origin) else {
            report_dropped(cx, kind, &element.source);
            return;
        };
        let Some(layer) = self.layer(element.layer) else {
            report_dropped(cx, kind, &element.source);
            return;
        };
        let element_of = |shape| Element {
            source: zumen_base::Ref::default(),
            layer,
            style: self.style(&element.style),
            shape,
        };
        edits.push(Addition::new(
            Edit::Add {
                place: place.id.clone(),
                element: Box::new((place.wrap)(element_of(shape))),
            },
            kind,
            &element.source,
        ));
        for text in detached {
            let before = edits.len();
            self.element(&text, place, edits, cx);
            if let Some(addition) = edits.get_mut(before) {
                addition.detached = true;
            }
        }
    }

    fn layer(&self, layer: LayerId) -> Option<LayerId> {
        let name = &self.source.layer(layer)?.name;
        find_layer(self.target, name).map(|layer| layer.id)
    }

    fn style(&self, style: &zumen_drawing::Style) -> zumen_drawing::Style {
        let line_type = match style.line_type {
            Inherit::Own(line_type) => map_line_type(self.source, self.target, line_type)
                .map_or(Inherit::Layer, Inherit::Own),
            other => other,
        };
        // 番号の色は ACI だけを書ける。ほかの体系の色は、RGB があれば RGB で、なければ白で書く
        let color = match style.color {
            Inherit::Own(color) if !color.code.is_some_and(|code| is_aci(&code)) => {
                Inherit::Own(match color.rgb {
                    Some(_) => Color {
                        code: None,
                        ..color
                    },
                    None => Color {
                        code: Some(ACI_WHITE),
                        rgb: None,
                    },
                })
            }
            other => other,
        };
        zumen_drawing::Style {
            color,
            line_type,
            ..*style
        }
    }

    /// 文字スタイルは名前で指す。書いた先にないものは、Standard にする
    fn text_style(&self, style: &mut Option<String>, id: &mut Option<String>) {
        *id = None;
        let known = style
            .as_deref()
            .is_some_and(|name| self.target.text_style(name).is_some());
        if !known {
            *style = Some("Standard".to_string());
        }
    }
}

/// 足す先の空間か用紙の ID と、そこの原点
struct Place<P> {
    id: String,
    origin: P,
    wrap: fn(Element<P>) -> Placed,
}

fn model_place(id: String, space: SpaceId) -> Place<ModelPoint> {
    Place {
        id,
        origin: ModelPoint {
            space,
            x: 0.0,
            y: 0.0,
        },
        wrap: Placed::Model,
    }
}

#[derive(Default)]
struct Added<'a> {
    elements: u64,
    layer_fields: Vec<&'a Layer>,
}

/// 変更をまとめて当て、当てられなければ半分に分けて当て直す。1 つでも当てられない変更は落として数える。
/// 当てられなかった試しの診断は捨てる
fn add_all<'a>(
    file: &mut DxfFile,
    edits: Vec<Addition<'a>>,
    cx: &mut Cx<'_>,
) -> Result<Added<'a>, Failed> {
    let mut added = Added::default();
    let mut pending = vec![edits];
    while let Some(edits) = pending.pop() {
        if edits.is_empty() {
            continue;
        }
        cx.check(Kind::Work, edits.len() as u64)
            .map_err(|stop| stopped(cx, stop))?;
        let changes = Changes {
            edits: edits.iter().map(|addition| addition.edit.clone()).collect(),
            document: None,
        };
        match quiet(cx, |cx| file.apply(&changes, cx)) {
            Ok(_) => {
                added
                    .layer_fields
                    .extend(edits.iter().filter_map(|a| a.layer_fields));
                added.elements += edits
                    .iter()
                    .filter(|addition| matches!(addition.edit, Edit::Add { .. }))
                    .count() as u64;
                for addition in edits.iter().filter(|addition| addition.detached) {
                    cx.report(
                        Diagnostic::new(codes::CONVERT_ATTRIBUTE_DRAWN)
                            .with("format", "DXF")
                            .at(addition.source.clone()),
                    );
                }
            }
            Err(Error::Rejected | Error::Invalid | Error::Unsupported) => {
                if let [addition] = edits.as_slice() {
                    report_dropped(cx, addition.kind, &addition.source);
                    if let Some(layer) = addition.layer_fields {
                        report_layer_fields(layer, cx);
                    }
                } else {
                    let mut first = edits;
                    let second = first.split_off(first.len() / 2);
                    // 後に pop するほうを先に当てる
                    pending.push(second);
                    pending.push(first);
                }
            }
            Err(error) => return Err(failed(cx, error)),
        }
    }
    Ok(added)
}

/// 診断を捨てて走らせる。止めた理由だけは残す
fn quiet<T>(
    cx: &mut Cx<'_>,
    run: impl FnOnce(&mut Cx<'_>) -> Result<T, Error>,
) -> Result<T, Error> {
    let mut scratch = Diagnostics::new();
    let mut inner = Cx {
        budget: &mut *cx.budget,
        diag: &mut scratch,
        cancel: cx.cancel,
    };
    run(&mut inner)
}

fn failed(cx: &mut Cx<'_>, error: Error) -> Failed {
    match error {
        Error::Stopped(stop) => stopped(cx, stop),
        // ひな形を読めないのは不具合
        _ => fail(cx, Diagnostic::new(codes::INTERNAL_PANIC_CAUGHT)),
    }
}

fn lift(file: &DxfFile, cx: &mut Cx<'_>) -> Result<Drawing2D, Failed> {
    quiet(cx, |cx| file.lift(cx)).map_err(|error| failed(cx, error))
}

fn find_layer<'d>(drawing: &'d Drawing2D, name: &str) -> Option<&'d Layer> {
    drawing
        .layers
        .iter()
        .find(|layer| layer.name.eq_ignore_ascii_case(name))
}

fn find_line_type<'d>(drawing: &'d Drawing2D, name: &str) -> Option<&'d LineType> {
    drawing
        .line_types
        .iter()
        .find(|line_type| line_type.name.eq_ignore_ascii_case(name))
}

fn map_line_type(source: &Drawing2D, target: &Drawing2D, id: LineTypeId) -> Option<LineTypeId> {
    let name = &source.line_type(id)?.name;
    find_line_type(target, name).map(|line_type| line_type.id)
}

/// 書いた先の部品の名前（小文字）ごとの、記号の定義の空間
fn definition_places(target: &Drawing2D) -> BTreeMap<String, Place<ModelPoint>> {
    let mut namer = id::DefinitionNamer::default();
    target
        .symbols
        .iter()
        .map(|definition| {
            let place = model_place(namer.next(definition), definition.space.id);
            (definition.key.name.to_lowercase(), place)
        })
        .collect()
}

fn kind_of<P>(shape: &Shape<P>) -> &'static str {
    match shape {
        Shape::Line { .. } => "line",
        Shape::Circle { .. } => "circle",
        Shape::Curve(_) => "curve",
        Shape::Polyline(_) => "polyline",
        Shape::Text(_) => "text",
        Shape::Formatted(_) => "formatted_text",
        Shape::Dimension(_) => "dimension",
        Shape::Leader(_) => "leader",
        Shape::Callout(_) => "callout",
        Shape::Table(_) => "table",
        Shape::Underlay(_) => "underlay",
        Shape::Symbol(_) => "symbol",
        Shape::Hatch(_) => "hatch",
        Shape::Point(_) => "point",
    }
}

/// ACI の白（7）
const ACI_WHITE: ColorCode = ColorCode {
    system: ColorSystem::Aci,
    value: 7,
};

fn is_aci(code: &ColorCode) -> bool {
    code.system == ColorSystem::Aci && (1..=255).contains(&code.value)
}

/// 実寸の空間 `space` を、DXF のモデル空間に書くか。どの用紙のビューにも置かれない空間と、書く用紙（1 枚目）が
/// 焼き込まないビューで置く空間を書く。焼き込むビューだけが置く空間は、用紙に焼き込んだ要素と重ねて書かない
/// （決定 0240）
fn modelled(drawing: &Drawing2D, space: SpaceId) -> bool {
    let mut views = drawing
        .sheets
        .iter()
        .flat_map(|sheet| &sheet.views)
        .filter(|view| view.space == space)
        .peekable();
    views.peek().is_none()
        || drawing.sheets.first().is_some_and(|sheet| {
            sheet
                .views
                .iter()
                .any(|view| view.space == space && !bake::bakes(view))
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use zumen_acad::db::{Data, Section};
    use zumen_base::{Budget, CancelToken, Limits, Ref};
    use zumen_civil::{Alignment, CivilModel, Horizontal, HorizontalKind, Vertical, VerticalKind};
    use zumen_crs::MapPoint;
    use zumen_drawing::{Axes, ModelSpace, Symbol, SymbolDefinition, SymbolKey, Unit};

    /// R2004 の表・図形・レイアウトを結ぶ参照は、すべてファイルの中の正しい種類を指す。
    fn r2004_structure(file: &DxfFile, cx: &mut Cx<'_>) {
        let db = file.objects(cx).unwrap();
        let mut handles = std::collections::BTreeSet::new();
        for object in db.objects() {
            let Some(handle) = object.handle() else {
                assert_eq!(object.kind, b"ENDTAB");
                continue;
            };
            assert!(handles.insert(handle), "同じハンドルがある: {handle:X}");
            let owner = object.owner().unwrap();
            if owner != 0 {
                assert!(db.get(owner).is_some(), "所有者がない: {owner:X}");
            }
            let subclass = |name: &[u8]| {
                object
                    .values
                    .iter()
                    .any(|value| value.code == 100 && value.data == Data::Text(name.to_vec()))
            };
            match object.kind.as_slice() {
                b"TABLE" => {
                    assert!(subclass(b"AcDbSymbolTable"));
                    let entries = db
                        .objects()
                        .iter()
                        .filter(|entry| {
                            entry.section == Section::Tables && entry.owner() == Some(handle)
                        })
                        .count();
                    assert_eq!(object.first(70), Some(&Data::Int(entries as i64)));
                }
                b"LTYPE" | b"LAYER" | b"STYLE" | b"DIMSTYLE" | b"BLOCK_RECORD" => {
                    assert!(subclass(b"AcDbSymbolTableRecord"));
                    assert_eq!(db.get(owner).unwrap().kind, b"TABLE");
                }
                b"BLOCK" | b"ENDBLK" | b"VIEWPORT" | b"LINE" | b"LWPOLYLINE" | b"TEXT" => {
                    assert!(subclass(b"AcDbEntity"));
                    assert_eq!(db.get(owner).unwrap().kind, b"BLOCK_RECORD");
                    if object.kind == b"VIEWPORT" {
                        assert!(subclass(b"AcDbViewport"));
                    }
                }
                b"LAYOUT" => {
                    assert!(subclass(b"AcDbPlotSettings") && subclass(b"AcDbLayout"));
                    assert_eq!(db.get(owner).unwrap().kind, b"DICTIONARY");
                    let block = object
                        .values
                        .iter()
                        .rfind(|value| value.code == 330)
                        .unwrap();
                    let Data::Handle(block) = block.data else {
                        panic!("ブロックの参照がない")
                    };
                    let block = db.get(block).unwrap();
                    assert_eq!(block.kind, b"BLOCK_RECORD");
                    assert_eq!(block.first(340), Some(&Data::Handle(handle)));
                }
                _ => {}
            }
        }
        let seed = &db.variable(b"$HANDSEED").unwrap()[0].data;
        let Data::Handle(seed) = seed else {
            panic!("次のハンドルがない")
        };
        assert!(handles.last().unwrap() < seed);
        assert_eq!(
            db.objects().iter().filter(|o| o.kind == b"LAYOUT").count(),
            2
        );
        assert!(!db.objects().iter().any(|o| o.kind == b"POINT"));
    }

    /// 北へ 220 m の直線と、勾配・放物線・勾配の縦断
    fn model() -> CivilModel {
        let segment = |start_distance, start_height, start_gradient, length, kind| Vertical {
            start_distance,
            start_height,
            start_gradient,
            length,
            kind,
            source: Ref::default(),
        };
        CivilModel {
            alignments: vec![Alignment {
                id: "本線".to_string(),
                horizontal: vec![Horizontal {
                    start: MapPoint {
                        northing: 0.0,
                        easting: 0.0,
                    },
                    direction: 0.0,
                    length: 220.0,
                    kind: HorizontalKind::Line,
                    source: Ref::default(),
                }],
                vertical: vec![
                    segment(0.0, 50.0, 0.02, 100.0, VerticalKind::ConstantGradient),
                    segment(
                        100.0,
                        52.0,
                        0.02,
                        40.0,
                        VerticalKind::Parabola {
                            end_gradient: -0.01,
                        },
                    ),
                    segment(140.0, 52.4, -0.01, 80.0, VerticalKind::ConstantGradient),
                ],
                ..Alignment::default()
            }],
            ..CivilModel::default()
        }
    }

    fn points(shape: &Shape<PaperPoint>) -> Option<Vec<[f64; 2]>> {
        match shape {
            Shape::Line { start, end } => Some(vec![[start.x, start.y], [end.x, end.y]]),
            Shape::Polyline(polyline) => Some(
                polyline
                    .vertices
                    .iter()
                    .map(|v| [v.point.x, v.point.y])
                    .collect(),
            ),
            _ => None,
        }
    }

    #[test]
    fn a_profile_is_baked_into_the_paper_and_reads_back_the_same() {
        let mut budget = Budget::new(Limits::default());
        let mut diag = Diagnostics::new();
        let cancel = CancelToken::new();
        let (read, expected) = {
            let mut cx = Cx::new(&mut budget, &mut diag, &cancel);
            let model = model();
            let mut drawing = zumen_civil_drawing::drawing(
                &model,
                &model.alignments[0],
                &zumen_civil_drawing::Options::default(),
                &mut cx,
            )
            .unwrap();
            // 縦断図の用紙だけを書く
            let profile = drawing.sheets.remove(1);
            let view = profile.views[0].clone();
            assert!(bake::bakes(&view));
            let space = drawing
                .spaces
                .iter()
                .find(|space| space.id == view.space)
                .unwrap();
            let expected: Vec<Vec<[f64; 2]>> = bake::bake(space, &view)
                .map(|element| points(&element.unwrap().shape).unwrap())
                .chain(
                    profile
                        .elements
                        .iter()
                        .filter_map(|element| points(&element.shape)),
                )
                .collect();
            assert!(expected.len() >= 4);
            drawing.sheets = vec![profile];
            let Ok(converted) = to_dxf(&drawing, &mut cx) else {
                panic!("書けない");
            };
            r2004_structure(&converted.file, &mut cx);
            let Ok(read) = lift(&converted.file, &mut cx) else {
                panic!("読めない");
            };
            (read, expected)
        };
        let codes: Vec<_> = diag.iter().map(|d| d.code()).collect();
        assert!(
            codes.contains(&codes::CONVERT_VIEW_APPROXIMATED),
            "{codes:?}"
        );
        let written: Vec<Vec<[f64; 2]>> = read.sheets[0]
            .elements
            .iter()
            .filter_map(|element| points(&element.shape))
            .collect();
        assert_eq!(written.len(), expected.len());
        for shape in &expected {
            let same = |found: &Vec<[f64; 2]>| {
                found.len() == shape.len()
                    && found
                        .iter()
                        .zip(shape)
                        .all(|(a, b)| (a[0] - b[0]).abs() < 1e-9 && (a[1] - b[1]).abs() < 1e-9)
            };
            assert!(written.iter().any(same), "{shape:?}");
        }
    }

    #[test]
    fn a_plan_view_is_written_as_a_viewport_and_reads_back_the_same() {
        let mut budget = Budget::new(Limits::default());
        let mut diag = Diagnostics::new();
        let cancel = CancelToken::new();
        let (read, view) = {
            let mut cx = Cx::new(&mut budget, &mut diag, &cancel);
            let model = model();
            let mut drawing = zumen_civil_drawing::drawing(
                &model,
                &model.alignments[0],
                &zumen_civil_drawing::Options::default(),
                &mut cx,
            )
            .unwrap();
            // 平面図の用紙だけを書く
            drawing.sheets.truncate(1);
            let view = drawing.sheets[0].views[0].clone();
            assert!(NewDxfTemplate::writes_viewport(&view, &drawing.sheets[0]));
            let Ok(converted) = to_dxf(&drawing, &mut cx) else {
                panic!("書けない");
            };
            r2004_structure(&converted.file, &mut cx);
            let Ok(read) = lift(&converted.file, &mut cx) else {
                panic!("読めない");
            };
            (read, view)
        };
        let codes: Vec<_> = diag.iter().map(|d| d.code()).collect();
        assert!(
            !codes.contains(&codes::CONVERT_VIEW_APPROXIMATED),
            "{codes:?}"
        );
        let [found] = read.sheets[0].views.as_slice() else {
            panic!("{:?}", read.sheets[0].views);
        };
        for (a, b) in found.transform.m.iter().zip(view.transform.m) {
            assert!(
                (a - b).abs() < 1e-9,
                "{:?} {:?}",
                found.transform,
                view.transform
            );
        }
        // 平面図の空間の線形は、モデル空間に書く
        let space = read
            .spaces
            .iter()
            .find(|space| space.id == found.space)
            .unwrap();
        assert!(
            space
                .elements
                .iter()
                .any(|e| matches!(e.shape, Shape::Line { .. } | Shape::Polyline(_)))
        );
    }

    /// 外部参照の記号の定義は書かず、定義と、それを置いた記号を、それぞれ元の順に報告する
    #[test]
    fn external_symbols_are_reported_and_not_written() {
        let key = |name: &str| SymbolKey {
            namespace: "dxf.block".into(),
            name: name.to_owned(),
        };
        let space = |id: u32, elements| ModelSpace {
            id: SpaceId(id),
            unit: Unit::Millimeter,
            axes: Axes::EastNorth,
            elements,
            order: vec![],
        };
        let definition = |id: u32, name: &str, external: bool| SymbolDefinition {
            key: key(name),
            space: space(id, vec![]),
            base: None,
            attributes: vec![],
            anonymous: false,
            external: external.then(|| "other.dwg".to_owned()),
            source: Ref::at_record(u64::from(id)),
        };
        let symbol = |name: &str, record: u64| Element {
            source: Ref::at_record(record),
            layer: LayerId(0),
            style: zumen_drawing::Style {
                color: Inherit::Layer,
                line_type: Inherit::Layer,
                line_type_scale: 1.0,
                line_weight: None,
                transparency: Inherit::Layer,
            },
            shape: Shape::Symbol(Symbol {
                key: key(name),
                position: ModelPoint {
                    space: SpaceId(0),
                    x: 0.0,
                    y: 0.0,
                },
                scale: [1.0, 1.0],
                rotation: 0.0,
                grid: None,
                attributes: vec![],
                clip: None,
            }),
        };
        let drawing = Drawing2D {
            spaces: vec![space(0, vec![symbol("外部B", 10), symbol("外部A", 11)])],
            symbols: vec![
                definition(1, "部品", false),
                definition(2, "外部A", true),
                definition(3, "外部B", true),
            ],
            ..Drawing2D::default()
        };
        let mut budget = Budget::new(Limits::default());
        let mut diag = Diagnostics::new();
        let cancel = CancelToken::new();
        let read = {
            let mut cx = Cx::new(&mut budget, &mut diag, &cancel);
            let Ok(converted) = to_dxf(&drawing, &mut cx) else {
                panic!("書けない");
            };
            r2004_structure(&converted.file, &mut cx);
            let Ok(read) = lift(&converted.file, &mut cx) else {
                panic!("読めない");
            };
            read
        };
        let dropped: Vec<_> = diag
            .iter()
            .filter(|d| d.code() == codes::CONVERT_ELEMENT_UNSUPPORTED)
            .map(|d| {
                (
                    d.param("kind").cloned(),
                    d.count(),
                    d.places().iter().map(Ref::record).collect::<Vec<_>>(),
                )
            })
            .collect();
        assert_eq!(
            dropped,
            [
                (
                    Some(Value::Key("external_symbol_definition")),
                    2,
                    vec![Some(2), Some(3)]
                ),
                (
                    Some(Value::Key("external_symbol")),
                    2,
                    vec![Some(10), Some(11)]
                ),
            ]
        );
        assert!(read.spaces.iter().all(|space| space.elements.is_empty()));
    }
}
