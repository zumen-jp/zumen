//! 図面モデルを、別の形式の新しいファイルに書く（lower。決定 0139・0206）。
//!
//! 形式ごとのひな形（表の既定の項目と、記号の定義の空の部品だけを持つファイル）を作り、図面モデルの線種・
//! 文字スタイル・寸法スタイル・レイヤ・要素を、書き戻し（apply）の追加で足す。書けない要素は落とし、種類ごとの
//! 件数を診断にする。

use std::collections::BTreeMap;

use zumen_acad::{Dxf, DxfFile};
use zumen_base::{Cx, Diagnostic, Diagnostics, Kind, Value, codes};
use zumen_drawing::{
    Changes, Color, ColorCode, ColorSystem, DimensionStyle, Drawing2D, Edit, Element, Hash,
    Inherit, Layer, LayerId, LayerState, LayerStyle, LineType, LineTypeId, ModelPoint, PaperPoint,
    Placed, Seen, Shape, SpaceId, SymbolKey, TextStyle, Unit, id,
};
use zumen_sdk::{Apply, Codec, Error, Lift};

use crate::geometry;
use crate::messages::Geometry;
use crate::session::{Failed, fail, stopped};

/// 書いたファイルと、足した要素の数
pub(crate) struct Converted {
    pub(crate) file: DxfFile,
    pub(crate) bytes: Vec<u8>,
    pub(crate) elements: u64,
}

/// 図面モデルを、新しい DXF（R2004）に書く
pub(crate) fn to_dxf(drawing: &Drawing2D, cx: &mut Cx<'_>) -> Result<Converted, Failed> {
    let mut lost = Lost::default();
    let blocks = block_names(drawing, &mut lost);
    let (template, placeholder) = template(drawing, &blocks);
    cx.check(Kind::Work, template.len() as u64)
        .map_err(|stop| stopped(cx, stop))?;
    let mut file = quiet(cx, |cx| Dxf::read(&template, cx)).map_err(|error| failed(cx, error))?;

    // 1. 線種・文字スタイル・寸法スタイル。レイヤは足した線種を同じ一覧で指せないので、先に足す
    let target = lift(&file, cx)?;
    let mut edits = Vec::new();
    for line_type in &drawing.line_types {
        cx.check(Kind::Work, 1).map_err(|stop| stopped(cx, stop))?;
        if line_type.definition.is_some() && find_line_type(&target, &line_type.name).is_none() {
            edits.push((
                Edit::AddLineType {
                    line_type: Box::new(line_type.clone()),
                },
                "line_type",
            ));
        }
    }
    for style in &drawing.text_styles {
        cx.check(Kind::Work, 1).map_err(|stop| stopped(cx, stop))?;
        if style.definition.is_some() && target.text_style(&style.name).is_none() {
            edits.push((
                Edit::AddTextStyle {
                    style: Box::new(TextStyle {
                        name: style.name.clone(),
                        definition: style.definition.clone(),
                        source: zumen_base::Ref::default(),
                    }),
                },
                "text_style",
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
            edits.push((
                Edit::AddDimensionStyle {
                    style: Box::new(DimensionStyle {
                        source: zumen_base::Ref::default(),
                        ..style.clone()
                    }),
                },
                "dimension_style",
            ));
        }
    }
    add_all(&mut file, edits, &mut lost, cx)?;

    // 2. レイヤ。線種は、書いた先の ID に直す
    let target = lift(&file, cx)?;
    let mut edits = Vec::new();
    for layer in &drawing.layers {
        cx.check(Kind::Work, 1).map_err(|stop| stopped(cx, stop))?;
        if find_layer(&target, &layer.name).is_some() {
            continue;
        }
        // DXF の画層は、状態・番号の色・線種を持つ。持たないものは既定で埋め、透過性と説明は書かない
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
        edits.push((
            Edit::AddLayer {
                layer: Box::new(Layer {
                    state: Some(state),
                    style: Some(LayerStyle {
                        color: Some(color),
                        line_type,
                        transparency: None,
                        ..style
                    }),
                    description: None,
                    source: zumen_base::Ref::default(),
                    ..layer.clone()
                }),
            },
            "layer",
        ));
    }
    add_all(&mut file, edits, &mut lost, cx)?;

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
        blocks: &blocks,
    };
    let mut edits = Vec::new();
    for definition in &drawing.symbols {
        let Some(name) = blocks.get(&key_of(&definition.key)) else {
            continue;
        };
        let Some(place) = places.get(&name.to_lowercase()) else {
            continue;
        };
        for element in &definition.space.elements {
            cx.check(Kind::Work, 1).map_err(|stop| stopped(cx, stop))?;
            maps.element(element, place, &mut edits, &mut lost);
        }
        for attribute in &definition.attributes {
            cx.check(Kind::Work, 1).map_err(|stop| stopped(cx, stop))?;
            if attribute.constant && !attribute.attribute.hidden {
                let text = Element {
                    source: zumen_base::Ref::default(),
                    layer: attribute.attribute.layer,
                    style: attribute.attribute.style,
                    shape: Shape::Text(attribute.attribute.text.clone()),
                };
                maps.element(&text, place, &mut edits, &mut lost);
            } else {
                lost.add("attribute_definition");
            }
        }
    }
    for space in &drawing.spaces {
        for element in &space.elements {
            cx.check(Kind::Work, 1).map_err(|stop| stopped(cx, stop))?;
            maps.element(element, &model, &mut edits, &mut lost);
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
        match &paper {
            Some(paper) if index == 0 => {
                for element in &sheet.elements {
                    cx.check(Kind::Work, 1).map_err(|stop| stopped(cx, stop))?;
                    maps.element(element, paper, &mut edits, &mut lost);
                }
            }
            _ => lost.add("sheet"),
        }
        if !sheet.views.is_empty() {
            lost.add_many("view", sheet.views.len() as u64);
        }
    }
    lost.added = 0;
    add_all(&mut file, edits, &mut lost, cx)?;
    let elements = lost.added;

    // 4. 用紙を作るために置いた点を除く
    if let Some(handle) = placeholder {
        let target = lift(&file, cx)?;
        let found = target.sheets.first().and_then(|sheet| {
            let place = id::sheet(sheet);
            let mut namer = id::Namer::new(&place);
            sheet
                .elements
                .iter()
                .enumerate()
                .find_map(|(index, element)| {
                    let id = namer.next(index, element);
                    (element.source.key() == Some(zumen_base::Key::Handle(handle))).then(|| Seen {
                        id,
                        hash: Hash::element(element),
                    })
                })
        });
        if let Some(target) = found {
            let changes = Changes {
                edits: vec![Edit::Remove { target }],
                document: None,
            };
            quiet(cx, |cx| file.apply(&changes, cx)).map_err(|error| failed(cx, error))?;
        }
    }

    let mut bytes = Vec::new();
    Dxf::write(&file, &mut bytes, cx).map_err(|_| Failed)?;
    lost.report("DXF", cx);
    Ok(Converted {
        file,
        bytes,
        elements,
    })
}

/// 書けなかったものの、種類ごとの件数
#[derive(Default)]
struct Lost {
    kinds: BTreeMap<&'static str, u64>,
    /// 足した要素の数
    added: u64,
    /// 記号から離した文字にした属性の数
    detached: u64,
}

impl Lost {
    fn add(&mut self, kind: &'static str) {
        self.add_many(kind, 1);
    }

    fn add_many(&mut self, kind: &'static str, count: u64) {
        *self.kinds.entry(kind).or_insert(0) += count;
    }

    fn report(self, format: &str, cx: &mut Cx<'_>) {
        // 同じ値の診断は 1 つにまとまり、件数（count）を数える
        for _ in 0..self.detached {
            cx.report(Diagnostic::new(codes::CONVERT_ATTRIBUTE_DRAWN).with("format", format));
        }
        for (kind, count) in self.kinds {
            for _ in 0..count {
                cx.report(
                    Diagnostic::new(codes::CONVERT_ELEMENT_UNSUPPORTED)
                        .with("kind", Value::Key(kind))
                        .with("format", format),
                );
            }
        }
    }
}

/// 書いた先の表と記号の定義の名前への対応
struct Maps<'a> {
    source: &'a Drawing2D,
    target: &'a Drawing2D,
    blocks: &'a BTreeMap<(&'static str, String), String>,
}

impl Maps<'_> {
    /// 要素を書いた先の表と空間に合わせて直し、足す変更にする。形は境界の形の値（`add` が受けるもの）を
    /// 通して作り直す。記号の属性は、記号から離した文字にする
    fn element<P: geometry::Spot + Clone>(
        &self,
        element: &Element<P>,
        place: &Place<P>,
        edits: &mut Vec<(Edit, &'static str)>,
        lost: &mut Lost,
    ) {
        let kind = kind_of(&element.shape);
        let Some(mut geometry) = geometry::geometry_of(&element.shape) else {
            lost.add(kind);
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
                    lost.add(kind);
                    return;
                };
                let Some(name) = self.blocks.get(&key_of(&source.key)) else {
                    lost.add("external_symbol");
                    return;
                };
                symbol.name = name.clone();
                symbol.attributes = None;
                for attribute in &source.attributes {
                    if !attribute.hidden {
                        detached.push(Element {
                            source: zumen_base::Ref::default(),
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
            lost.add(kind);
            return;
        };
        let element_of = |shape| Element {
            source: zumen_base::Ref::default(),
            layer: self.layer(element.layer),
            style: self.style(&element.style),
            shape,
        };
        edits.push((
            Edit::Add {
                place: place.id.clone(),
                element: Box::new((place.wrap)(element_of(shape))),
            },
            kind,
        ));
        for text in detached {
            let before = edits.len();
            self.element(&text, place, edits, lost);
            if edits.len() > before {
                lost.detached += 1;
            }
        }
    }

    fn layer(&self, layer: LayerId) -> LayerId {
        let name = self.source.layer(layer).map_or("0", |layer| &layer.name);
        find_layer(self.target, name)
            .or_else(|| find_layer(self.target, "0"))
            .map_or(layer, |layer| layer.id)
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

/// 変更をまとめて当て、当てられなければ半分に分けて当て直す。1 つでも当てられない変更は落として数える。
/// 当てられなかった試しの診断は捨てる
fn add_all(
    file: &mut DxfFile,
    edits: Vec<(Edit, &'static str)>,
    lost: &mut Lost,
    cx: &mut Cx<'_>,
) -> Result<(), Failed> {
    let mut pending = vec![edits];
    while let Some(edits) = pending.pop() {
        if edits.is_empty() {
            continue;
        }
        cx.check(Kind::Work, edits.len() as u64)
            .map_err(|stop| stopped(cx, stop))?;
        let changes = Changes {
            edits: edits.iter().map(|(edit, _)| edit.clone()).collect(),
            document: None,
        };
        match quiet(cx, |cx| file.apply(&changes, cx)) {
            Ok(_) => {
                lost.added += edits
                    .iter()
                    .filter(|(edit, _)| matches!(edit, Edit::Add { .. }))
                    .count() as u64;
            }
            Err(Error::Rejected | Error::Invalid | Error::Unsupported) => {
                if let [(_, kind)] = edits.as_slice() {
                    lost.add(kind);
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
    Ok(())
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

fn key_of(key: &SymbolKey) -> (&'static str, String) {
    (key.namespace, key.name.clone())
}

/// 記号の定義ごとの、書いた先の部品の名前。外部参照の定義は書かない。無名の定義には名前を付ける
fn block_names(drawing: &Drawing2D, lost: &mut Lost) -> BTreeMap<(&'static str, String), String> {
    let mut names = BTreeMap::new();
    let mut used: BTreeMap<String, ()> = BTreeMap::new();
    for reserved in ["*model_space", "*paper_space"] {
        used.insert(reserved.to_string(), ());
    }
    for (index, definition) in drawing.symbols.iter().enumerate() {
        if definition.external.is_some() {
            lost.add("external_symbol_definition");
            continue;
        }
        let base = if definition.anonymous || definition.key.name.starts_with('*') {
            format!("zumen_{index}")
        } else {
            definition
                .key
                .name
                .chars()
                .map(|c| {
                    if "<>/\\\":;?*|=,`".contains(c) || c.is_control() {
                        '_'
                    } else {
                        c
                    }
                })
                .collect()
        };
        let base = if base.is_empty() {
            format!("zumen_{index}")
        } else {
            base
        };
        let mut name = base.clone();
        let mut repeat = 1;
        while used.contains_key(&name.to_lowercase()) {
            name = format!("{base}_{repeat}");
            repeat += 1;
        }
        used.insert(name.to_lowercase(), ());
        names.insert(key_of(&definition.key), name);
    }
    names
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

/// ひな形の DXF（R2004）。表の既定の項目と、記号の定義の空の部品を持つ。用紙に書く要素があれば、
/// 用紙を作るための点をペーパー空間に置き、そのハンドルを返す
fn template(
    drawing: &Drawing2D,
    blocks: &BTreeMap<(&'static str, String), String>,
) -> (Vec<u8>, Option<u64>) {
    let mut handle = 0x10_u64;
    let mut next = || {
        handle += 1;
        format!("{handle:X}")
    };
    let mut tables = String::new();
    tables.push_str("0\nSECTION\n2\nTABLES\n0\nTABLE\n2\nLTYPE\n");
    for name in ["ByBlock", "ByLayer", "Continuous"] {
        let description = if name == "Continuous" {
            "Solid line"
        } else {
            ""
        };
        tables.push_str(&format!(
            "0\nLTYPE\n5\n{}\n2\n{name}\n70\n0\n3\n{description}\n72\n65\n73\n0\n40\n0.0\n",
            next()
        ));
    }
    tables.push_str("0\nENDTAB\n0\nTABLE\n2\nLAYER\n");
    tables.push_str(&format!(
        "0\nLAYER\n5\n{}\n2\n0\n70\n0\n62\n7\n6\nContinuous\n",
        next()
    ));
    tables.push_str("0\nENDTAB\n0\nTABLE\n2\nSTYLE\n");
    tables.push_str(&format!(
        "0\nSTYLE\n5\n{}\n2\nStandard\n70\n0\n40\n0.0\n41\n1.0\n50\n0.0\n71\n0\n42\n2.5\n3\ntxt\n4\n\n",
        next()
    ));
    tables.push_str("0\nENDTAB\n0\nTABLE\n2\nDIMSTYLE\n");
    tables.push_str(&format!(
        "0\nDIMSTYLE\n105\n{}\n2\nStandard\n70\n0\n",
        next()
    ));
    tables.push_str("0\nENDTAB\n0\nTABLE\n2\nBLOCK_RECORD\n");
    let mut parts = vec![
        ("*Model_Space".to_string(), [0.0, 0.0]),
        ("*Paper_Space".to_string(), [0.0, 0.0]),
    ];
    for definition in &drawing.symbols {
        if let Some(name) = blocks.get(&key_of(&definition.key)) {
            let base = definition.base.map_or([0.0, 0.0], |base| [base.x, base.y]);
            parts.push((name.clone(), base));
        }
    }
    for (name, _) in &parts {
        tables.push_str(&format!("0\nBLOCK_RECORD\n5\n{}\n2\n{name}\n", next()));
    }
    tables.push_str("0\nENDTAB\n0\nENDSEC\n");
    let mut section = String::from("0\nSECTION\n2\nBLOCKS\n");
    for (name, [x, y]) in &parts {
        section.push_str(&format!(
            "0\nBLOCK\n5\n{}\n8\n0\n2\n{name}\n70\n0\n10\n{x:?}\n20\n{y:?}\n30\n0.0\n3\n{name}\n1\n\n0\nENDBLK\n5\n{}\n8\n0\n",
            next(),
            next()
        ));
    }
    section.push_str("0\nENDSEC\n");
    let unit = match drawing.spaces.first().map(|space| space.unit) {
        Some(Unit::Millimeter) => 4,
        Some(Unit::Centimeter) => 5,
        Some(Unit::Meter) => 6,
        Some(Unit::Inch) => 1,
        Some(Unit::Foot) => 2,
        // 実寸の空間のない図面（SXF）は、用紙の単位の mm にする
        None => 4,
        _ => 0,
    };
    let placeholder = drawing
        .sheets
        .first()
        .is_some_and(|sheet| !sheet.elements.is_empty())
        .then(&mut next);
    let seed = next();
    // 用紙の大きさ（mm）。DXF の用紙の単位を mm とする
    let limits = drawing.sheets.first().and_then(|sheet| sheet.size).map_or(
        String::new(),
        |[width, height]| {
            format!(
                "9\n$PLIMMIN\n10\n0.0\n20\n0.0\n9\n$PLIMMAX\n10\n{:?}\n20\n{:?}\n",
                width.0, height.0
            )
        },
    );
    let mut out = format!(
        "0\nSECTION\n2\nHEADER\n9\n$ACADVER\n1\nAC1018\n9\n$INSUNITS\n70\n{unit}\n{limits}9\n$HANDSEED\n5\n{seed}\n0\nENDSEC\n"
    );
    out.push_str(&tables);
    out.push_str(&section);
    out.push_str("0\nSECTION\n2\nENTITIES\n");
    if let Some(handle) = &placeholder {
        out.push_str(&format!(
            "0\nPOINT\n5\n{handle}\n8\n0\n67\n1\n10\n0.0\n20\n0.0\n30\n0.0\n"
        ));
    }
    out.push_str("0\nENDSEC\n0\nEOF\n");
    let placeholder = placeholder.and_then(|handle| u64::from_str_radix(&handle, 16).ok());
    (out.into_bytes(), placeholder)
}
