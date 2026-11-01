//! 土木の領域モデル（決定 0230）から、線形の平面図・縦断図・横断図を図面モデルに出す（決定 0231）。
//!
//! 平面図は 1 つ目の空間（x を東、y を北）、縦断図は 2 つ目の空間（x を追加距離、y を標高）、横断図は 3 つ目から
//! の空間（x を離れ、y を標高）に置き、図ごとの用紙にビューで置く。縦断図のビューは縦横で尺度が違うので、
//! 縦断図の文字と欄は用紙の要素にする。

use zumen_base::{Cx, Diagnostic, Diagnostics, Kind, Ref, Stop, codes};
use zumen_civil::station::Notation;
use zumen_civil::{
    Alignment, CivilModel, CrossSection, HorizontalKind, SurfaceRole, Tin, TransitionCurve,
    Vertical, VerticalKind,
};
use zumen_crs::MapPoint;
use zumen_drawing::{
    Affine, Drawing2D, Edge, Element, Height, HeightMeaning, Inherit, Layer, LayerId, Length,
    ModelLength, ModelPoint, ModelSpace, PaperLength, PaperPoint, Placement, Polyline, Shape,
    Sheet, SheetId, SheetItem, SpaceId, Style, Text, Unit, Vertex, View,
};
use zumen_drawing::{Horizontal as TextHorizontal, Vertical as TextVertical};
use zumen_geom2d::Ramp;

/// 図の尺度と書き方。尺度は分母で持つ
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Options {
    /// 平面図の尺度
    pub plan: f64,
    /// 縦断図の横の尺度
    pub horizontal: f64,
    /// 縦断図の縦の尺度
    pub vertical: f64,
    /// 横断図の尺度
    pub cross_section: f64,
    /// 文字の高さ（mm）
    pub text_height: f64,
    /// 曲線を折れ線にするときの、用紙の上の許容差（mm）
    pub tolerance: f64,
    /// 測点の表記
    pub notation: Notation,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            plan: 1000.0,
            horizontal: 1000.0,
            vertical: 100.0,
            cross_section: 100.0,
            text_height: 2.5,
            tolerance: 0.05,
            notation: Notation::default(),
        }
    }
}

/// レイヤ
const ALIGNMENT: LayerId = LayerId(0);
const STATION: LayerId = LayerId(1);
const SURFACE: LayerId = LayerId(2);
const DESIGN: LayerId = LayerId(3);
const GROUND: LayerId = LayerId(4);
const FRAME: LayerId = LayerId(5);
const LAYERS: [(LayerId, &str); 6] = [
    (ALIGNMENT, "線形"),
    (STATION, "測点"),
    (SURFACE, "地表面"),
    (DESIGN, "計画線"),
    (GROUND, "地盤線"),
    (FRAME, "欄"),
];

/// 色・線種・線幅は、レイヤから継ぐ
const STYLE: Style = Style {
    color: Inherit::Layer,
    line_type: Inherit::Layer,
    line_type_scale: 1.0,
    line_weight: Some(Inherit::Layer),
    transparency: Inherit::Layer,
};

const PLAN: SpaceId = SpaceId(0);
const PROFILE: SpaceId = SpaceId(1);
/// 用紙の余白（mm）
const MARGIN: f64 = 10.0;
/// 縦断図の欄の 1 行の高さと、欄の見出しの幅（mm）
const ROW: f64 = 8.0;
const HEADING: f64 = 30.0;

/// 線形 `alignment` の平面図・縦断図・横断図を作る。平面図には `model` の地表面も描く。縦断線形がなければ縦断図を、
/// 横断がなければ横断図を作らない。描けない区間は除き、理由を `cx` に知らせる
pub fn drawing(
    model: &CivilModel,
    alignment: &Alignment,
    options: &Options,
    cx: &mut Cx<'_>,
) -> Result<Drawing2D, Stop> {
    let mut drawing = Drawing2D {
        layers: LAYERS
            .iter()
            .map(|(id, name)| Layer {
                id: *id,
                name: (*name).to_string(),
                state: None,
                style: None,
                follows_placement: false,
                description: None,
                source: Ref::default(),
            })
            .collect(),
        ..Drawing2D::default()
    };
    let plan = plan(model, alignment, options, cx)?;
    push(&mut drawing, plan);
    if !alignment.vertical.is_empty() {
        let profile = profile(alignment, options, cx)?;
        push(&mut drawing, profile);
    }
    for (index, section) in alignment.cross_sections.iter().enumerate() {
        let id = u32::try_from(index).map_or(u32::MAX, |index| index.saturating_add(2));
        let sheet = cross_section(section, SpaceId(id), SheetId(id), options, cx)?;
        push(&mut drawing, sheet);
    }
    Ok(drawing)
}

/// 1 つの図。空間と、それを置いた用紙
struct Figure {
    space: ModelSpace,
    sheet: Sheet,
}

fn push(drawing: &mut Drawing2D, figure: Figure) {
    drawing.spaces.push(figure.space);
    drawing.sheets.push(figure.sheet);
}

fn element<P>(layer: LayerId, shape: Shape<P>, source: &Ref) -> Element<P> {
    Element {
        source: source.clone(),
        layer,
        style: STYLE,
        shape,
    }
}

fn space(id: SpaceId, elements: Vec<Element<ModelPoint>>) -> ModelSpace {
    ModelSpace {
        id,
        unit: Unit::Meter,
        axes: Default::default(),
        order: (0..elements.len()).collect(),
        elements,
    }
}

/// 用紙。要素を先に、ビューを後に描く
fn sheet(
    id: SheetId,
    name: String,
    size: [f64; 2],
    elements: Vec<Element<PaperPoint>>,
    view: View,
) -> Sheet {
    let mut order: Vec<SheetItem> = (0..elements.len()).map(SheetItem::Element).collect();
    order.push(SheetItem::View(0));
    Sheet {
        id,
        name: Some(name),
        size: Some([PaperLength(size[0]), PaperLength(size[1])]),
        elements,
        views: vec![view],
        order,
        source: Ref::default(),
    }
}

/// 実寸の範囲を、余白を空けて用紙に置くビューと、用紙の大きさ。`scale` は横と縦の尺度の分母
fn fit(
    space: SpaceId,
    sheet: SheetId,
    extent: [f64; 4],
    scale: [f64; 2],
    lift: [f64; 2],
) -> (View, [f64; 2]) {
    let [x0, y0, x1, y1] = extent;
    let (sx, sy) = (1000.0 / scale[0], 1000.0 / scale[1]);
    let view = View {
        space,
        sheet,
        transform: Affine {
            m: [sx, 0.0, lift[0] - x0 * sx, 0.0, sy, lift[1] - y0 * sy],
        },
        clip: Vec::new(),
        hidden_layers: Vec::new(),
        source: Ref::default(),
    };
    let size = [
        lift[0] + (x1 - x0) * sx + MARGIN,
        lift[1] + (y1 - y0) * sy + MARGIN,
    ];
    (view, size)
}

/// 点の並びの範囲。点がなければ原点の 1 m 四方
fn extent(points: impl IntoIterator<Item = [f64; 2]>) -> [f64; 4] {
    let mut out = [
        f64::INFINITY,
        f64::INFINITY,
        f64::NEG_INFINITY,
        f64::NEG_INFINITY,
    ];
    for [x, y] in points {
        if x.is_finite() && y.is_finite() {
            out = [out[0].min(x), out[1].min(y), out[2].max(x), out[3].max(y)];
        }
    }
    if out[0] > out[2] {
        return [0.0, 0.0, 1.0, 1.0];
    }
    out
}

fn text<P>(
    content: String,
    point: P,
    height: Length,
    rotation: f64,
    horizontal: TextHorizontal,
) -> Shape<P> {
    Shape::Text(Text {
        content,
        height: Height {
            value: height,
            meaning: HeightMeaning::Frame,
        },
        placement: Placement::Anchor {
            point,
            horizontal,
            vertical: TextVertical::Middle,
        },
        rotation,
        width_factor: 1.0,
        oblique: 0.0,
        backward: false,
        upside_down: false,
        style: String::new(),
    })
}

fn polyline<P>(points: Vec<P>, closed: bool) -> Shape<P> {
    Shape::Polyline(Polyline {
        vertices: points
            .into_iter()
            .map(|point| Vertex { point, bulge: 0.0 })
            .collect(),
        closed,
        width: None,
        vertex_widths: Vec::new(),
        line_type_generation: false,
    })
}

fn real(space: SpaceId, value: f64) -> Length {
    Length::Model(ModelLength { space, value })
}

/// 地図の点を平面図の点にする（x を東、y を北）
fn map(point: [f64; 2]) -> ModelPoint {
    ModelPoint {
        space: PLAN,
        x: point[1],
        y: point[0],
    }
}

/// 弦の誤差が `tolerance` より小さくなる刻みの数。`curvature` はいちばん大きい曲率
/// 縦断の円弧とクロソイドの区間を、曲率 `curvature` で決めた刻みで点にする（決定 0236）。高さを計算できなければ、
/// 理由を知らせて `None`
fn curve(
    index: usize,
    segment: &Vertical,
    curvature: f64,
    tolerance: f64,
    cx: &mut Cx<'_>,
) -> Result<Option<Vec<[f64; 2]>>, Stop> {
    let steps = steps(segment.length, curvature, tolerance);
    cx.check(Kind::Elements, steps as u64)?;
    let mut points = Vec::with_capacity(steps + 1);
    for step in 0..=steps {
        let u = segment.length * step as f64 / steps as f64;
        match zumen_civil::elevation(index, segment, u) {
            Ok((height, _)) => points.push([segment.start_distance + u, height]),
            Err(diagnostic) => {
                cx.report(*diagnostic);
                return Ok(None);
            }
        }
    }
    Ok(Some(points))
}

fn steps(length: f64, curvature: f64, tolerance: f64) -> usize {
    let step = if curvature > 0.0 && tolerance > 0.0 {
        libm::sqrt(8.0 * tolerance / curvature)
    } else {
        length
    };
    let steps = libm::ceil(length / step.max(f64::MIN_POSITIVE));
    if steps.is_finite() {
        steps.clamp(1.0, zumen_geom2d::MAX_STEPS as f64) as usize
    } else {
        zumen_geom2d::MAX_STEPS
    }
}

/// 位置を求めるときの、縦断の診断を捨てる文脈。平面図の測点では高さを使わない
fn quiet<T>(cx: &mut Cx<'_>, f: impl FnOnce(&mut Cx<'_>) -> T) -> T {
    let mut diagnostics = Diagnostics::new();
    let mut quiet = Cx::new(cx.budget, &mut diagnostics, cx.cancel);
    f(&mut quiet)
}

fn plan(
    model: &CivilModel,
    alignment: &Alignment,
    options: &Options,
    cx: &mut Cx<'_>,
) -> Result<Figure, Stop> {
    let unit = options.plan / 1000.0;
    let tolerance = options.tolerance * unit;
    let mut elements = Vec::new();
    let mut corners: Vec<[f64; 2]> = Vec::new();
    for (index, segment) in alignment.horizontal.iter().enumerate() {
        cx.check(Kind::Elements, 1)?;
        let curvatures = match zumen_civil::curvature(index, segment) {
            Ok(found) => found,
            Err(diagnostic) => {
                cx.report(*diagnostic);
                continue;
            }
        };
        let start = [segment.start.northing, segment.start.easting];
        let ([k1, k2], ramp) = curvatures;
        let (end, _) = zumen_geom2d::along(
            start,
            segment.direction,
            [k1, k2],
            segment.length,
            ramp,
            segment.length,
        );
        corners.extend([start, end]);
        let shape = if k1 == k2 && k1 == 0.0 {
            Shape::Line {
                start: map(start),
                end: map(end),
            }
        } else if k1 == k2 {
            let radius = 1.0 / k1;
            // 右へ曲がる向きの法線は (−sin, cos)
            let center = [
                start[0] - radius * libm::sin(segment.direction),
                start[1] + radius * libm::cos(segment.direction),
            ];
            let angle = |point: [f64; 2]| {
                libm::atan2(point[0] - center[0], point[1] - center[1]).to_degrees()
            };
            Shape::Curve(Edge::Arc {
                center: map(center),
                radius: real(PLAN, radius.abs()),
                start: angle(start),
                end: angle(end),
                counterclockwise: k1 < 0.0,
            })
        } else if ramp == Some(Ramp::Clothoid)
            && let Some(form) =
                zumen_geom2d::clothoid_origin(start, segment.direction, [k1, k2], segment.length)
        {
            // 地図の点の向き（北から東へ）を、図面の向き（x から反時計回り）にする
            Shape::Curve(Edge::Clothoid {
                origin: map(form.origin),
                parameter: real(PLAN, form.parameter),
                angle: 90.0 - form.angle.to_degrees(),
                counterclockwise: !form.positive,
                start: real(PLAN, form.start),
                end: real(PLAN, form.end),
            })
        } else {
            let steps = steps(segment.length, k1.abs().max(k2.abs()), tolerance);
            cx.check(Kind::Elements, steps as u64)?;
            let points = (0..=steps)
                .map(|step| {
                    let s = segment.length * step as f64 / steps as f64;
                    map(zumen_geom2d::along(
                        start,
                        segment.direction,
                        [k1, k2],
                        segment.length,
                        ramp,
                        s,
                    )
                    .0)
                })
                .collect();
            if let HorizontalKind::Transition { curve, .. } = &segment.kind {
                cx.report(
                    Diagnostic::new(codes::CIVIL_CURVE_APPROXIMATED)
                        .with("index", index as u64)
                        .with("curve", curve_name(curve)),
                );
            }
            polyline(points, false)
        };
        elements.push(element(ALIGNMENT, shape, &segment.source));
    }
    stations(alignment, options, unit, &mut elements, cx)?;
    for tin in &model.surfaces {
        surface(tin, &mut elements, cx)?;
        corners.extend(
            tin.points
                .iter()
                .map(|(point, _)| [point.northing, point.easting]),
        );
    }
    let [n0, e0, n1, e1] = extent(corners);
    let (view, size) = fit(
        PLAN,
        SheetId(0),
        [e0, n0, e1, n1],
        [options.plan; 2],
        [MARGIN; 2],
    );
    Ok(Figure {
        space: space(PLAN, elements),
        sheet: sheet(SheetId(0), "平面図".to_string(), size, Vec::new(), view),
    })
}

fn curve_name(curve: &TransitionCurve) -> String {
    match curve {
        TransitionCurve::Clothoid => "clothoid",
        TransitionCurve::CubicParabola => "cubic_parabola",
        TransitionCurve::SineHalfWave => "sine_half_wave",
        TransitionCurve::Cosine => "cosine",
        TransitionCurve::Bloss => "bloss",
        TransitionCurve::Helmert => "helmert",
        TransitionCurve::Viennese => "viennese",
        TransitionCurve::Other(name) => name,
    }
    .to_string()
}

/// 測点の間隔の倍数の測点の、追加距離と番号。測点の不連続の先も数える
fn whole_stations(
    alignment: &Alignment,
    pitch: f64,
    cx: &mut Cx<'_>,
) -> Result<Vec<(f64, f64)>, Stop> {
    let length = zumen_civil::length(alignment);
    let stations = &alignment.stations;
    let mut bounds = vec![stations.value(0.0), stations.value(length)];
    for at in &stations.breaks {
        bounds.extend([at.back, at.ahead]);
    }
    let low = bounds.iter().copied().fold(f64::INFINITY, f64::min);
    let high = bounds.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let (first, last) = (libm::ceil(low / pitch), libm::floor(high / pitch));
    if !(first.is_finite() && last.is_finite() && pitch > 0.0) || last < first {
        return Ok(Vec::new());
    }
    cx.check(Kind::Elements, (last - first) as u64 + 1)?;
    let mut found = Vec::new();
    let mut number = first;
    while number <= last {
        for distance in stations.distances(number * pitch, length) {
            found.push((distance, number));
        }
        number += 1.0;
    }
    found.sort_by(|a, b| a.0.total_cmp(&b.0));
    Ok(found)
}

/// 平面図の測点。線形に直交する短い線と、番号の文字
fn stations(
    alignment: &Alignment,
    options: &Options,
    unit: f64,
    elements: &mut Vec<Element<ModelPoint>>,
    cx: &mut Cx<'_>,
) -> Result<(), Stop> {
    let half = 2.0 * unit;
    for (distance, number) in whole_stations(alignment, options.notation.pitch, cx)? {
        let Ok(placed) = quiet(cx, |cx| zumen_civil::place(alignment, distance, cx)) else {
            continue;
        };
        let MapPoint { northing, easting } = placed.point;
        let (sin, cos) = (libm::sin(placed.direction), libm::cos(placed.direction));
        // 右の法線は (−sin, cos)
        let at = |k: f64| map([northing - k * sin, easting + k * cos]);
        elements.push(element(
            STATION,
            Shape::Line {
                start: at(-half),
                end: at(half),
            },
            &Ref::default(),
        ));
        // 文字は左へ、線形に直交して書く
        elements.push(element(
            STATION,
            text(
                format!("No.{number}"),
                at(-1.5 * half),
                real(PLAN, options.text_height * unit),
                libm::atan2(sin, -cos).to_degrees(),
                TextHorizontal::Left,
            ),
            &Ref::default(),
        ));
    }
    Ok(())
}

/// TIN の三角形を、閉じた折れ線にする。点の番号が範囲の外の三角形は除いて知らせる
fn surface(
    tin: &Tin,
    elements: &mut Vec<Element<ModelPoint>>,
    cx: &mut Cx<'_>,
) -> Result<(), Stop> {
    cx.check(Kind::Elements, tin.triangles.len() as u64)?;
    for (index, triangle) in tin.triangles.iter().enumerate() {
        let corners: Option<Vec<ModelPoint>> = triangle
            .iter()
            .map(|at| {
                tin.points
                    .get(*at as usize)
                    .map(|(point, _)| map([point.northing, point.easting]))
            })
            .collect();
        match corners {
            Some(corners) => elements.push(element(SURFACE, polyline(corners, true), &tin.source)),
            None => cx.report(
                Diagnostic::new(codes::CIVIL_TRIANGLE_INVALID)
                    .with("surface", tin.id.as_str())
                    .with("triangle", index as u64),
            ),
        }
    }
    Ok(())
}

/// 縦断線形の追加距離での高さ。範囲の外か計算できない区間なら `None`
fn height(vertical: &[Vertical], distance: f64) -> Option<f64> {
    vertical.iter().find_map(|segment| {
        let u = distance - segment.start_distance;
        if !(0.0..=segment.length).contains(&u) {
            return None;
        }
        let (h0, g0) = (segment.start_height, segment.start_gradient);
        match segment.kind {
            VerticalKind::ConstantGradient => Some(h0 + g0 * u),
            VerticalKind::Parabola { end_gradient } => {
                Some(h0 + g0 * u + (end_gradient - g0) / segment.length * u * u / 2.0)
            }
            VerticalKind::CircularArc { .. } | VerticalKind::Clothoid { .. } => None,
        }
    })
}

/// 地盤線。横断の現況の線の、中心（離れ 0）の高さ
fn ground(alignment: &Alignment) -> Vec<(f64, f64)> {
    let mut found: Vec<(f64, f64)> = alignment
        .cross_sections
        .iter()
        .filter_map(|section| {
            let line = section
                .lines
                .iter()
                .find(|line| line.role == SurfaceRole::Existing)?;
            line.points.windows(2).find_map(|pair| {
                let [[o0, h0], [o1, h1]] = pair else {
                    return None;
                };
                let (low, high) = (o0.min(*o1), o0.max(*o1));
                if !(low <= 0.0 && 0.0 <= high) {
                    return None;
                }
                let t = if o1 == o0 { 0.0 } else { -o0 / (o1 - o0) };
                Some((section.distance, h0 + t * (h1 - h0)))
            })
        })
        .collect();
    found.sort_by(|a, b| a.0.total_cmp(&b.0));
    found
}

fn profile(alignment: &Alignment, options: &Options, cx: &mut Cx<'_>) -> Result<Figure, Stop> {
    // 縦断曲線の許容差は、縦の尺度で用紙に写したときの値で決める
    let tolerance = options.tolerance * options.vertical / 1000.0;
    let point = |x: f64, y: f64| ModelPoint {
        space: PROFILE,
        x,
        y,
    };
    let mut elements = Vec::new();
    let mut corners = Vec::new();
    for (index, segment) in alignment.vertical.iter().enumerate() {
        cx.check(Kind::Elements, 1)?;
        let (d0, h0, g0, length) = (
            segment.start_distance,
            segment.start_height,
            segment.start_gradient,
            segment.length,
        );
        if !(length.is_finite() && length > 0.0) {
            cx.report(
                Diagnostic::new(codes::CIVIL_SEGMENT_INVALID)
                    .with("index", index as u64)
                    .with("field", "length"),
            );
            continue;
        }
        let shape = match segment.kind {
            VerticalKind::ConstantGradient => Shape::Line {
                start: point(d0, h0),
                end: point(d0 + length, h0 + g0 * length),
            },
            VerticalKind::Parabola { end_gradient } => {
                let change = (end_gradient - g0) / length;
                let steps = steps(length, change.abs(), tolerance);
                cx.check(Kind::Elements, steps as u64)?;
                let points = (0..=steps)
                    .map(|step| {
                        let u = length * step as f64 / steps as f64;
                        point(d0 + u, h0 + g0 * u + change * u * u / 2.0)
                    })
                    .collect();
                polyline(points, false)
            }
            VerticalKind::CircularArc { radius } => {
                let Some(points) = curve(index, segment, 1.0 / radius.abs(), tolerance, cx)? else {
                    continue;
                };
                corners.extend(points.iter().map(|[d, h]| [*d, *h]));
                polyline(points.iter().map(|[d, h]| point(*d, *h)).collect(), false)
            }
            VerticalKind::Clothoid {
                start_radius,
                end_radius,
            } => {
                let sharpest = [start_radius, end_radius]
                    .iter()
                    .flatten()
                    .map(|radius| 1.0 / radius.abs())
                    .fold(0.0, f64::max);
                let Some(points) = curve(index, segment, sharpest, tolerance, cx)? else {
                    continue;
                };
                corners.extend(points.iter().map(|[d, h]| [*d, *h]));
                polyline(points.iter().map(|[d, h]| point(*d, *h)).collect(), false)
            }
        };
        corners.extend([[d0, h0], [d0 + length, h0 + g0 * length]]);
        elements.push(element(DESIGN, shape, &segment.source));
    }
    let ground = ground(alignment);
    if ground.len() > 1 {
        elements.push(element(
            GROUND,
            polyline(ground.iter().map(|(d, h)| point(*d, *h)).collect(), false),
            &Ref::default(),
        ));
    }
    corners.extend(ground.iter().map(|(d, h)| [*d, *h]));
    let [x0, y0, x1, y1] = extent(corners);
    // 基準の高さ（DL）は、いちばん低い所の 1 m 下の整数
    let datum = libm::floor(y0) - 1.0;
    let lift = [HEADING, 3.0 * ROW];
    let (view, size) = fit(
        PROFILE,
        SheetId(1),
        [x0, datum, x1, y1],
        [options.horizontal, options.vertical],
        lift,
    );
    let paper = |x: f64, y: f64| PaperPoint {
        sheet: SheetId(1),
        x,
        y,
    };
    let place = |distance: f64, height: f64| {
        let (x, y) = view.transform.apply(distance, height);
        paper(x, y)
    };
    let size_text = Length::Paper(PaperLength(options.text_height));
    let mut band = Vec::new();
    let right = size[0] - MARGIN;
    // 欄の横の線と見出し
    for (row, name) in ["測点", "地盤高", "計画高"].iter().enumerate() {
        let y = ROW * row as f64;
        band.push(element(
            FRAME,
            Shape::Line {
                start: paper(MARGIN, y),
                end: paper(right, y),
            },
            &Ref::default(),
        ));
        band.push(element(
            FRAME,
            text(
                (*name).to_string(),
                paper(MARGIN + 1.0, y + ROW / 2.0),
                size_text,
                0.0,
                TextHorizontal::Left,
            ),
            &Ref::default(),
        ));
    }
    band.push(element(
        FRAME,
        Shape::Line {
            start: paper(MARGIN, lift[1]),
            end: paper(right, lift[1]),
        },
        &Ref::default(),
    ));
    band.push(element(
        FRAME,
        text(
            format!("DL={datum:.3}"),
            paper(MARGIN + 1.0, lift[1] + ROW / 2.0),
            size_text,
            0.0,
            TextHorizontal::Left,
        ),
        &Ref::default(),
    ));
    // 測点ごとの縦の線と、測点・地盤高・計画高
    for (distance, number) in whole_stations(alignment, options.notation.pitch, cx)? {
        let planned = height(&alignment.vertical, distance);
        let top = place(distance, planned.unwrap_or(datum));
        let x = top.x;
        band.push(element(
            FRAME,
            Shape::Line {
                start: paper(x, 0.0),
                end: top,
            },
            &Ref::default(),
        ));
        let mut write = |row: usize, content: String| {
            band.push(element(
                FRAME,
                text(
                    content,
                    paper(x + 0.5, ROW * row as f64 + ROW / 2.0),
                    size_text,
                    90.0,
                    TextHorizontal::Center,
                ),
                &Ref::default(),
            ));
        };
        write(0, format!("No.{number}"));
        if let Some(found) = interpolate(&ground, distance) {
            write(1, format!("{found:.3}"));
        }
        if let Some(planned) = planned {
            write(2, format!("{planned:.3}"));
        }
    }
    Ok(Figure {
        space: space(PROFILE, elements),
        sheet: sheet(SheetId(1), "縦断図".to_string(), size, band, view),
    })
}

/// 追加距離の順の点の並びの、`distance` での高さ。範囲の外なら `None`
fn interpolate(points: &[(f64, f64)], distance: f64) -> Option<f64> {
    points.windows(2).find_map(|pair| {
        let [(d0, h0), (d1, h1)] = pair else {
            return None;
        };
        if !(*d0 <= distance && distance <= *d1) {
            return None;
        }
        let t = if d1 == d0 {
            0.0
        } else {
            (distance - d0) / (d1 - d0)
        };
        Some(h0 + t * (h1 - h0))
    })
}

fn cross_section(
    section: &CrossSection,
    id: SpaceId,
    sheet_id: SheetId,
    options: &Options,
    cx: &mut Cx<'_>,
) -> Result<Figure, Stop> {
    let point = |x: f64, y: f64| ModelPoint { space: id, x, y };
    let mut elements = Vec::new();
    let mut corners = Vec::new();
    for line in &section.lines {
        cx.check(Kind::Elements, line.points.len() as u64)?;
        let layer = if line.role == SurfaceRole::Existing {
            GROUND
        } else {
            DESIGN
        };
        corners.extend(line.points.iter().copied());
        elements.push(element(
            layer,
            polyline(
                line.points.iter().map(|[x, y]| point(*x, *y)).collect(),
                false,
            ),
            &section.source,
        ));
    }
    let [x0, y0, x1, y1] = extent(corners);
    // 中心の線
    elements.push(element(
        ALIGNMENT,
        Shape::Line {
            start: point(0.0, y0),
            end: point(0.0, y1),
        },
        &section.source,
    ));
    let lift = [MARGIN, MARGIN + 2.0 * options.text_height];
    let (view, size) = fit(
        id,
        sheet_id,
        [x0.min(0.0), y0, x1.max(0.0), y1],
        [options.cross_section; 2],
        lift,
    );
    let (center, _) = view.transform.apply(0.0, y0);
    let name = section
        .name
        .clone()
        .unwrap_or_else(|| format!("{:.3}", section.distance));
    let label = element(
        FRAME,
        text(
            name.clone(),
            PaperPoint {
                sheet: sheet_id,
                x: center,
                y: MARGIN,
            },
            Length::Paper(PaperLength(options.text_height)),
            0.0,
            TextHorizontal::Center,
        ),
        &section.source,
    );
    Ok(Figure {
        space: space(id, elements),
        sheet: sheet(sheet_id, format!("横断図 {name}"), size, vec![label], view),
    })
}
