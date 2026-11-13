//! SIMA の中間モデル（`SimaData`）から、図面（`Drawing2D`）・土木（`CivilModel`）・地理（`GeoModel`）への変換。

use std::borrow::Cow;
use std::collections::BTreeSet;

use zumen_base::{Diagnostic, Kind, Ref, codes};
use zumen_civil::{
    Alignment, CivilModel, Horizontal as CivilHorizontal, HorizontalKind, Stationing,
    TransitionCurve,
};
use zumen_crs::{Axes, MapPoint, Realization, VerticalCrs};
use zumen_drawing::{
    Draft, Drawing2D, Edge, Element, Field, FieldValue, Height, HeightMeaning, Horizontal, Inherit,
    Layer, LayerId, LayerState, Length, ModelLength, ModelPoint, ModelSpace, Placement, Point,
    PointLook, PointMark, PointSize, PointStyle, Polyline, Position as ExtPosition, Shape, SpaceId,
    Style, Text, Unit, Vertex, Vertical,
};
use zumen_geo::{
    Attribute as GeoAttribute, Feature, Form, GeoModel, Geometry, Location, Position as GeoPos,
    Value as GeoValue,
};
use zumen_sdk::{Cx, Error};

use crate::model::{
    AttributeVal, CURVE_TOLERANCE_M, MapLocation, ParcelKind, PointKind, RouteSegment,
    RouteSegmentKind, SimaData, arc_to_drawing, clothoid_curvatures, norm_zero,
    sample_arc_locations, sample_clothoid_locations,
};
use crate::stopped;

const SPACE_ID: SpaceId = SpaceId(0);
const LAYER_POINTS: LayerId = LayerId(0);
const LAYER_PARCELS: LayerId = LayerId(1);
const LAYER_ROUTES: LayerId = LayerId(2);

fn default_style() -> Style {
    Style {
        color: Inherit::Layer,
        line_type: Inherit::Layer,
        line_type_scale: 1.0,
        line_weight: None,
        transparency: Inherit::Layer,
    }
}

fn make_layer(id: LayerId, name: &str) -> Layer {
    Layer {
        id,
        name: name.to_string(),
        state: Some(LayerState {
            visible: true,
            printable: true,
            editable: true,
            frozen: false,
        }),
        style: None,
        follows_placement: false,
        description: None,
        source: Ref::default(),
    }
}

fn model_pt(mp: MapPoint) -> ModelPoint {
    ModelPoint {
        space: SPACE_ID,
        x: norm_zero(mp.easting),
        y: norm_zero(mp.northing),
    }
}

fn model_len(v: f64) -> Length {
    Length::Model(ModelLength {
        space: SPACE_ID,
        value: norm_zero(v),
    })
}

fn attr_to_field(key: String, val: &AttributeVal) -> Field {
    let value = match val {
        AttributeVal::Text(s) | AttributeVal::Date(s) => FieldValue::Text(s.clone()),
        AttributeVal::Integer(i) => FieldValue::Integer(*i),
        AttributeVal::Real(r) => FieldValue::Real(*r),
    };
    Field {
        key: Cow::Owned(key),
        code: None,
        value,
    }
}

pub(crate) fn lift_drawing(data: &SimaData, cx: &mut Cx) -> Result<Drawing2D, Error> {
    let mut elements: Vec<Element<ModelPoint>> = Vec::new();
    let mut drafts: Vec<Draft> = Vec::new();

    // 文書レベルの拡張（現場名・座標系・現場情報）
    let mut doc_fields = Vec::new();
    if let Some(ref site) = data.site_name {
        doc_fields.push(Field {
            key: Cow::Borrowed("site_name"),
            code: None,
            value: FieldValue::Text(site.clone()),
        });
    }
    if let Some(crs) = data.crs {
        doc_fields.push(Field {
            key: Cow::Borrowed("epsg"),
            code: None,
            value: FieldValue::Integer(i64::from(crs.epsg())),
        });
    }
    for (k, v) in &data.site_attributes {
        doc_fields.push(attr_to_field(k.clone(), v));
    }
    if !doc_fields.is_empty() {
        drafts.push(Draft {
            namespace: Cow::Borrowed("sima.attribute"),
            name: "document".to_string(),
            version: data.version.clone(),
            schema: 1,
            at: ExtPosition::Document,
            fields: doc_fields,
            source: Ref::default(),
        });
    }

    // 1. 測点（点＋ラベル文字）
    for pt in &data.points {
        cx.check(Kind::Work, 1)
            .map_err(|s| stopped(cx, s, Some(pt.source.clone())))?;
        cx.check(Kind::Elements, 1)
            .map_err(|s| stopped(cx, s, Some(pt.source.clone())))?;

        let idx = elements.len();
        elements.push(Element {
            source: pt.source.clone(),
            layer: LAYER_POINTS,
            style: default_style(),
            shape: Shape::Point(Point {
                position: model_pt(pt.position),
                rotation: 0.0,
                look: PointLook::Document,
                construction: false,
            }),
        });

        let mut fields = vec![
            Field {
                key: Cow::Borrowed("number"),
                code: None,
                value: FieldValue::Text(pt.number.clone()),
            },
            Field {
                key: Cow::Borrowed("name"),
                code: None,
                value: FieldValue::Text(pt.name.clone()),
            },
            Field {
                key: Cow::Borrowed("kind"),
                code: None,
                value: FieldValue::Text(pt.kind.geo_kind().to_string()),
            },
        ];
        if let Some(alt) = pt.altitude {
            fields.push(Field {
                key: Cow::Borrowed("altitude"),
                code: None,
                value: FieldValue::Real(alt),
            });
        }
        for (k, v) in &pt.attributes {
            fields.push(attr_to_field(k.clone(), v));
        }
        drafts.push(Draft {
            namespace: Cow::Borrowed("sima.attribute"),
            name: "point".to_string(),
            version: data.version.clone(),
            schema: 1,
            at: ExtPosition::Model {
                space: SPACE_ID,
                index: idx,
            },
            fields,
            source: pt.source.clone(),
        });

        let label = if !pt.name.is_empty() {
            pt.name.as_str()
        } else {
            pt.number.as_str()
        };
        if !label.is_empty() {
            cx.check(Kind::Elements, 1)
                .map_err(|s| stopped(cx, s, Some(pt.source.clone())))?;
            let text_idx = elements.len();
            elements.push(Element {
                source: pt.source.clone(),
                layer: LAYER_POINTS,
                style: default_style(),
                shape: Shape::Text(Text {
                    content: label.to_string(),
                    height: Height {
                        value: model_len(0.8),
                        meaning: HeightMeaning::CapHeight,
                    },
                    placement: Placement::Anchor {
                        point: ModelPoint {
                            space: SPACE_ID,
                            x: norm_zero(pt.position.easting + 0.3),
                            y: norm_zero(pt.position.northing + 0.3),
                        },
                        horizontal: Horizontal::Left,
                        vertical: Vertical::Bottom,
                    },
                    rotation: 0.0,
                    width_factor: 1.0,
                    oblique: 0.0,
                    backward: false,
                    upside_down: false,
                    style: String::new(),
                }),
            });
            drafts.push(Draft {
                namespace: Cow::Borrowed("sima.label"),
                name: "point_label".to_string(),
                version: data.version.clone(),
                schema: 1,
                at: ExtPosition::Model {
                    space: SPACE_ID,
                    index: text_idx,
                },
                fields: Vec::new(),
                source: pt.source.clone(),
            });
        }
    }

    // 2. 画地・開放画地・地番
    for parcel in &data.parcels {
        cx.check(Kind::Work, (parcel.outer.len() as u64).max(1))
            .map_err(|s| stopped(cx, s, Some(parcel.source.clone())))?;

        let first_idx = elements.len();
        if !parcel.curve_segments.is_empty() {
            for seg in &parcel.curve_segments {
                push_segment_element(&mut elements, LAYER_PARCELS, seg, cx)?;
            }
        } else {
            if parcel.ring_curve_approximated {
                cx.report(
                    Diagnostic::new(codes::SIMA_CURVE_APPROXIMATED)
                        .with("kind", "ring_arc")
                        .with("tolerance", CURVE_TOLERANCE_M)
                        .at(parcel.source.clone()),
                );
            }
            cx.check(Kind::Elements, 1)
                .map_err(|s| stopped(cx, s, Some(parcel.source.clone())))?;
            let vertices = parcel
                .outer
                .iter()
                .map(|loc| Vertex {
                    point: model_pt(loc.point),
                    bulge: 0.0,
                })
                .collect();
            elements.push(Element {
                source: parcel.source.clone(),
                layer: LAYER_PARCELS,
                style: default_style(),
                shape: Shape::Polyline(Polyline {
                    vertices,
                    closed: parcel.closed,
                    width: None,
                    vertex_widths: Vec::new(),
                    line_type_generation: false,
                }),
            });
            for hole in &parcel.holes {
                cx.check(Kind::Elements, 1)
                    .map_err(|s| stopped(cx, s, Some(parcel.source.clone())))?;
                let hole_vertices = hole
                    .iter()
                    .map(|loc| Vertex {
                        point: model_pt(loc.point),
                        bulge: 0.0,
                    })
                    .collect();
                elements.push(Element {
                    source: parcel.source.clone(),
                    layer: LAYER_PARCELS,
                    style: default_style(),
                    shape: Shape::Polyline(Polyline {
                        vertices: hole_vertices,
                        closed: true,
                        width: None,
                        vertex_widths: Vec::new(),
                        line_type_generation: false,
                    }),
                });
            }
        }

        if first_idx < elements.len() {
            let mut fields = vec![
                Field {
                    key: Cow::Borrowed("number"),
                    code: None,
                    value: FieldValue::Text(parcel.number.clone()),
                },
                Field {
                    key: Cow::Borrowed("kind"),
                    code: None,
                    value: FieldValue::Text(parcel.kind.geo_kind().to_string()),
                },
            ];
            if let Some(ref name) = parcel.name {
                fields.push(Field {
                    key: Cow::Borrowed("name"),
                    code: None,
                    value: FieldValue::Text(name.clone()),
                });
            }
            for (k, v) in &parcel.attributes {
                fields.push(attr_to_field(k.clone(), v));
            }
            drafts.push(Draft {
                namespace: Cow::Borrowed("sima.attribute"),
                name: "parcel".to_string(),
                version: data.version.clone(),
                schema: 1,
                at: ExtPosition::Model {
                    space: SPACE_ID,
                    index: first_idx,
                },
                fields,
                source: parcel.source.clone(),
            });
        }
    }

    // 3. 路線
    for route in &data.routes {
        let first_idx = elements.len();
        for seg in &route.segments {
            push_segment_element(&mut elements, LAYER_ROUTES, seg, cx)?;
        }
        if first_idx < elements.len() {
            let mut fields = vec![Field {
                key: Cow::Borrowed("number"),
                code: None,
                value: FieldValue::Text(route.number.clone()),
            }];
            if let Some(ref name) = route.name {
                fields.push(Field {
                    key: Cow::Borrowed("name"),
                    code: None,
                    value: FieldValue::Text(name.clone()),
                });
            }
            for (k, v) in &route.attributes {
                fields.push(attr_to_field(k.clone(), v));
            }
            drafts.push(Draft {
                namespace: Cow::Borrowed("sima.attribute"),
                name: "route".to_string(),
                version: data.version.clone(),
                schema: 1,
                at: ExtPosition::Model {
                    space: SPACE_ID,
                    index: first_idx,
                },
                fields,
                source: route.source.clone(),
            });
        }
    }

    let order = (0..elements.len()).collect();
    let mut drawing = Drawing2D {
        spaces: vec![ModelSpace {
            id: SPACE_ID,
            unit: Unit::Meter,
            axes: Axes::EastNorth,
            elements,
            order,
        }],
        layers: vec![
            make_layer(LAYER_POINTS, "測点"),
            make_layer(LAYER_PARCELS, "画地"),
            make_layer(LAYER_ROUTES, "路線"),
        ],
        point_style: Some(PointStyle {
            mark: PointMark::Cross,
            circle: true,
            square: false,
            size: PointSize::Screen(1.5),
        }),
        ..Drawing2D::default()
    };
    drawing
        .settle_extensions(drafts, cx)
        .map_err(|s| stopped(cx, s, None))?;
    Ok(drawing)
}

fn push_segment_element(
    elements: &mut Vec<Element<ModelPoint>>,
    layer: LayerId,
    seg: &RouteSegment,
    cx: &mut Cx,
) -> Result<(), Error> {
    cx.check(Kind::Work, 1)
        .map_err(|s| stopped(cx, s, Some(seg.source.clone())))?;
    cx.check(Kind::Elements, 1)
        .map_err(|s| stopped(cx, s, Some(seg.source.clone())))?;
    let shape = match seg.kind {
        RouteSegmentKind::Line => {
            let end = seg.end.unwrap_or_else(|| MapPoint {
                northing: norm_zero(seg.start.northing + seg.length * libm::cos(seg.direction)),
                easting: norm_zero(seg.start.easting + seg.length * libm::sin(seg.direction)),
            });
            Shape::Line {
                start: model_pt(seg.start),
                end: model_pt(end),
            }
        }
        RouteSegmentKind::Arc { radius } => {
            if let Some(([cx_e, cy_n], r, start_deg, end_deg, ccw, _)) =
                arc_to_drawing(seg.start, seg.direction, seg.length, radius)
            {
                Shape::Curve(Edge::Arc {
                    center: ModelPoint {
                        space: SPACE_ID,
                        x: cx_e,
                        y: cy_n,
                    },
                    radius: model_len(r),
                    start: start_deg,
                    end: end_deg,
                    counterclockwise: ccw,
                })
            } else {
                let end = seg.end.unwrap_or(seg.start);
                Shape::Line {
                    start: model_pt(seg.start),
                    end: model_pt(end),
                }
            }
        }
        RouteSegmentKind::Clothoid {
            parameter,
            start_radius,
            end_radius,
        } => {
            let [k1, k2] = clothoid_curvatures(parameter, seg.length, start_radius, end_radius);
            let work = zumen_geom2d::clothoid_origin_work([k1, k2], seg.length);
            cx.check(Kind::Work, work)
                .map_err(|s| stopped(cx, s, Some(seg.source.clone())))?;
            if let Some(form) = zumen_geom2d::clothoid_origin(
                [seg.start.northing, seg.start.easting],
                seg.direction,
                [k1, k2],
                seg.length,
            ) && form.origin[0].is_finite()
                && form.origin[1].is_finite()
                && form.parameter.is_finite()
                && form.angle.is_finite()
                && form.start.is_finite()
                && form.end.is_finite()
            {
                let [orig_n, orig_e] = form.origin;
                let angle_deg = norm_zero((90.0 - form.angle.to_degrees()).rem_euclid(360.0));
                Shape::Curve(Edge::Clothoid {
                    origin: ModelPoint {
                        space: SPACE_ID,
                        x: norm_zero(orig_e),
                        y: norm_zero(orig_n),
                    },
                    parameter: model_len(norm_zero(form.parameter)),
                    angle: angle_deg,
                    counterclockwise: !form.positive,
                    start: model_len(norm_zero(form.start)),
                    end: model_len(norm_zero(form.end)),
                })
            } else {
                let end = seg.end.unwrap_or(seg.start);
                Shape::Line {
                    start: model_pt(seg.start),
                    end: model_pt(end),
                }
            }
        }
    };
    elements.push(Element {
        source: seg.source.clone(),
        layer,
        style: default_style(),
        shape,
    });
    Ok(())
}

pub(crate) fn lift_civil(data: &SimaData, cx: &mut Cx) -> Result<CivilModel, Error> {
    if data.crs.is_none()
        && !data.crs_reported
        && (!data.points.is_empty() || !data.parcels.is_empty() || !data.routes.is_empty())
    {
        cx.report(Diagnostic::new(codes::SIMA_CRS_MISSING));
    }

    let mut alignments = Vec::new();
    let mut used_ids = BTreeSet::new();

    for (idx, route) in data.routes.iter().enumerate() {
        cx.check(Kind::Work, (route.segments.len() as u64).max(1))
            .map_err(|s| stopped(cx, s, Some(route.source.clone())))?;
        cx.check(Kind::Elements, 1)
            .map_err(|s| stopped(cx, s, Some(route.source.clone())))?;

        let id = unique_id(
            &mut used_ids,
            "alignment",
            route.name.as_deref().or(Some(route.number.as_str())),
            idx + 1,
        );
        let horizontal = route
            .segments
            .iter()
            .map(to_civil_horizontal)
            .collect::<Vec<_>>();
        alignments.push(Alignment {
            id,
            name: route
                .name
                .clone()
                .or_else(|| (!route.number.is_empty()).then(|| route.number.clone())),
            usage: None,
            horizontal,
            vertical: Vec::new(),
            cant: Vec::new(),
            stations: Stationing {
                start: route.start_station,
                breaks: Vec::new(),
                pitch: None,
            },
            cross_sections: Vec::new(),
            source: route.source.clone(),
        });
    }

    // 路線ブロックがなく開放画地がある場合（JPGIS 版の KaihouKakuchi など）も平面線形として参照できるようにする
    if alignments.is_empty() {
        for (idx, parcel) in data.parcels.iter().filter(|p| !p.closed).enumerate() {
            cx.check(Kind::Work, (parcel.outer.len() as u64).max(1))
                .map_err(|s| stopped(cx, s, Some(parcel.source.clone())))?;
            let horizontal = if !parcel.curve_segments.is_empty() {
                parcel
                    .curve_segments
                    .iter()
                    .map(to_civil_horizontal)
                    .collect()
            } else {
                let mut segs = Vec::new();
                for pair in parcel.outer.windows(2) {
                    let (Some(a), Some(b)) = (pair.first(), pair.get(1)) else {
                        continue;
                    };
                    if let Some((direction, length)) =
                        crate::model::bearing_and_distance(a.point, b.point)
                    {
                        segs.push(CivilHorizontal {
                            start: a.point,
                            direction,
                            length,
                            kind: HorizontalKind::Line,
                            source: parcel.source.clone(),
                        });
                    }
                }
                segs
            };
            if horizontal.is_empty() {
                continue;
            }
            cx.check(Kind::Elements, 1)
                .map_err(|s| stopped(cx, s, Some(parcel.source.clone())))?;
            let id = unique_id(
                &mut used_ids,
                "alignment",
                parcel.name.as_deref().or(Some(parcel.number.as_str())),
                idx + 1,
            );
            alignments.push(Alignment {
                id,
                name: parcel.name.clone(),
                usage: None,
                horizontal,
                vertical: Vec::new(),
                cant: Vec::new(),
                stations: Stationing::default(),
                cross_sections: Vec::new(),
                source: parcel.source.clone(),
            });
        }
    }

    Ok(CivilModel {
        alignments,
        surfaces: Vec::new(),
        crs: data.crs,
        height_datum: None,
        realization: data.realization,
    })
}

fn to_civil_horizontal(seg: &RouteSegment) -> CivilHorizontal {
    let kind = match seg.kind {
        RouteSegmentKind::Line => HorizontalKind::Line,
        RouteSegmentKind::Arc { radius } => HorizontalKind::Arc { radius },
        RouteSegmentKind::Clothoid {
            start_radius,
            end_radius,
            ..
        } => HorizontalKind::Transition {
            start_radius,
            end_radius,
            curve: TransitionCurve::Clothoid,
        },
    };
    CivilHorizontal {
        start: seg.start,
        direction: seg.direction,
        length: seg.length,
        kind,
        source: seg.source.clone(),
    }
}

pub(crate) fn lift_geo(data: &SimaData, cx: &mut Cx) -> Result<GeoModel, Error> {
    let has_crs = data.crs.is_some();
    if !has_crs
        && !data.crs_reported
        && (!data.points.is_empty() || !data.parcels.is_empty() || !data.routes.is_empty())
    {
        cx.report(Diagnostic::new(codes::SIMA_CRS_MISSING));
    }

    let height = match data.realization {
        Some(Realization::Survey2011) => Some(VerticalCrs::Jgd2011),
        Some(Realization::Survey2024) => Some(VerticalCrs::Jgd2024),
        None => None,
    };

    let mut features = Vec::new();
    let mut used_ids = BTreeSet::new();

    for (idx, pt) in data.points.iter().enumerate() {
        cx.check(Kind::Work, 1)
            .map_err(|s| stopped(cx, s, Some(pt.source.clone())))?;
        cx.check(Kind::Elements, 1)
            .map_err(|s| stopped(cx, s, Some(pt.source.clone())))?;

        let prefix = match pt.kind {
            PointKind::Sokuten => "Sokuten",
            PointKind::Kijyunten => "Kijyunten",
            PointKind::Suijyunten => "Suijyunten",
        };
        let id = unique_id(
            &mut used_ids,
            prefix,
            pt.id_hint
                .as_deref()
                .or_else(|| (!pt.number.is_empty()).then_some(pt.number.as_str())),
            idx + 1,
        );
        let mut attributes = Vec::new();
        if let Some(alt) = pt.altitude {
            attributes.push(GeoAttribute {
                name: "altitude".to_string(),
                value: GeoValue::Real(alt),
            });
        }
        for (k, v) in &pt.attributes {
            attributes.push(to_geo_attr(k, v));
        }
        let geometries = if has_crs {
            vec![Geometry {
                lod: None,
                form: Form::Point(Location {
                    position: GeoPos::Map(pt.position),
                    height: pt.altitude,
                }),
            }]
        } else {
            Vec::new()
        };
        features.push(Feature {
            id,
            kind: pt.kind.geo_kind().to_string(),
            code: (!pt.number.is_empty()).then(|| pt.number.clone()),
            name: (!pt.name.is_empty()).then(|| pt.name.clone()),
            attributes,
            geometries,
            parent: None,
            source: pt.source.clone(),
        });
    }

    for (idx, parcel) in data.parcels.iter().enumerate() {
        cx.check(Kind::Work, (parcel.outer.len() as u64).max(1))
            .map_err(|s| stopped(cx, s, Some(parcel.source.clone())))?;
        cx.check(Kind::Elements, 1)
            .map_err(|s| stopped(cx, s, Some(parcel.source.clone())))?;

        let prefix = match parcel.kind {
            ParcelKind::Kakuchi => "Kakuchi",
            ParcelKind::KaihouKakuchi => "KaihouKakuchi",
            ParcelKind::Chiban => "Chiban",
        };
        let id = unique_id(
            &mut used_ids,
            prefix,
            parcel
                .id_hint
                .as_deref()
                .or_else(|| (!parcel.number.is_empty()).then_some(parcel.number.as_str())),
            idx + 1,
        );
        let attributes = parcel
            .attributes
            .iter()
            .map(|(k, v)| to_geo_attr(k, v))
            .collect();
        let geometries = if has_crs {
            let to_locs = |locs: &[MapLocation]| -> Vec<Location> {
                locs.iter()
                    .map(|l| Location {
                        position: GeoPos::Map(l.point),
                        height: l.altitude,
                    })
                    .collect()
            };
            let form = if parcel.closed {
                if parcel.ring_curve_approximated {
                    cx.report(
                        Diagnostic::new(codes::SIMA_CURVE_APPROXIMATED)
                            .with("kind", "ring_arc")
                            .with("tolerance", CURVE_TOLERANCE_M)
                            .at(parcel.source.clone()),
                    );
                }
                Form::Area {
                    outer: to_locs(&parcel.outer),
                    holes: parcel.holes.iter().map(|h| to_locs(h)).collect(),
                }
            } else if !parcel.curve_segments.is_empty() {
                let sampled = sample_route_segments_geo(&parcel.curve_segments, cx)?;
                if sampled.len() >= 2 {
                    Form::Line(sampled)
                } else {
                    Form::Line(to_locs(&parcel.outer))
                }
            } else {
                Form::Line(to_locs(&parcel.outer))
            };
            vec![Geometry { lod: None, form }]
        } else {
            Vec::new()
        };
        features.push(Feature {
            id,
            kind: parcel.kind.geo_kind().to_string(),
            code: (!parcel.number.is_empty()).then(|| parcel.number.clone()),
            name: parcel.name.clone(),
            attributes,
            geometries,
            parent: None,
            source: parcel.source.clone(),
        });
    }

    for (idx, route) in data.routes.iter().enumerate() {
        cx.check(Kind::Work, (route.segments.len() as u64).max(1))
            .map_err(|s| stopped(cx, s, Some(route.source.clone())))?;
        cx.check(Kind::Elements, 1)
            .map_err(|s| stopped(cx, s, Some(route.source.clone())))?;

        let id = unique_id(
            &mut used_ids,
            "Rosen",
            (!route.number.is_empty()).then_some(route.number.as_str()),
            idx + 1,
        );
        let attributes = route
            .attributes
            .iter()
            .map(|(k, v)| to_geo_attr(k, v))
            .collect();
        let geometries = if has_crs {
            let line_pts = sample_route_segments_geo(&route.segments, cx)?;
            if line_pts.len() >= 2 {
                vec![Geometry {
                    lod: None,
                    form: Form::Line(line_pts),
                }]
            } else {
                Vec::new()
            }
        } else {
            Vec::new()
        };
        features.push(Feature {
            id,
            kind: "sima:Rosen".to_string(),
            code: (!route.number.is_empty()).then(|| route.number.clone()),
            name: route.name.clone(),
            attributes,
            geometries,
            parent: None,
            source: route.source.clone(),
        });
    }

    Ok(GeoModel {
        crs: data.crs,
        height,
        features,
    })
}

fn sample_route_segments_geo(
    segments: &[RouteSegment],
    cx: &mut Cx,
) -> Result<Vec<Location>, Error> {
    let mut out: Vec<Location> = Vec::new();
    let push_loc = |dst: &mut Vec<Location>, loc: MapLocation| {
        let g = Location {
            position: GeoPos::Map(loc.point),
            height: loc.altitude,
        };
        if dst.last() != Some(&g) {
            dst.push(g);
        }
    };
    for seg in segments {
        match seg.kind {
            RouteSegmentKind::Line => {
                push_loc(
                    &mut out,
                    MapLocation {
                        point: seg.start,
                        altitude: None,
                    },
                );
                let end = seg.end.unwrap_or_else(|| MapPoint {
                    northing: norm_zero(seg.start.northing + seg.length * libm::cos(seg.direction)),
                    easting: norm_zero(seg.start.easting + seg.length * libm::sin(seg.direction)),
                });
                push_loc(
                    &mut out,
                    MapLocation {
                        point: end,
                        altitude: None,
                    },
                );
            }
            RouteSegmentKind::Arc { radius } => {
                cx.report(
                    Diagnostic::new(codes::SIMA_CURVE_APPROXIMATED)
                        .with("kind", "arc")
                        .with("tolerance", CURVE_TOLERANCE_M)
                        .at(seg.source.clone()),
                );
                let locs = sample_arc_locations(
                    seg.start,
                    seg.direction,
                    seg.length,
                    radius,
                    (None, None),
                    &seg.source,
                    cx,
                )?;
                for l in locs {
                    push_loc(&mut out, l);
                }
            }
            RouteSegmentKind::Clothoid {
                parameter,
                start_radius,
                end_radius,
            } => {
                cx.report(
                    Diagnostic::new(codes::SIMA_CURVE_APPROXIMATED)
                        .with("kind", "clothoid")
                        .with("tolerance", CURVE_TOLERANCE_M)
                        .at(seg.source.clone()),
                );
                let locs = sample_clothoid_locations(seg, parameter, start_radius, end_radius, cx)?;
                for l in locs {
                    push_loc(&mut out, l);
                }
            }
        }
    }
    Ok(out)
}

fn to_geo_attr(key: &str, val: &AttributeVal) -> GeoAttribute {
    let value = match val {
        AttributeVal::Text(s) => GeoValue::Text(s.clone()),
        AttributeVal::Integer(i) => GeoValue::Integer(*i),
        AttributeVal::Real(r) => GeoValue::Real(*r),
        AttributeVal::Date(d) => GeoValue::Date(d.clone()),
    };
    GeoAttribute {
        name: key.to_string(),
        value,
    }
}

fn unique_id(used: &mut BTreeSet<String>, prefix: &str, hint: Option<&str>, seq: usize) -> String {
    if let Some(raw) = hint {
        let trimmed = raw.trim();
        if !trimmed.is_empty() {
            let candidate = format!("{prefix}/{trimmed}");
            if used.insert(candidate.clone()) {
                return candidate;
            }
        }
    }
    let mut n = seq;
    loop {
        let candidate = format!("{prefix}/{n}");
        if used.insert(candidate.clone()) {
            return candidate;
        }
        n += 1;
    }
}
