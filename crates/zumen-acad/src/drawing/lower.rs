//! 図面モデル（L2）を、新しい DXF（R2004）に書く（決定 0008・0139・0206・0281）。
//!
//! 形式ごとのひな形（[`NewDxfTemplate`]。表の既定の項目、記号の定義の空の部品、レイアウトと用紙のビューポートを持つ
//! ファイル）を作り、図面モデルの線種・文字スタイル・寸法スタイル・レイヤ・要素を、書き戻し（apply）の追加で足す。
//! 書けない要素は落とし、種類ごとの件数を診断にする。要素の形を書く先の空間か用紙に移す方法は、[`Relocate`] で
//! 呼び出し元が決める。既定は [`Reseat`]（点と長さの空間を付け替える）。

use std::collections::{BTreeMap, BTreeSet};

use zumen_base::{Cx, Diagnostic, Diagnostics, Kind, Ref, Stop, Value, codes};
use zumen_drawing::transform::Point as _;
use zumen_drawing::{
    Attribute, Branch, Callout, CalloutContent, Changes, Color, ColorCode, ColorSystem, Dimension,
    DimensionKind, DimensionStyle, Dogleg, Drawing2D, Edge, Edit, Element, FormattedText, Grid,
    HeightMeaning, Inherit, Layer, LayerId, LayerState, LayerStyle, Leader, Length, LineType,
    LineTypeId, ModelLength, ModelPoint, PaperPoint, Placed, Placement, Polyline, Shape, SheetId,
    SheetItem, SpaceId, Spline, Symbol, Text, TextStyle, Vertex, id,
};
use zumen_sdk::{Apply, Codec, Error, Lift};

use super::bake;
use super::template::NewDxfTemplate;
use crate::conversion_report::Report;
use crate::{ConversionFidelity, ConversionReport};
use crate::{Dxf, DxfFile};

/// 図面モデルから書いたファイルと、そのバイト列、足した要素の数
#[derive(Clone, Debug)]
pub struct Lowered {
    /// 書いたファイル
    pub file: DxfFile,
    /// 書いたバイト列
    pub bytes: Vec<u8>,
    /// 足した要素の数
    pub elements: u64,
    /// 書き手が採用・棄却した項目の集計と、指定した上限までの明細
    pub report: ConversionReport,
}

/// 要素の形を、書く先の空間か用紙に移す方法。移せない形は `None` で、その要素は落として診断にする
///
/// `origin` は書く先の原点（座標は 0）で、空間か用紙の ID を持つ。形の点と長さを、その空間か用紙のものにして返す。
pub trait Relocate {
    /// 実寸の記号を、部品の参照を引く前に受け入れるか。`false` なら `symbol` として落とす。
    /// 既定はすべて受け入れ、形を移せるかは [`Self::model`] で確かめる
    fn accepts_model_symbol(&self, _symbol: &Symbol<ModelPoint>) -> bool {
        true
    }
    /// 用紙の記号を、部品の参照を引く前に受け入れるか。既定と診断は [`Self::accepts_model_symbol`] と同じ
    fn accepts_paper_symbol(&self, _symbol: &Symbol<PaperPoint>) -> bool {
        true
    }
    /// 実寸の空間（モデル空間か記号の定義）に書く形
    fn model(&self, shape: &Shape<ModelPoint>, origin: ModelPoint) -> Option<Shape<ModelPoint>>;
    /// 用紙に書く形
    fn paper(&self, shape: &Shape<PaperPoint>, origin: PaperPoint) -> Option<Shape<PaperPoint>>;
    /// 実寸の形の移し方が保つ忠実度。独自の移し方は、既定では見た目だけとする
    fn fidelity_model(
        &self,
        _source: &Shape<ModelPoint>,
        _output: &Shape<ModelPoint>,
    ) -> ConversionFidelity {
        ConversionFidelity::Drawn
    }
    /// 用紙の形の移し方が保つ忠実度。既定の扱いは [`Self::fidelity_model`] と同じ
    fn fidelity_paper(
        &self,
        _source: &Shape<PaperPoint>,
        _output: &Shape<PaperPoint>,
    ) -> ConversionFidelity {
        ConversionFidelity::Drawn
    }
}

/// 点と実寸の長さの空間（用紙の点なら用紙）を、書く先のものに付け替える。座標と値は変えない。表・下絵・ハッチングは、
/// 書き戻しで足せないので移さない。文字（TEXT と MTEXT）の高さは、値を変えずに DXF の高さ（40）として書く。API の
/// 形の値から足す文字と同じである
#[derive(Clone, Copy, Debug, Default)]
pub struct Reseat;

impl Relocate for Reseat {
    fn fidelity_model(
        &self,
        _source: &Shape<ModelPoint>,
        _output: &Shape<ModelPoint>,
    ) -> ConversionFidelity {
        ConversionFidelity::Equivalent
    }

    fn fidelity_paper(
        &self,
        _source: &Shape<PaperPoint>,
        _output: &Shape<PaperPoint>,
    ) -> ConversionFidelity {
        ConversionFidelity::Equivalent
    }
    fn model(&self, shape: &Shape<ModelPoint>, origin: ModelPoint) -> Option<Shape<ModelPoint>> {
        reseat(
            shape,
            &|point: ModelPoint| origin.at(point.xy()),
            &|length| match length {
                Length::Model(model) => Length::Model(ModelLength {
                    space: origin.space,
                    ..model
                }),
                paper => paper,
            },
        )
        .map(declared)
    }

    fn paper(&self, shape: &Shape<PaperPoint>, origin: PaperPoint) -> Option<Shape<PaperPoint>> {
        reseat(
            shape,
            &|point: PaperPoint| origin.at(point.xy()),
            &|length| length,
        )
        .map(declared)
    }
}

/// 文字の高さの意味を、DXF の高さ（TEXT・MTEXT の 40）にする。`apply` はこの意味の文字だけを足す
fn declared<P>(shape: Shape<P>) -> Shape<P> {
    match shape {
        Shape::Text(mut text) => {
            text.height.meaning = HeightMeaning::Declared("dxf.TEXT.40".into());
            Shape::Text(text)
        }
        Shape::Formatted(mut text) => {
            text.height.meaning = HeightMeaning::Declared("dxf.MTEXT.40".into());
            Shape::Formatted(text)
        }
        shape => shape,
    }
}

/// 図面モデルを、新しい DXF（R2004）に書く。形は [`Reseat`] で移す
///
/// # Errors
///
/// 予算の上限か中断の合図で止めたときは [`Error::Stopped`]。ひな形を読めないなどの内部の失敗は、
/// `internal.panic.caught` を報告して [`Error::Invalid`]。この `Invalid` は入力の誤りではなく内部の不具合で、
/// 診断の code で見分ける。どちらも診断は報告済み
pub fn lower_to_dxf(drawing: &Drawing2D, cx: &mut Cx<'_>) -> Result<Lowered, Error> {
    lower_to_dxf_with(drawing, &Reseat, cx)
}

/// 図面モデルを、新しい DXF（R2004）に書く。形は `relocate` で移す
///
/// # Errors
///
/// [`lower_to_dxf`] と同じ
pub fn lower_to_dxf_with(
    drawing: &Drawing2D,
    relocate: &dyn Relocate,
    cx: &mut Cx<'_>,
) -> Result<Lowered, Error> {
    lower_to_dxf_with_report(drawing, relocate, 0, cx)
}

/// 図面モデルを DXF に書き、全件の集計と `limit` 件までの明細を返す
///
/// 明細の上限は出力と診断に影響しない。同じ出所を持つ別の項目も、それぞれ数える。
///
/// # Errors
///
/// [`lower_to_dxf`] と同じ
pub fn lower_to_dxf_with_report(
    drawing: &Drawing2D,
    relocate: &dyn Relocate,
    limit: u64,
    cx: &mut Cx<'_>,
) -> Result<Lowered, Error> {
    let mut record = Report::new(limit);
    let report = &mut record;
    let template = NewDxfTemplate::new(drawing);
    // ひな形は、キーごとに先の定義だけを書き、それが外部参照なら書かない（`NewDxfTemplate::new`、#656）。書かない
    // 定義をここで報告する。外部参照の定義を置いた記号は、下の要素で報告する
    for (index, definition) in drawing.symbols.iter().enumerate() {
        if template.duplicate(index) {
            report_dropped(
                cx,
                report,
                "duplicate_symbol_definition",
                &definition.source,
            )?;
        } else if definition.external.is_some() {
            report_dropped(cx, report, "external_symbol_definition", &definition.source)?;
        } else {
            report
                .record(
                    "symbol_definition",
                    ConversionFidelity::Equivalent,
                    &definition.source,
                    0,
                    cx,
                )
                .map_err(|error| failed(cx, error))?;
        }
    }
    // ひな形を符号化できなかったとき（不具合）は空。空のバイト列も読めてしまうので、ここで内部の失敗にする
    if template.bytes().is_empty() {
        return Err(internal(cx));
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
        } else {
            report
                .record_with_reasons(
                    "line_type",
                    ConversionFidelity::Drawn,
                    &line_type.source,
                    0,
                    &["template_definition"],
                    cx,
                )
                .map_err(|error| failed(cx, error))?;
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
                        source: Ref::default(),
                    }),
                },
                "text_style",
                &style.source,
            ));
        } else {
            report
                .record_with_reasons(
                    "text_style",
                    ConversionFidelity::Drawn,
                    &style.source,
                    0,
                    &["template_definition"],
                    cx,
                )
                .map_err(|error| failed(cx, error))?;
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
                        source: Ref::default(),
                        ..style.clone()
                    }),
                },
                "dimension_style",
                &style.source,
            ));
        } else {
            report
                .record_with_reasons(
                    "dimension_style",
                    ConversionFidelity::Drawn,
                    &style.source,
                    0,
                    &["template_definition"],
                    cx,
                )
                .map_err(|error| failed(cx, error))?;
        }
    }
    add_all(&mut file, edits, None, report, cx)?;

    // 2. レイヤ。線種は、書いた先の ID に直す
    let target = lift(&file, cx)?;
    let mut edits = Vec::new();
    let mut fields = Vec::new();
    let mut reused = BTreeSet::new();
    for layer in &drawing.layers {
        cx.check(Kind::Work, 1).map_err(|stop| stopped(cx, stop))?;
        if let Some(existing) = find_layer(&target, &layer.name) {
            report
                .record_with_reasons(
                    "layer",
                    ConversionFidelity::Drawn,
                    &layer.source,
                    0,
                    &["template_definition"],
                    cx,
                )
                .map_err(|error| failed(cx, error))?;
            // 同じひな形の画層を指す入力が複数あれば先の画層を使う。
            if !reused.insert(existing.id) {
                report_layer_fields(layer, report, cx)?;
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
                    source: Ref::default(),
                }),
            },
            "layer",
            &layer.source,
        );
        if style
            .line_type
            .is_some_and(|id| map_line_type(drawing, &target, id).is_none())
        {
            addition.reasons.push("line_type_default");
        }
        if style.color.is_some_and(|original| original != color) {
            addition.reasons.push("color_code");
        }
        if layer.state.is_some_and(|original| original != state) {
            addition.reasons.push("layer_state");
        }
        if !addition.reasons.is_empty() {
            addition.fidelity = ConversionFidelity::Drawn;
        }
        addition.layer_fields = has_layer_fields(layer).then_some(layer);
        edits.push(addition);
    }
    fields.extend(add_all(&mut file, edits, None, report, cx)?.layer_fields);

    // 3. 要素。記号の定義の中身を先に、実寸の空間の要素を後に足す
    let target = lift(&file, cx)?;
    let places = definition_places(&target);
    let Some(model) = target
        .spaces
        .first()
        .map(|space| model_place(id::space(space.id), space.id))
    else {
        return Err(internal(cx));
    };
    let maps = Maps {
        source: drawing,
        target: &target,
        blocks: &template,
        relocate,
    };
    let mut edits = Vec::new();
    for (index, definition) in drawing.symbols.iter().enumerate() {
        if template.duplicate(index) {
            continue;
        }
        let Some(name) = template.block(&definition.key) else {
            continue;
        };
        let Some(place) = places.get(&name.to_lowercase()) else {
            continue;
        };
        for element in &definition.space.elements {
            cx.check(Kind::Work, 1).map_err(|stop| stopped(cx, stop))?;
            maps.element(element, place, &mut edits, report, cx)?;
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
                maps.element(&text, place, &mut edits, report, cx)?;
            } else {
                report_dropped(
                    cx,
                    report,
                    "attribute_definition",
                    &attribute.attribute.source,
                )?;
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
            maps.element(element, &model, &mut edits, report, cx)?;
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
            report_dropped(cx, report, "sheet", &sheet.source)?;
            for view in &sheet.views {
                report_dropped(cx, report, "view", &view.source)?;
            }
            for element in &sheet.elements {
                report
                    .record_with_reasons(
                        kind_of(&element.shape),
                        ConversionFidelity::Dropped,
                        &element.source,
                        0,
                        &["sheet"],
                        cx,
                    )
                    .map_err(|error| failed(cx, error))?;
            }
            continue;
        };
        report
            .record_with_reasons(
                "sheet",
                ConversionFidelity::Drawn,
                &sheet.source,
                0,
                &["template_sheet"],
                cx,
            )
            .map_err(|error| failed(cx, error))?;
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
                        maps.element(element, paper, &mut edits, report, cx)?;
                    }
                }
                SheetItem::View(index) => {
                    let Some(view) = sheet.views.get(index) else {
                        continue;
                    };
                    if NewDxfTemplate::writes_viewport(view, sheet) {
                        report
                            .record("view", ConversionFidelity::Equivalent, &view.source, 0, cx)
                            .map_err(|error| failed(cx, error))?;
                        continue;
                    }
                    let space = drawing.spaces.iter().find(|space| space.id == view.space);
                    let Some(space) = space.filter(|_| bake::bakes(view)) else {
                        report_dropped(cx, report, "view", &view.source)?;
                        continue;
                    };
                    cx.report(
                        Diagnostic::new(codes::CONVERT_VIEW_APPROXIMATED)
                            .with("format", "DXF")
                            .at(view.source.clone()),
                    );
                    report
                        .record_with_reasons(
                            "view",
                            ConversionFidelity::Approximated,
                            &view.source,
                            0,
                            &["baked_view"],
                            cx,
                        )
                        .map_err(|error| failed(cx, error))?;
                    for element in bake::elements(space, view) {
                        // 曲線を折れ線にする手間と作る点を、作る前に数えられるものは前に、ほかは作ったあとに
                        // 数える（#753・#756）
                        let (work, points) = bake::before(&element.shape, view);
                        cx.check(Kind::Work, work.saturating_add(1))
                            .map_err(|stop| stopped(cx, stop))?;
                        cx.check(Kind::Segments, points)
                            .map_err(|stop| stopped(cx, stop))?;
                        let baked = bake::element_of(element, view);
                        cx.check(Kind::Work, baked.work)
                            .map_err(|stop| stopped(cx, stop))?;
                        cx.check(Kind::Segments, baked.points)
                            .map_err(|stop| stopped(cx, stop))?;
                        match baked.element {
                            Some(baked) => {
                                let before = edits.len();
                                maps.element(&baked, paper, &mut edits, report, cx)?;
                                for addition in edits.iter_mut().skip(before) {
                                    addition.fidelity =
                                        addition.fidelity.max(ConversionFidelity::Drawn);
                                    addition.reasons.push("baked_view");
                                }
                            }
                            None => report_dropped(
                                cx,
                                report,
                                kind_of(&element.shape),
                                &element.source,
                            )?,
                        }
                    }
                }
            }
        }
    }
    let paper = paper.map(|place| (place.id, place.origin.sheet));
    let elements = add_all(&mut file, edits, paper, report, cx)?.elements;
    let db = quiet(cx, |cx| file.objects(cx)).map_err(|error| failed(cx, error))?;
    let db = db
        .finish_new_dxf_tables_with_report(&fields, report, cx)
        .map_err(|error| failed(cx, error))?;
    quiet(cx, |cx| file.write_back(&db, cx)).map_err(|error| failed(cx, error))?;
    let mut bytes = Vec::new();
    // 止めたときは、書き出しが報告する
    Dxf::write(&file, &mut bytes, cx)?;
    // lower は形式固有の拡張や表題欄の意味、文書の既定値を書かない。図形の成功と別の項目にして、
    // 図形が書けたことを、これらの値も保てたという結果にしない。
    for extension in &drawing.extensions {
        report
            .record_with_reasons(
                "extension",
                ConversionFidelity::Dropped,
                &extension.source,
                0,
                &["format_extension"],
                cx,
            )
            .map_err(|error| failed(cx, error))?;
    }
    for title in &drawing.title_blocks {
        report
            .record_with_reasons(
                "title_block",
                ConversionFidelity::Dropped,
                &title.source,
                0,
                &["semantic_fields"],
                cx,
            )
            .map_err(|error| failed(cx, error))?;
    }
    for (kind, present) in [
        ("line_type_scale", drawing.line_type_scale.is_some()),
        ("default_line_weight", drawing.default_line_weight.is_some()),
        ("default_color", drawing.default_color.is_some()),
        ("default_line_type", drawing.default_line_type.is_some()),
        ("point_style", drawing.point_style.is_some()),
    ] {
        if present {
            report
                .record_with_reasons(
                    kind,
                    ConversionFidelity::Dropped,
                    &Ref::default(),
                    0,
                    &["document_default"],
                    cx,
                )
                .map_err(|error| failed(cx, error))?;
        }
    }
    Ok(Lowered {
        file,
        bytes,
        elements,
        report: record.finish(cx).map_err(|error| failed(cx, error))?,
    })
}

fn report_dropped(
    cx: &mut Cx<'_>,
    report: &mut Report,
    kind: &'static str,
    source: &Ref,
) -> Result<(), Error> {
    report
        .record(kind, ConversionFidelity::Dropped, source, 0, cx)
        .map_err(|error| failed(cx, error))?;
    cx.report(
        Diagnostic::new(codes::CONVERT_ELEMENT_UNSUPPORTED)
            .with("kind", Value::Key(kind))
            .with("format", "DXF")
            .at(source.clone()),
    );
    Ok(())
}

fn has_layer_fields(layer: &Layer) -> bool {
    layer.description.is_some() || layer.style.and_then(|style| style.transparency).is_some()
}

fn report_layer_fields(layer: &Layer, report: &mut Report, cx: &mut Cx<'_>) -> Result<(), Error> {
    let transparency = layer.style.and_then(|style| style.transparency);
    for (field, present) in [
        ("description", layer.description.is_some()),
        ("transparency", transparency.is_some()),
    ] {
        if present {
            report
                .record(
                    &format!("layer_{field}"),
                    ConversionFidelity::Dropped,
                    &layer.source,
                    0,
                    cx,
                )
                .map_err(|error| failed(cx, error))?;
            cx.report(
                Diagnostic::new(codes::CONVERT_LAYER_FIELD_DROPPED)
                    .with("field", Value::Key(field))
                    .with("format", "DXF")
                    .at(layer.source.clone()),
            );
        }
    }
    Ok(())
}

/// 追加と、失敗したときに返す元の種類・場所。属性の描画は、追加が通ってから数える。
struct Addition<'a> {
    edit: Edit,
    kind: &'static str,
    source: Ref,
    detached: bool,
    layer_fields: Option<&'a Layer>,
    fidelity: ConversionFidelity,
    reasons: Vec<&'static str>,
}

impl Addition<'_> {
    fn new(edit: Edit, kind: &'static str, source: &Ref) -> Self {
        Self {
            edit,
            kind,
            source: source.clone(),
            detached: false,
            layer_fields: None,
            fidelity: ConversionFidelity::Equivalent,
            reasons: Vec::new(),
        }
    }
}

/// 書く先の空間か用紙の点。[`Relocate`] の対応する方法を選ぶ
trait Spot: zumen_drawing::transform::Point {
    fn accepts_symbol(relocate: &dyn Relocate, symbol: &Symbol<Self>) -> bool;
    fn relocate(relocate: &dyn Relocate, shape: &Shape<Self>, origin: Self) -> Option<Shape<Self>>;
    fn fidelity(
        relocate: &dyn Relocate,
        source: &Shape<Self>,
        output: &Shape<Self>,
    ) -> ConversionFidelity;
}

impl Spot for ModelPoint {
    fn fidelity(
        relocate: &dyn Relocate,
        source: &Shape<Self>,
        output: &Shape<Self>,
    ) -> ConversionFidelity {
        relocate.fidelity_model(source, output)
    }
    fn accepts_symbol(relocate: &dyn Relocate, symbol: &Symbol<Self>) -> bool {
        relocate.accepts_model_symbol(symbol)
    }

    fn relocate(relocate: &dyn Relocate, shape: &Shape<Self>, origin: Self) -> Option<Shape<Self>> {
        relocate.model(shape, origin)
    }
}

impl Spot for PaperPoint {
    fn fidelity(
        relocate: &dyn Relocate,
        source: &Shape<Self>,
        output: &Shape<Self>,
    ) -> ConversionFidelity {
        relocate.fidelity_paper(source, output)
    }
    fn accepts_symbol(relocate: &dyn Relocate, symbol: &Symbol<Self>) -> bool {
        relocate.accepts_paper_symbol(symbol)
    }

    fn relocate(relocate: &dyn Relocate, shape: &Shape<Self>, origin: Self) -> Option<Shape<Self>> {
        relocate.paper(shape, origin)
    }
}

/// 書いた先の表と記号の定義の名前への対応
struct Maps<'a> {
    source: &'a Drawing2D,
    target: &'a Drawing2D,
    blocks: &'a NewDxfTemplate,
    relocate: &'a dyn Relocate,
}

impl Maps<'_> {
    /// 要素を書いた先の表と空間に合わせて直し、足す変更にする。形は [`Relocate`] で書く先に移す。記号の属性は、
    /// 記号から離した文字にする
    fn element<P: Spot + Clone>(
        &self,
        element: &Element<P>,
        place: &Place<P>,
        edits: &mut Vec<Addition<'_>>,
        report: &mut Report,
        cx: &mut Cx<'_>,
    ) -> Result<(), Error> {
        let kind = kind_of(&element.shape);
        if let Shape::Symbol(symbol) = &element.shape
            && !P::accepts_symbol(self.relocate, symbol)
        {
            report_dropped(cx, report, kind, &element.source)?;
            return Ok(());
        }
        let mut shape = element.shape.clone();
        let mut detached = Vec::new();
        match &mut shape {
            Shape::Text(text) => self.text_style(&mut text.style),
            Shape::Formatted(text) => self.text_style(&mut text.style),
            Shape::Symbol(symbol) => {
                for attribute in symbol.attributes.drain(..) {
                    if !attribute.hidden {
                        detached.push(Element {
                            source: attribute.source,
                            layer: attribute.layer,
                            style: attribute.style,
                            shape: Shape::Text(attribute.text),
                        });
                    }
                }
            }
            _ => {}
        }
        // 受け入れた記号が書かない定義（外部参照など）を指せば、形を戻せるかによらず `external_symbol` で落とす
        let block = match &element.shape {
            Shape::Symbol(source) => {
                let Some(name) = self.blocks.block(&source.key) else {
                    report_dropped(cx, report, "external_symbol", &element.source)?;
                    return Ok(());
                };
                Some(name)
            }
            _ => None,
        };
        let Some(mut shape) = P::relocate(self.relocate, &shape, place.origin) else {
            report_dropped(cx, report, kind, &element.source)?;
            return Ok(());
        };
        let mut reasons = Vec::new();
        let mut fidelity = P::fidelity(self.relocate, &element.shape, &shape);
        if matches!(
            fidelity,
            ConversionFidelity::Drawn | ConversionFidelity::Approximated
        ) {
            reasons.push("relocated_shape");
        }
        match &element.shape {
            Shape::Text(text)
                if text.height.meaning != HeightMeaning::Declared("dxf.TEXT.40".into()) =>
            {
                reasons.push("text_height_meaning");
                fidelity = fidelity.max(ConversionFidelity::Drawn);
            }
            Shape::Formatted(text)
                if text.height.meaning != HeightMeaning::Declared("dxf.MTEXT.40".into()) =>
            {
                reasons.push("text_height_meaning");
                fidelity = fidelity.max(ConversionFidelity::Drawn);
            }
            _ => {}
        }
        if let (Shape::Curve(Edge::Spline(before)), Shape::Curve(Edge::Spline(after))) =
            (&element.shape, &shape)
        {
            if before.fit_tolerance != after.fit_tolerance {
                reasons.push("spline_fit_tolerance");
            }
            if before.knot_parameterization != after.knot_parameterization {
                reasons.push("spline_knot_parameterization");
            }
        }
        if let (Shape::Symbol(symbol), Some(name)) = (&mut shape, block) {
            name.clone_into(&mut symbol.key.name);
        }
        let Some(layer) = self.layer(element.layer) else {
            report_dropped(cx, report, kind, &element.source)?;
            return Ok(());
        };
        let style = self.style(&element.style);
        if self.style_loses_values(&element.style, &style) {
            reasons.push("style_default");
            fidelity = fidelity.max(ConversionFidelity::Drawn);
        }
        let mut addition = Addition::new(
            Edit::Add {
                place: place.id.clone(),
                element: Box::new((place.wrap)(Element {
                    source: Ref::default(),
                    layer,
                    style,
                    shape,
                })),
            },
            kind,
            &element.source,
        );
        addition.fidelity = fidelity;
        addition.reasons = reasons;
        edits.push(addition);
        for text in detached {
            let before = edits.len();
            self.element(&text, place, edits, report, cx)?;
            if let Some(addition) = edits.get_mut(before) {
                addition.detached = true;
            }
        }
        Ok(())
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

    fn style_loses_values(
        &self,
        before: &zumen_drawing::Style,
        after: &zumen_drawing::Style,
    ) -> bool {
        before.color != after.color
            || matches!(before.line_type, Inherit::Own(id)
                if map_line_type(self.source, self.target, id).is_none())
    }

    /// 文字スタイルは名前で指す。書いた先にないものは、Standard にする
    fn text_style(&self, style: &mut String) {
        if self.target.text_style(style).is_none() {
            "Standard".clone_into(style);
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
/// 当てられなかった試しの診断は捨てる。
///
/// 中身のない用紙と中身のある用紙では、読み直したときの用紙の ID が変わる（`layout.rs`）。そのため、用紙に
/// 初めて要素を足せたら読み直し、`paper`（用紙の場所の ID と、足す要素の点が持つ用紙の ID）の ID が変わって
/// いれば、まだ当てていない用紙の要素の点を新しい ID に直す
fn add_all<'a>(
    file: &mut DxfFile,
    edits: Vec<Addition<'a>>,
    mut paper: Option<(String, SheetId)>,
    report: &mut Report,
    cx: &mut Cx<'_>,
) -> Result<Added<'a>, Error> {
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
                for addition in &edits {
                    let (fidelity, reasons) = if addition.detached {
                        (ConversionFidelity::Drawn, ["detached_attribute"].as_slice())
                    } else {
                        (addition.fidelity, addition.reasons.as_slice())
                    };
                    report
                        .record_with_reasons(
                            addition.kind,
                            fidelity,
                            &addition.source,
                            u64::from(matches!(addition.edit, Edit::Add { .. })),
                            reasons,
                            cx,
                        )
                        .map_err(|error| failed(cx, error))?;
                }
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
                let on_paper = edits.iter().any(|addition| {
                    matches!(addition.edit, Edit::Add { element: ref placed, .. } if matches!(**placed, Placed::Paper(_)))
                });
                if on_paper && let Some((place, sheet)) = paper.take() {
                    resheet(file, &place, sheet, &mut pending, cx)?;
                }
            }
            Err(Error::Rejected | Error::Invalid | Error::Unsupported) => {
                if let [addition] = edits.as_slice() {
                    report_dropped(cx, report, addition.kind, &addition.source)?;
                    if let Some(layer) = addition.layer_fields {
                        report_layer_fields(layer, report, cx)?;
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

/// 用紙を読み直し、場所の ID が `place` の用紙の ID が `sheet` から変わっていれば、`pending` の用紙の要素の
/// 点を新しい ID に直す
fn resheet(
    file: &DxfFile,
    place: &str,
    sheet: SheetId,
    pending: &mut [Vec<Addition<'_>>],
    cx: &mut Cx<'_>,
) -> Result<(), Error> {
    if pending.iter().all(Vec::is_empty) {
        return Ok(());
    }
    let read = lift(file, cx)?;
    let Some(now) = read
        .sheets
        .iter()
        .find(|found| id::sheet(found) == place)
        .map(|found| found.id)
        .filter(|now| *now != sheet)
    else {
        return Ok(());
    };
    for addition in pending.iter_mut().flatten() {
        cx.check(Kind::Work, 1).map_err(|stop| stopped(cx, stop))?;
        if let Edit::Add { element, .. } = &mut addition.edit
            && let Placed::Paper(element) = &mut **element
            && let Some(shape) = reseat(
                &element.shape,
                &|point: PaperPoint| PaperPoint {
                    sheet: now,
                    ..point
                },
                &|length| length,
            )
        {
            element.shape = shape;
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

/// 止めた理由を診断にする
fn stopped(cx: &mut Cx<'_>, stop: Stop) -> Error {
    cx.report(stop.to_diagnostic());
    Error::Stopped(stop)
}

/// ひな形を読めないなどの不具合
fn internal(cx: &mut Cx<'_>) -> Error {
    cx.report(Diagnostic::new(codes::INTERNAL_PANIC_CAUGHT));
    Error::Invalid
}

fn failed(cx: &mut Cx<'_>, error: Error) -> Error {
    match error {
        Error::Stopped(stop) => stopped(cx, stop),
        _ => internal(cx),
    }
}

fn lift(file: &DxfFile, cx: &mut Cx<'_>) -> Result<Drawing2D, Error> {
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

/// 形の点を `point` で、長さを `length` で写す。表・下絵・ハッチングは `None`
fn reseat<P: Copy>(
    shape: &Shape<P>,
    point: &dyn Fn(P) -> P,
    length: &dyn Fn(Length) -> Length,
) -> Option<Shape<P>> {
    Some(match shape {
        Shape::Line { start, end } => Shape::Line {
            start: point(*start),
            end: point(*end),
        },
        Shape::Circle { center, radius } => Shape::Circle {
            center: point(*center),
            radius: length(*radius),
        },
        Shape::Curve(edge) => Shape::Curve(match edge {
            Edge::Line { start, end } => Edge::Line {
                start: point(*start),
                end: point(*end),
            },
            Edge::Arc {
                center,
                radius,
                start,
                end,
                counterclockwise,
            } => Edge::Arc {
                center: point(*center),
                radius: length(*radius),
                start: *start,
                end: *end,
                counterclockwise: *counterclockwise,
            },
            Edge::Elliptic {
                center,
                major,
                ratio,
                start,
                end,
                counterclockwise,
            } => Edge::Elliptic {
                center: point(*center),
                major: point(*major),
                ratio: *ratio,
                start: *start,
                end: *end,
                counterclockwise: *counterclockwise,
            },
            Edge::Spline(spline) => Edge::Spline(Spline {
                control: spline.control.iter().map(|p| point(*p)).collect(),
                fit: spline.fit.iter().map(|p| point(*p)).collect(),
                ..spline.clone()
            }),
            Edge::Clothoid {
                origin,
                parameter,
                angle,
                counterclockwise,
                start,
                end,
            } => Edge::Clothoid {
                origin: point(*origin),
                parameter: length(*parameter),
                angle: *angle,
                counterclockwise: *counterclockwise,
                start: length(*start),
                end: length(*end),
            },
        }),
        Shape::Polyline(polyline) => Shape::Polyline(Polyline {
            vertices: polyline
                .vertices
                .iter()
                .map(|vertex| Vertex {
                    point: point(vertex.point),
                    bulge: vertex.bulge,
                })
                .collect(),
            closed: polyline.closed,
            width: polyline.width.map(length),
            vertex_widths: polyline
                .vertex_widths
                .iter()
                .map(|[a, b]| [length(*a), length(*b)])
                .collect(),
            line_type_generation: polyline.line_type_generation,
        }),
        Shape::Text(text) => Shape::Text(reseat_text(text, point, length)),
        Shape::Formatted(text) => Shape::Formatted(reseat_formatted(text, point, length)),
        Shape::Dimension(dimension) => Shape::Dimension(Dimension {
            kind: match dimension.kind {
                DimensionKind::Linear {
                    first,
                    second,
                    line,
                    rotation,
                    oblique,
                } => DimensionKind::Linear {
                    first: point(first),
                    second: point(second),
                    line: point(line),
                    rotation,
                    oblique,
                },
                DimensionKind::Aligned {
                    first,
                    second,
                    line,
                    oblique,
                } => DimensionKind::Aligned {
                    first: point(first),
                    second: point(second),
                    line: point(line),
                    oblique,
                },
                DimensionKind::Angular {
                    vertex,
                    first,
                    second,
                    arc,
                } => DimensionKind::Angular {
                    vertex: point(vertex),
                    first: point(first),
                    second: point(second),
                    arc: point(arc),
                },
                DimensionKind::AngularLines { first, second, arc } => DimensionKind::AngularLines {
                    first: first.map(point),
                    second: second.map(point),
                    arc: point(arc),
                },
                DimensionKind::ArcLength {
                    center,
                    first,
                    second,
                    arc,
                } => DimensionKind::ArcLength {
                    center: point(center),
                    first: point(first),
                    second: point(second),
                    arc: point(arc),
                },
                DimensionKind::Radius {
                    center,
                    chord,
                    leader,
                } => DimensionKind::Radius {
                    center: point(center),
                    chord: point(chord),
                    leader: length(leader),
                },
                DimensionKind::Diameter {
                    chord,
                    opposite,
                    leader,
                } => DimensionKind::Diameter {
                    chord: point(chord),
                    opposite: point(opposite),
                    leader: length(leader),
                },
                DimensionKind::Ordinate {
                    origin,
                    feature,
                    leader_end,
                    axis,
                } => DimensionKind::Ordinate {
                    origin: point(origin),
                    feature: point(feature),
                    leader_end: point(leader_end),
                    axis,
                },
            },
            text: zumen_drawing::DimensionText {
                position: dimension.text.position.map(point),
                ..dimension.text
            },
            ..dimension.clone()
        }),
        Shape::Leader(leader) => Shape::Leader(Leader {
            path: leader.path.iter().map(|p| point(*p)).collect(),
            ..leader.clone()
        }),
        Shape::Callout(callout) => Shape::Callout(Callout {
            branches: callout
                .branches
                .iter()
                .map(|branch| Branch {
                    lines: branch
                        .lines
                        .iter()
                        .map(|line| line.iter().map(|p| point(*p)).collect())
                        .collect(),
                    end: branch.end.map(point),
                    dogleg: branch.dogleg.as_ref().map(|dogleg| Dogleg {
                        length: length(dogleg.length),
                        ..*dogleg
                    }),
                })
                .collect(),
            content: match &callout.content {
                CalloutContent::None => CalloutContent::None,
                CalloutContent::Text(text) => {
                    CalloutContent::Text(Box::new(reseat_formatted(text, point, length)))
                }
                CalloutContent::Symbol { symbol, values } => CalloutContent::Symbol {
                    symbol: Box::new(reseat_symbol(symbol, point, length)),
                    values: values.clone(),
                },
            },
            arrow_size: length(callout.arrow_size),
            ..callout.clone()
        }),
        Shape::Symbol(symbol) => Shape::Symbol(reseat_symbol(symbol, point, length)),
        Shape::Point(dot) => Shape::Point(zumen_drawing::Point {
            position: point(dot.position),
            ..dot.clone()
        }),
        Shape::Table(_) | Shape::Underlay(_) | Shape::Hatch(_) => return None,
    })
}

fn reseat_text<P: Copy>(
    text: &Text<P>,
    point: &dyn Fn(P) -> P,
    length: &dyn Fn(Length) -> Length,
) -> Text<P> {
    Text {
        height: zumen_drawing::Height {
            value: length(text.height.value),
            ..text.height.clone()
        },
        placement: match text.placement {
            Placement::Anchor {
                point: anchor,
                horizontal,
                vertical,
            } => Placement::Anchor {
                point: point(anchor),
                horizontal,
                vertical,
            },
            Placement::Between { first, second, fit } => Placement::Between {
                first: point(first),
                second: point(second),
                fit,
            },
        },
        ..text.clone()
    }
}

fn reseat_formatted<P: Copy>(
    text: &FormattedText<P>,
    point: &dyn Fn(P) -> P,
    length: &dyn Fn(Length) -> Length,
) -> FormattedText<P> {
    FormattedText {
        height: zumen_drawing::Height {
            value: length(text.height.value),
            ..text.height.clone()
        },
        point: point(text.point),
        width: text.width.map(length),
        frame_height: text.frame_height.map(length),
        ..text.clone()
    }
}

fn reseat_symbol<P: Copy>(
    symbol: &Symbol<P>,
    point: &dyn Fn(P) -> P,
    length: &dyn Fn(Length) -> Length,
) -> Symbol<P> {
    Symbol {
        position: point(symbol.position),
        grid: symbol.grid.as_ref().map(|grid| Grid {
            column_spacing: length(grid.column_spacing),
            row_spacing: length(grid.row_spacing),
            ..*grid
        }),
        attributes: symbol
            .attributes
            .iter()
            .map(|attribute| Attribute {
                text: reseat_text(&attribute.text, point, length),
                formatted: attribute
                    .formatted
                    .as_ref()
                    .map(|text| reseat_formatted(text, point, length)),
                ..attribute.clone()
            })
            .collect(),
        ..symbol.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{Data, Section};
    use zumen_base::{Budget, CancelToken, Code, Limit, Limits, Ref};
    use zumen_civil::{Alignment, CivilModel, Horizontal, HorizontalKind, Vertical, VerticalKind};
    use zumen_crs::MapPoint;
    use zumen_drawing::{Axes, ModelSpace, Symbol, SymbolDefinition, SymbolKey, Unit};

    #[test]
    fn a_remapped_line_type_remains_equivalent_and_layer_defaults_are_exposed() {
        let mut budget = Budget::default();
        let mut diagnostics = Diagnostics::new();
        let cancel = CancelToken::new();
        let mut cx = Cx::new(&mut budget, &mut diagnostics, &cancel);
        let template = NewDxfTemplate::new(&Drawing2D::default());
        let mut drawing = Dxf::read(template.bytes(), &mut cx)
            .unwrap()
            .lift(&mut cx)
            .unwrap();
        let continuous = drawing
            .line_types
            .iter()
            .position(|line_type| line_type.name.eq_ignore_ascii_case("Continuous"))
            .unwrap();
        drawing.line_types.swap(0, continuous);
        for (index, line_type) in drawing.line_types.iter_mut().enumerate() {
            line_type.id = LineTypeId(index.try_into().unwrap());
        }
        let mut fallback = drawing.layers.first().unwrap().clone();
        fallback.id = LayerId(drawing.layers.len().try_into().unwrap());
        fallback.name = "Fallback".into();
        fallback.source = Ref::at_record(901);
        let style = fallback.style.as_mut().unwrap();
        style.line_type = Some(LineTypeId(999));
        style.color = Some(Color {
            code: Some(ColorCode {
                system: ColorSystem::Jww,
                value: 1,
            }),
            rgb: None,
        });
        drawing.layers.push(fallback);
        let layer = drawing.layers.first().unwrap().id;
        let space = drawing.spaces.first_mut().unwrap();
        let point = ModelPoint {
            space: space.id,
            x: 0.0,
            y: 0.0,
        };
        space.elements.push(Element {
            source: Ref::at_record(900),
            layer,
            style: zumen_drawing::Style {
                line_type: Inherit::Own(LineTypeId(0)),
                transparency: Inherit::Layer,
                ..zumen_drawing::Style::BLOCK
            },
            shape: Shape::Line {
                start: point,
                end: point.at([10.0, 0.0]),
            },
        });
        let written = lower_to_dxf_with_report(&drawing, &Reseat, 1000, &mut cx).unwrap();
        let line = written
            .report
            .details
            .iter()
            .find(|row| row.source.record() == Some(900))
            .unwrap();
        assert_eq!(line.fidelity, ConversionFidelity::Equivalent);
        assert!(line.reasons.is_empty());
        let layer = written
            .report
            .details
            .iter()
            .find(|row| row.source.record() == Some(901) && row.kind == "layer")
            .unwrap();
        assert_eq!(layer.fidelity, ConversionFidelity::Drawn);
        assert!(
            layer
                .reasons
                .iter()
                .any(|reason| reason == "line_type_default")
        );
        assert!(layer.reasons.iter().any(|reason| reason == "color_code"));
        let read = written.file.lift(&mut cx).unwrap();
        let element = read
            .spaces
            .iter()
            .flat_map(|space| &space.elements)
            .find(|element| matches!(element.shape, Shape::Line { .. }))
            .unwrap();
        let Inherit::Own(line_type) = element.style.line_type else {
            panic!()
        };
        assert_ne!(line_type, LineTypeId(0));
        assert!(
            read.line_type(line_type)
                .unwrap()
                .name
                .eq_ignore_ascii_case("Continuous")
        );
        let fallback = read
            .layers
            .iter()
            .find(|layer| layer.name == "Fallback")
            .unwrap()
            .style
            .unwrap();
        assert_eq!(fallback.color.unwrap().code, Some(ACI_WHITE));
        assert!(
            read.line_type(fallback.line_type.unwrap())
                .unwrap()
                .name
                .eq_ignore_ascii_case("Continuous")
        );
    }

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
            let Ok(converted) = lower_to_dxf(&drawing, &mut cx) else {
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
        // 文字も落とさずに書く
        assert!(
            !codes.contains(&codes::CONVERT_ELEMENT_UNSUPPORTED),
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

    /// 縦断図の用紙だけを持つ図面。`shape` があれば、焼き込むビューの空間に足す
    fn profile_with(shape: Option<Shape<ModelPoint>>) -> Drawing2D {
        let mut budget = Budget::new(Limits::default());
        let mut diag = Diagnostics::new();
        let cancel = CancelToken::new();
        let mut cx = Cx::new(&mut budget, &mut diag, &cancel);
        let model = model();
        let mut drawing = zumen_civil_drawing::drawing(
            &model,
            &model.alignments[0],
            &zumen_civil_drawing::Options::default(),
            &mut cx,
        )
        .unwrap();
        let profile = drawing.sheets.remove(1);
        let view = profile.views[0].clone();
        assert!(bake::bakes(&view));
        drawing.sheets = vec![profile];
        if let Some(shape) = shape {
            let space = drawing
                .spaces
                .iter_mut()
                .find(|space| space.id == view.space)
                .unwrap();
            if !space.order.is_empty() && space.order.len() == space.elements.len() {
                space.order.push(space.elements.len());
            }
            let like = space.elements[0].clone();
            space.elements.push(Element { shape, ..like });
        }
        drawing
    }

    /// 上限 `limits` で書く。書けたかと、使った予算と、診断の code を返す
    fn written_within(drawing: &Drawing2D, limits: Limits) -> (bool, Budget, Vec<Code>) {
        let mut budget = Budget::new(limits);
        let mut diag = Diagnostics::new();
        let cancel = CancelToken::new();
        let written = {
            let mut cx = Cx::new(&mut budget, &mut diag, &cancel);
            lower_to_dxf(drawing, &mut cx).is_ok()
        };
        let codes = diag.iter().map(|d| d.code()).collect();
        (written, budget, codes)
    }

    /// 焼き込むビューの曲線は、折れ線にする手間と作る点を予算に数える。クロソイドの手間と、膨らみを持つ
    /// 折れ線の点は、作る前に数え、スプラインの手間と、ほかの曲線の点は、作ったあとに数える（#753・#756）。
    /// どれも、数えるのに要る分の半分しか残さなければ、書かずに止まる
    #[test]
    fn a_baked_view_counts_the_work_and_points_of_its_curves() {
        let base = profile_with(None);
        let (written, used, _) = written_within(&base, Limits::default());
        assert!(written);
        let view = &base.sheets[0].views[0];
        let at = |x, y| ModelPoint {
            space: view.space,
            x,
            y,
        };
        let length = |value| {
            Length::Model(zumen_drawing::ModelLength {
                space: view.space,
                value,
            })
        };
        // 区間が原点から離れたクロソイドは、原点から区間の初めまで刻んで積む手間も要る
        let clothoid = Shape::Curve(Edge::Clothoid {
            origin: at(0.0, 0.0),
            parameter: length(50.0),
            angle: 0.0,
            counterclockwise: true,
            start: length(35.0),
            end: length(50.0),
        });
        // 半径 50 m の円を、膨らみを持つ 2 つの区間で描く
        let bulged = Shape::Polyline(Polyline {
            vertices: vec![
                Vertex {
                    point: at(0.0, 0.0),
                    bulge: 1.0,
                },
                Vertex {
                    point: at(100.0, 0.0),
                    bulge: 1.0,
                },
            ],
            closed: true,
            width: None,
            vertex_widths: Vec::new(),
            line_type_generation: false,
        });
        // 40 の制御点を持つ 3 次のスプラインと、半径 50 m の円
        let spline = Shape::Curve(Edge::Spline(zumen_drawing::Spline {
            degree: 3,
            knots: (0..44)
                .map(|i: i32| f64::from((i - 3).clamp(0, 37)))
                .collect(),
            control: (0..40)
                .map(|i| at(f64::from(i) * 5.0, 50.0 + f64::from(i % 3)))
                .collect(),
            weights: Vec::new(),
            fit: Vec::new(),
            start_tangent: None,
            end_tangent: None,
            closed: false,
            periodic: false,
            fit_tolerance: None,
            knot_parameterization: None,
        }));
        let circle = Shape::Circle {
            center: at(100.0, 50.0),
            radius: length(50.0),
        };
        for (shape, kind, code) in [
            (clothoid, Kind::Work, codes::BUDGET_WORK_EXCEEDED),
            (bulged, Kind::Segments, codes::BUDGET_SEGMENTS_EXCEEDED),
            (spline, Kind::Work, codes::BUDGET_WORK_EXCEEDED),
            (circle, Kind::Segments, codes::BUDGET_SEGMENTS_EXCEEDED),
        ] {
            let drawing = profile_with(Some(shape));
            let element = drawing
                .spaces
                .iter()
                .find(|space| space.id == view.space)
                .and_then(|space| space.elements.last())
                .unwrap();
            let (work, points) = bake::before(&element.shape, view);
            let baked = bake::element_of(element, view);
            let needed = if kind == Kind::Work {
                work + baked.work
            } else {
                points + baked.points
            };
            assert!(needed > 100, "{:?} {needed}", element.shape);
            let limits = Limits::default().with(kind, Limit::Finite(used.used(kind) + needed / 2));
            let (written, _, codes) = written_within(&drawing, limits);
            assert!(!written, "{kind:?}");
            assert!(codes.contains(&code), "{codes:?}");
        }
    }

    /// 作業量の上限をどこにしても、途中で止めたときは `Stopped` を返し、止めた理由を 1 件だけ最後に報告する。
    /// 中断の合図でも `Stopped` を返す
    #[test]
    fn stopping_anywhere_returns_stopped_with_one_reason() {
        let drawing = {
            let mut budget = Budget::new(Limits::default());
            let mut diag = Diagnostics::new();
            let cancel = CancelToken::new();
            let mut cx = Cx::new(&mut budget, &mut diag, &cancel);
            let model = model();
            zumen_civil_drawing::drawing(
                &model,
                &model.alignments[0],
                &zumen_civil_drawing::Options::default(),
                &mut cx,
            )
            .unwrap()
        };
        let run = |limit: Limit, cancelled: bool| {
            let mut budget = Budget::new(Limits::default().with(Kind::Work, limit));
            let mut diag = Diagnostics::new();
            let cancel = CancelToken::new();
            if cancelled {
                cancel.cancel();
            }
            let result = lower_to_dxf(&drawing, &mut Cx::new(&mut budget, &mut diag, &cancel))
                .map(|lowered| lowered.bytes);
            (result, budget.used(Kind::Work), diag.into_vec())
        };
        let (full, used, _) = run(Limit::Unlimited, false);
        let full = full.unwrap();
        assert_eq!(run(Limit::Finite(used), false).0.unwrap(), full);
        let stops = [codes::BUDGET_WORK_EXCEEDED, codes::CALL_REQUEST_CANCELLED];
        let step = (used / 97).max(1);
        let limits = (0..used)
            .step_by(usize::try_from(step).unwrap())
            .chain([used - 1]);
        for (limit, cancelled) in limits
            .map(|limit| (Limit::Finite(limit), false))
            .chain([(Limit::Unlimited, true)])
        {
            let (result, _, diagnostics) = run(limit, cancelled);
            assert!(matches!(result, Err(Error::Stopped(_))), "{limit:?}");
            let reasons: Vec<_> = diagnostics
                .iter()
                .filter(|d| stops.contains(&d.code()))
                .collect();
            assert_eq!(reasons.len(), 1, "{limit:?} {diagnostics:?}");
            assert_eq!(
                diagnostics.last().map(Diagnostic::code),
                Some(reasons[0].code())
            );
            assert!(
                !diagnostics
                    .iter()
                    .any(|d| d.code() == codes::INTERNAL_PANIC_CAUGHT)
            );
            let expected = if cancelled {
                codes::CALL_REQUEST_CANCELLED
            } else {
                codes::BUDGET_WORK_EXCEEDED
            };
            assert_eq!(reasons[0].code(), expected);
        }
    }

    /// 要素を移した直後に中断し、lower の次の予算検査が合図を受け取ることを確かめる
    #[test]
    fn cancellation_during_relocation_stops_the_lower() {
        struct CancelAfterElement {
            cancel: CancelToken,
            visited: std::cell::Cell<u64>,
        }

        impl Relocate for CancelAfterElement {
            fn model(
                &self,
                shape: &Shape<ModelPoint>,
                origin: ModelPoint,
            ) -> Option<Shape<ModelPoint>> {
                self.visited.set(self.visited.get() + 1);
                self.cancel.cancel();
                Reseat.model(shape, origin)
            }

            fn paper(
                &self,
                shape: &Shape<PaperPoint>,
                origin: PaperPoint,
            ) -> Option<Shape<PaperPoint>> {
                self.visited.set(self.visited.get() + 1);
                self.cancel.cancel();
                Reseat.paper(shape, origin)
            }
        }

        let drawing = profile_with(None);
        let mut budget = Budget::default();
        let mut diagnostics = Diagnostics::new();
        let cancel = CancelToken::new();
        let relocate = CancelAfterElement {
            cancel: cancel.clone(),
            visited: std::cell::Cell::new(0),
        };
        let result = lower_to_dxf_with(
            &drawing,
            &relocate,
            &mut Cx::new(&mut budget, &mut diagnostics, &cancel),
        );
        assert_eq!(relocate.visited.get(), 1);
        assert!(matches!(result, Err(Error::Stopped(Stop::Cancelled))));
        assert_eq!(
            diagnostics.iter().map(Diagnostic::code).collect::<Vec<_>>(),
            [codes::CALL_REQUEST_CANCELLED]
        );
    }

    /// 文字を、`apply` が足さない高さの意味にして移す
    struct Undeclared;

    impl Relocate for Undeclared {
        fn model(
            &self,
            shape: &Shape<ModelPoint>,
            origin: ModelPoint,
        ) -> Option<Shape<ModelPoint>> {
            Reseat.model(shape, origin)
        }

        fn paper(
            &self,
            shape: &Shape<PaperPoint>,
            origin: PaperPoint,
        ) -> Option<Shape<PaperPoint>> {
            match Reseat.paper(shape, origin)? {
                Shape::Text(mut text) => {
                    text.height.meaning = HeightMeaning::Frame;
                    Some(Shape::Text(text))
                }
                shape => Some(shape),
            }
        }
    }

    /// 用紙の要素を分けて当て直しても、初めに当てた後に変わる用紙の ID に合わせて、残りの要素を書く
    #[test]
    fn paper_elements_after_a_rejected_one_are_written() {
        let mut budget = Budget::new(Limits::default());
        let mut diag = Diagnostics::new();
        let cancel = CancelToken::new();
        let mut cx = Cx::new(&mut budget, &mut diag, &cancel);
        let model = model();
        let mut drawing = zumen_civil_drawing::drawing(
            &model,
            &model.alignments[0],
            &zumen_civil_drawing::Options::default(),
            &mut cx,
        )
        .unwrap();
        let mut profile = drawing.sheets.remove(1);
        profile.views.clear();
        profile.order.clear();
        let texts = profile
            .elements
            .iter()
            .filter(|element| matches!(element.shape, Shape::Text(_)))
            .count();
        let lines = profile.elements.len() - texts;
        assert!(texts >= 1 && lines >= 2, "{texts} {lines}");
        // 当てられない文字を先頭に置き、分けて当て直させる
        profile
            .elements
            .sort_by_key(|element| !matches!(element.shape, Shape::Text(_)));
        drawing.sheets = vec![profile];
        let Ok(converted) = lower_to_dxf_with(&drawing, &Undeclared, &mut cx) else {
            panic!("書けない");
        };
        let Ok(read) = lift(&converted.file, &mut cx) else {
            panic!("読めない");
        };
        assert_eq!(read.sheets[0].elements.len(), lines);
        let dropped: u64 = diag
            .iter()
            .filter(|d| d.code() == codes::CONVERT_ELEMENT_UNSUPPORTED)
            .map(|d| d.count())
            .sum();
        assert_eq!(dropped, texts as u64);
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
            let Ok(converted) = lower_to_dxf(&drawing, &mut cx) else {
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

    fn block_key(name: &str) -> SymbolKey {
        SymbolKey {
            namespace: "dxf.block".into(),
            name: name.to_owned(),
        }
    }

    fn block_space(id: u32, elements: Vec<Element<ModelPoint>>) -> ModelSpace {
        ModelSpace {
            id: SpaceId(id),
            unit: Unit::Millimeter,
            axes: Axes::EastNorth,
            elements,
            order: vec![],
        }
    }

    fn by_layer() -> zumen_drawing::Style {
        zumen_drawing::Style {
            color: Inherit::Layer,
            line_type: Inherit::Layer,
            line_type_scale: 1.0,
            line_weight: None,
            transparency: Inherit::Layer,
        }
    }

    /// `id` 番の空間と `id` 番のレコードを持つ記号の定義。`x` があれば、X が `x` の縦の線を 1 本持つ
    fn block_definition(id: u32, name: &str, external: bool, x: Option<f64>) -> SymbolDefinition {
        let at = |y: f64| ModelPoint {
            space: SpaceId(id),
            x: x.unwrap_or_default(),
            y,
        };
        let elements = x
            .map(|_| Element {
                source: Ref::at_record(u64::from(id) + 100),
                layer: LayerId(0),
                style: by_layer(),
                shape: Shape::Line {
                    start: at(0.0),
                    end: at(1.0),
                },
            })
            .into_iter()
            .collect();
        SymbolDefinition {
            key: block_key(name),
            space: block_space(id, elements),
            base: None,
            attributes: vec![],
            anonymous: false,
            external: external.then(|| "other.dwg".to_owned()),
            source: Ref::at_record(u64::from(id)),
        }
    }

    fn placed_block(name: &str, record: u64) -> Element<ModelPoint> {
        Element {
            source: Ref::at_record(record),
            layer: LayerId(0),
            style: by_layer(),
            shape: Shape::Symbol(Symbol {
                key: block_key(name),
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
        }
    }

    /// 図面モデルを新しい DXF に書いて読み直し、読んだ図面と、落としたものの報告（種類・件数・場所）を返す
    #[allow(clippy::type_complexity)]
    fn convert_and_read(
        drawing: &Drawing2D,
    ) -> (Drawing2D, Vec<(Option<Value>, u64, Vec<Option<u64>>)>) {
        let mut budget = Budget::new(Limits::default());
        let mut diag = Diagnostics::new();
        let cancel = CancelToken::new();
        let read = {
            let mut cx = Cx::new(&mut budget, &mut diag, &cancel);
            let Ok(converted) = lower_to_dxf(drawing, &mut cx) else {
                panic!("書けない");
            };
            r2004_structure(&converted.file, &mut cx);
            let Ok(read) = lift(&converted.file, &mut cx) else {
                panic!("読めない");
            };
            read
        };
        let dropped = diag
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
        (read, dropped)
    }

    /// 外部参照の記号の定義は書かず、定義と、それを置いた記号を、それぞれ元の順に報告する
    #[test]
    fn external_symbols_are_reported_and_not_written() {
        let drawing = Drawing2D {
            spaces: vec![block_space(
                0,
                vec![placed_block("外部B", 10), placed_block("外部A", 11)],
            )],
            symbols: vec![
                block_definition(1, "部品", false, None),
                block_definition(2, "外部A", true, None),
                block_definition(3, "外部B", true, None),
            ],
            ..Drawing2D::default()
        };
        let (read, dropped) = convert_and_read(&drawing);
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

    /// キーが重なる記号の定義は、キーごとに先の定義だけを書く。先の定義が外部参照なら書かない。あとの定義と、外部参照の
    /// 定義を置いた記号を報告し、部品の名前は重ならない（#656）
    #[test]
    fn symbol_definitions_with_the_same_key_write_the_first() {
        let drawing = Drawing2D {
            spaces: vec![block_space(
                0,
                vec![
                    placed_block("K", 20),
                    placed_block("L", 21),
                    placed_block("M", 22),
                ],
            )],
            symbols: vec![
                block_definition(1, "K", false, Some(1.0)),
                block_definition(2, "K", true, Some(2.0)),
                block_definition(3, "L", true, Some(3.0)),
                block_definition(4, "L", false, Some(4.0)),
                block_definition(5, "M", false, Some(5.0)),
                block_definition(6, "M", false, Some(6.0)),
            ],
            layers: vec![Layer {
                id: LayerId(0),
                name: "0".to_owned(),
                state: None,
                style: None,
                follows_placement: false,
                source: Ref::default(),
                description: None,
            }],
            ..Drawing2D::default()
        };
        let (read, dropped) = convert_and_read(&drawing);
        assert_eq!(
            dropped,
            [
                (
                    Some(Value::Key("duplicate_symbol_definition")),
                    3,
                    vec![Some(2), Some(4), Some(6)]
                ),
                (
                    Some(Value::Key("external_symbol_definition")),
                    1,
                    vec![Some(3)]
                ),
                (Some(Value::Key("external_symbol")), 1, vec![Some(21)]),
            ]
        );
        // 部品は K と M の 1 つずつで、中身は先の定義の線
        let written: Vec<_> = read
            .symbols
            .iter()
            .map(|definition| {
                let xs: Vec<_> = definition
                    .space
                    .elements
                    .iter()
                    .filter_map(|element| match &element.shape {
                        Shape::Line { start, .. } => Some(start.x),
                        _ => None,
                    })
                    .collect();
                (definition.key.name.as_str(), xs)
            })
            .collect();
        assert_eq!(written, [("K", vec![1.0]), ("M", vec![5.0])]);
        // 置いた記号は、書いた K と M の 2 つ
        let placed: Vec<_> = read
            .spaces
            .iter()
            .flat_map(|space| &space.elements)
            .filter_map(|element| match &element.shape {
                Shape::Symbol(symbol) => Some(symbol.key.name.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(placed, ["K", "M"]);

        // 外部参照の定義どうしが重なると、先の定義を外部参照として、あとの定義を重なりとして報告する
        let drawing = Drawing2D {
            spaces: vec![block_space(0, vec![placed_block("X", 30)])],
            symbols: vec![
                block_definition(1, "X", true, None),
                block_definition(2, "X", true, None),
            ],
            ..Drawing2D::default()
        };
        let (read, dropped) = convert_and_read(&drawing);
        assert_eq!(
            dropped,
            [
                (
                    Some(Value::Key("external_symbol_definition")),
                    1,
                    vec![Some(1)]
                ),
                (
                    Some(Value::Key("duplicate_symbol_definition")),
                    1,
                    vec![Some(2)]
                ),
                (Some(Value::Key("external_symbol")), 1, vec![Some(30)]),
            ]
        );
        assert!(read.symbols.is_empty());
    }
}
