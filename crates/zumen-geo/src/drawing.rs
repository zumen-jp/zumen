//! 地物を、東が X・北が Y の実寸の平面図へ移す。語彙は利用側のプロファイルから受け取る。
use zumen_base::{Cx, Kind, Ref};
use zumen_crs::Zone;
use zumen_drawing::{self as d, Drawing2D, ModelPoint, SpaceId};
use zumen_geom3d::Failure;

use crate::{
    Feature, Form, GeoModel, Location, Selection,
    visual::{self, Projection},
};

/// 型・分類コードとレイヤの対応。型が同じときはコードを指定した規則を優先する。
#[derive(Clone, Copy, Debug)]
pub struct LayerRule<'a> {
    /// 接頭辞を含む元の型。
    pub kind: &'a str,
    /// 分類コード。None はその型のすべてのコード。
    pub code: Option<&'a str>,
    /// プロファイルが定めたレイヤ名。
    pub layer: &'a str,
}

/// 平面図の条件。モデルの値を変更せず、紙や縮尺は作らない。
#[derive(Clone, Copy, Debug, Default)]
pub struct Options<'a> {
    /// 投影先。平面座標なら元の系、経緯度なら範囲の中心から計算用の系を仮定する。
    pub zone: Option<Zone>,
    /// 地物と LOD の選択。
    pub selection: Selection<'a>,
    /// プロファイルの型の語彙。
    pub layers: &'a [LayerRule<'a>],
    /// 語彙にない型のレイヤ。None なら診断してその地物を除く。
    pub fallback_layer: Option<&'a str>,
}

/// 図面の要素から、元の地物・形・輪へ戻る対応。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Source {
    /// 通常の図面の命名規則による ID。
    pub drawing_id: String,
    /// 元の地物 ID。
    pub feature: String,
    /// 元の地物の中の形の番号。
    pub geometry: u64,
    /// 面の外周は 0、穴は 1 から。点・線では None。
    pub ring: Option<u64>,
}

/// 1 つの実寸の空間を持つ図面。メートル、X が東、Y が北。
#[derive(Clone, Debug, PartialEq)]
pub struct Output {
    /// 図面モデル。要素の出所にも元の地物 ID と入力位置を保つ。
    pub drawing: Drawing2D,
    /// 出力した平面直角座標系の系。
    pub zone: Zone,
    /// 図面の要素と同じ順の対応。
    pub sources: Vec<Source>,
}

/// 点・線・面を平面図へ移す。壊れた面の穴を黙って除くことはせず、その形全体を診断して除く。
pub fn build(model: &GeoModel, options: Options<'_>, cx: &mut Cx) -> Result<Output, Failure> {
    visual::validate(model, options.selection, cx)?;
    validate_rules(options, cx)?;
    let projection = Projection::new(model, options.zone, cx)?;
    cx.check(Kind::DecodedBytes, 2048)?;
    let space = SpaceId(0);
    let mut drawing = Drawing2D {
        spaces: vec![d::ModelSpace {
            id: space,
            unit: d::Unit::Meter,
            axes: d::Axes::EastNorth,
            elements: Vec::new(),
            order: Vec::new(),
        }],
        ..Drawing2D::default()
    };
    let mut sources = Vec::new();
    let mut accounted = 0u64;
    for feature in &model.features {
        if !options.selection.includes(feature, cx)? {
            continue;
        }
        let Some(layer_name) = layer(feature, options, cx)? else {
            visual::omitted(feature, "feature", "layer_missing", cx)?;
            continue;
        };
        let mut layer_id = None;
        for old in &drawing.layers {
            cx.check(Kind::Work, layer_name.len().min(old.name.len()) as u64 + 1)?;
            if old.name == layer_name {
                layer_id = Some(old.id);
                break;
            }
        }
        let layer_id = if let Some(id) = layer_id {
            id
        } else {
            let id = d::LayerId(
                u32::try_from(drawing.layers.len()).map_err(|_| Failure::Invalid("layers"))?,
            );
            visual::reserve(&mut drawing.layers, 1, 256, cx)?;
            let layer = d::Layer {
                id,
                name: visual::text(layer_name, cx)?,
                state: None,
                style: None,
                follows_placement: false,
                description: None,
                source: Ref::default(),
            };
            accounted = accounted.saturating_add(d::check_model_bytes(&layer, cx)?);
            drawing.layers.push(layer);
            id
        };
        for (geometry_index, geometry) in feature.geometries.iter().enumerate() {
            cx.check(Kind::Work, 1)?;
            if !options.selection.geometry(geometry) {
                continue;
            }
            let shapes = match shapes(&geometry.form, projection, space, cx) {
                Ok(shapes) => shapes,
                Err(Failure::Stopped(stop)) => return Err(stop.into()),
                Err(error) => {
                    visual::omitted(
                        feature,
                        visual::form_name(&geometry.form),
                        reason(&error),
                        cx,
                    )?;
                    continue;
                }
            };
            let model_space = drawing
                .spaces
                .first_mut()
                .ok_or(Failure::Invalid("space"))?;
            projection.report(feature, &geometry.form, cx)?;
            for (shape, ring) in shapes {
                cx.check(Kind::Elements, 1)?;
                visual::reserve(&mut model_space.elements, 1, 512, cx)?;
                visual::reserve(&mut model_space.order, 1, 8, cx)?;
                visual::reserve(&mut sources, 1, 96, cx)?;
                let source = visual::source(feature, cx)?;
                let element = d::Element {
                    source,
                    layer: layer_id,
                    style: d::Style {
                        color: d::Inherit::Layer,
                        line_type: d::Inherit::Layer,
                        line_type_scale: 1.0,
                        line_weight: None,
                        transparency: d::Inherit::Layer,
                    },
                    shape,
                };
                accounted = accounted.saturating_add(d::check_model_bytes(&element, cx)?);
                model_space.order.push(model_space.elements.len());
                model_space.elements.push(element);
                sources.push(Source {
                    drawing_id: String::new(),
                    feature: visual::text(&feature.id, cx)?,
                    geometry: geometry_index as u64,
                    ring,
                });
            }
        }
    }
    let mut namer = d::id::Namer::new("space/0");
    let model_space = drawing.spaces.first().ok_or(Failure::Invalid("space"))?;
    for (index, (element, source)) in model_space.elements.iter().zip(&mut sources).enumerate() {
        cx.check(Kind::Work, 128)?;
        cx.check(Kind::DecodedBytes, 256)?;
        source.drawing_id = namer.next(index, element);
    }
    cx.check(Kind::Work, 0)?;
    drawing.finish_model_bytes(accounted, cx)?;
    Ok(Output {
        drawing,
        zone: projection.zone,
        sources,
    })
}

fn validate_rules(options: Options<'_>, cx: &mut Cx) -> Result<(), Failure> {
    if options.fallback_layer.is_some_and(str::is_empty) {
        return Err(Failure::Invalid("layer"));
    }
    let mut keys = std::collections::BTreeSet::new();
    for rule in options.layers {
        cx.check(
            Kind::Work,
            (rule.kind.len() as u64 + rule.code.map_or(0, str::len) as u64 + 1).saturating_mul(64),
        )?;
        cx.check(Kind::DecodedBytes, 128)?;
        if rule.kind.is_empty() || rule.layer.is_empty() || !keys.insert((rule.kind, rule.code)) {
            return Err(Failure::Invalid("layer_rule"));
        }
    }
    Ok(())
}

fn layer<'a>(
    feature: &Feature,
    options: Options<'a>,
    cx: &mut Cx,
) -> Result<Option<&'a str>, Failure> {
    let mut generic = None;
    for rule in options.layers {
        cx.check(
            Kind::Work,
            (rule.kind.len() + rule.code.map_or(0, str::len) + 1) as u64,
        )?;
        if rule.kind != feature.kind {
            continue;
        }
        if let Some(code) = rule.code {
            if Some(code) == feature.code.as_deref() {
                return Ok(Some(rule.layer));
            }
        } else {
            generic = Some(rule.layer);
        }
    }
    Ok(generic.or(options.fallback_layer))
}

fn reason(error: &Failure) -> &str {
    match error {
        Failure::Invalid(reason) | Failure::Unsupported(reason) => reason,
        _ => "geometry",
    }
}

type DrawShapes = Vec<(d::Shape<ModelPoint>, Option<u64>)>;
fn shapes(
    form: &Form,
    projection: Projection,
    space: SpaceId,
    cx: &mut Cx,
) -> Result<DrawShapes, Failure> {
    let mut out = Vec::new();
    let mut append = |shape, ring, cx: &mut Cx| {
        visual::reserve(&mut out, 1, 512, cx)?;
        out.push((shape, ring));
        Ok::<_, Failure>(())
    };
    match form {
        Form::Point(location) => {
            let [x, y] = projection.xy(location.position, cx)?;
            cx.check(Kind::Segments, 1)?;
            append(
                d::Shape::Point(d::Point {
                    position: ModelPoint { space, x, y },
                    rotation: 0.0,
                    look: d::PointLook::Document,
                    construction: false,
                }),
                None,
                cx,
            )?;
        }
        Form::Line(points) => append(polyline(points, false, projection, space, cx)?, None, cx)?,
        Form::Area { outer, holes } => {
            for (index, ring) in std::iter::once(outer).chain(holes).enumerate() {
                append(
                    polyline(ring, true, projection, space, cx)?,
                    Some(index as u64),
                    cx,
                )?;
            }
        }
        Form::Solid(_) => return Err(Failure::Unsupported("solid_in_plan")),
    }
    Ok(out)
}

fn polyline(
    points: &[Location],
    closed: bool,
    projection: Projection,
    space: SpaceId,
    cx: &mut Cx,
) -> Result<d::Shape<ModelPoint>, Failure> {
    if points.len() < if closed { 3 } else { 2 } {
        return Err(Failure::Invalid("points"));
    }
    let mut vertices = Vec::new();
    visual::reserve(&mut vertices, points.len(), 32, cx)?;
    cx.check(Kind::Segments, points.len() as u64)?;
    for point in points {
        let [x, y] = projection.xy(point.position, cx)?;
        vertices.push(d::Vertex {
            point: ModelPoint { space, x, y },
            bulge: 0.0,
        });
    }
    if closed && vertices.first() == vertices.last() {
        vertices.pop();
    }
    if vertices.len() < if closed { 3 } else { 2 } {
        return Err(Failure::Invalid("points"));
    }
    Ok(d::Shape::Polyline(d::Polyline {
        vertices,
        closed,
        width: None,
        vertex_widths: Vec::new(),
        line_type_generation: false,
    }))
}
