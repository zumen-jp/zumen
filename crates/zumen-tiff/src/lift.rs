//! `TiffFile` から `Drawing2D`・`GeoModel`・`PointCloud` への持ち上げ。

use std::borrow::Cow;

use zumen_base::{Diagnostic, Kind, Ref, codes};
use zumen_crs::{Axes, GeoPoint, MapPoint};
use zumen_drawing::{
    Draft, Drawing2D, Element, Field, FieldValue, Inherit, Layer, LayerId, LayerState, ModelPoint,
    ModelSpace, Polyline, Position as ExtPosition, Resource, Shape, SpaceId, Style, Underlay,
    UnderlayFrame, UnderlayKind, Unit, Vertex,
};
use zumen_geo::{
    Attribute as GeoAttribute, Feature, Form, GeoModel, Geometry, Location, Position as GeoPos,
    Value as GeoValue,
};
use zumen_pointcloud::{Octree, PointCloud, Points};
use zumen_sdk::{Cx, Error};

use crate::geotiff::norm_zero;
use crate::ifd::{DecodedIfd, PagePixels, ResolutionUnit};
use crate::raster::read_dem_value;
use crate::{MAX_ALLOC, TiffFile, stopped};

const LAYER_IMAGE: LayerId = LayerId(0);
const LAYER_BOUNDS: LayerId = LayerId(1);

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

pub(crate) fn lift_drawing(file: &TiffFile, cx: &mut Cx) -> Result<Drawing2D, Error> {
    let mut spaces = Vec::with_capacity(file.pages.len());
    let mut drafts = Vec::new();

    for (idx, page) in file.pages.iter().enumerate() {
        cx.check(Kind::Work, 1)
            .map_err(|s| stopped(cx, s, Some(page.info.source.clone())))?;

        let space_id = SpaceId(u32::try_from(idx).unwrap_or(u32::MAX));
        let (unit, corners) = page_drawing_frame(page);

        let mut elements = Vec::new();
        if matches!(page.data, PagePixels::Native(_)) {
            cx.check(Kind::Elements, 2)
                .map_err(|s| stopped(cx, s, Some(page.info.source.clone())))?;
            cx.check(Kind::Segments, 4)
                .map_err(|s| stopped(cx, s, Some(page.info.source.clone())))?;

            let [bl, br, tr, tl] = corners;
            let pt = |(x, y): (f64, f64)| ModelPoint {
                space: space_id,
                x: norm_zero(x),
                y: norm_zero(y),
            };

            elements.push(Element {
                source: page.info.source.clone(),
                layer: LAYER_IMAGE,
                style: default_style(),
                shape: Shape::Underlay(Underlay {
                    kind: UnderlayKind::Image,
                    resource: Resource {
                        path: ".".to_string(),
                        definition: page.info.source.clone(),
                    },
                    page: Some((idx + 1).to_string()),
                    frame: UnderlayFrame::Pixels {
                        size: [f64::from(page.info.width), f64::from(page.info.height)],
                        origin: pt(bl),
                        right: pt(br),
                        top: pt(tl),
                    },
                    clip: None,
                    visible: true,
                }),
            });

            elements.push(Element {
                source: page.info.source.clone(),
                layer: LAYER_BOUNDS,
                style: default_style(),
                shape: Shape::Polyline(Polyline {
                    vertices: vec![
                        Vertex {
                            point: pt(bl),
                            bulge: 0.0,
                        },
                        Vertex {
                            point: pt(br),
                            bulge: 0.0,
                        },
                        Vertex {
                            point: pt(tr),
                            bulge: 0.0,
                        },
                        Vertex {
                            point: pt(tl),
                            bulge: 0.0,
                        },
                    ],
                    closed: true,
                    width: None,
                    vertex_widths: Vec::new(),
                    line_type_generation: false,
                }),
            });

            let mut fields = vec![
                Field {
                    key: Cow::Borrowed("page"),
                    code: None,
                    value: FieldValue::Integer((idx + 1) as i64),
                },
                Field {
                    key: Cow::Borrowed("width"),
                    code: None,
                    value: FieldValue::Integer(i64::from(page.info.width)),
                },
                Field {
                    key: Cow::Borrowed("height"),
                    code: None,
                    value: FieldValue::Integer(i64::from(page.info.height)),
                },
                Field {
                    key: Cow::Borrowed("bits_per_sample"),
                    code: None,
                    value: FieldValue::Integer(i64::from(page.info.bits_per_sample)),
                },
                Field {
                    key: Cow::Borrowed("samples_per_pixel"),
                    code: None,
                    value: FieldValue::Integer(i64::from(page.info.samples_per_pixel)),
                },
            ];
            if let Some(crs) = page.info.geo.as_ref().and_then(|g| g.crs) {
                fields.push(Field {
                    key: Cow::Borrowed("epsg"),
                    code: None,
                    value: FieldValue::Integer(i64::from(crs.epsg())),
                });
            }
            drafts.push(Draft {
                namespace: Cow::Borrowed("tiff.page"),
                name: "metadata".to_string(),
                version: None,
                schema: 1,
                at: ExtPosition::Model {
                    space: space_id,
                    index: 0,
                },
                fields,
                source: page.info.source.clone(),
            });
        }

        let order = (0..elements.len()).collect();
        spaces.push(ModelSpace {
            id: space_id,
            unit,
            axes: Axes::EastNorth,
            elements,
            order,
        });
    }

    let mut drawing = Drawing2D {
        spaces,
        layers: vec![
            make_layer(LAYER_IMAGE, "画像"),
            make_layer(LAYER_BOUNDS, "境界"),
        ],
        ..Drawing2D::default()
    };
    drawing
        .settle_extensions(drafts, cx)
        .map_err(|s| stopped(cx, s, None))?;
    Ok(drawing)
}

fn page_drawing_frame(page: &DecodedIfd) -> (Unit, [(f64, f64); 4]) {
    let w = page.info.width;
    let h = page.info.height;
    if let Some(geo) = &page.info.geo
        && let Some(crs) = geo.crs
        && crs.zone().is_some()
        && let Some(tf) = geo.transform
        && let Some(corners) = tf.footprint_corners(w, h, geo.raster_type)
    {
        return (Unit::Meter, corners);
    }

    // 非 GeoTIFF（または経緯度画像）は解像度タグがあれば mm 実寸にし、なければピクセル座標にする
    if let (Some(xr), Some(yr)) = (page.info.x_resolution, page.info.y_resolution) {
        let factor_mm = match page.info.resolution_unit {
            ResolutionUnit::Inch => Some(25.4),
            ResolutionUnit::Centimeter => Some(10.0),
            ResolutionUnit::None => None,
        };
        if let Some(mm_per_unit) = factor_mm {
            let w_mm = f64::from(w) * mm_per_unit / xr;
            let h_mm = f64::from(h) * mm_per_unit / yr;
            if w_mm.is_finite() && h_mm.is_finite() && w_mm > 0.0 && h_mm > 0.0 {
                return (
                    Unit::Millimeter,
                    [(0.0, 0.0), (w_mm, 0.0), (w_mm, h_mm), (0.0, h_mm)],
                );
            }
        }
    }

    let wf = f64::from(w);
    let hf = f64::from(h);
    (Unit::Unknown, [(0.0, 0.0), (wf, 0.0), (wf, hf), (0.0, hf)])
}

pub(crate) fn lift_geo(file: &TiffFile, cx: &mut Cx) -> Result<GeoModel, Error> {
    let Some(first) = file.pages.first() else {
        return Ok(GeoModel {
            crs: None,
            height: None,
            features: Vec::new(),
        });
    };

    cx.check(Kind::Work, 1)
        .map_err(|s| stopped(cx, s, Some(first.info.source.clone())))?;

    let crs = first.info.geo.as_ref().and_then(|g| g.crs);
    let height = first.info.geo.as_ref().and_then(|g| g.vertical_crs);

    let Some(crs_val) = crs else {
        if first
            .info
            .geo
            .as_ref()
            .and_then(|g| g.unsupported_epsg)
            .is_none()
        {
            cx.report(Diagnostic::new(codes::TIFF_CRS_MISSING).at(first.info.source.clone()));
        }
        return Ok(GeoModel {
            crs: None,
            height,
            features: Vec::new(),
        });
    };

    let Some(geo) = &first.info.geo else {
        return Ok(GeoModel {
            crs: Some(crs_val),
            height,
            features: Vec::new(),
        });
    };
    let Some(tf) = geo.transform else {
        cx.report(Diagnostic::new(codes::TIFF_CRS_MISSING).at(first.info.source.clone()));
        return Ok(GeoModel {
            crs: Some(crs_val),
            height,
            features: Vec::new(),
        });
    };
    let Some([bl, br, tr, tl]) =
        tf.footprint_corners(first.info.width, first.info.height, geo.raster_type)
    else {
        return Ok(GeoModel {
            crs: Some(crs_val),
            height,
            features: Vec::new(),
        });
    };

    cx.check(Kind::Elements, 1)
        .map_err(|s| stopped(cx, s, Some(first.info.source.clone())))?;
    cx.check(Kind::Segments, 5)
        .map_err(|s| stopped(cx, s, Some(first.info.source.clone())))?;

    let ring_coords = [bl, br, tr, tl, bl];
    let outer = if crs_val.zone().is_some() {
        ring_coords
            .iter()
            .map(|&(x, y)| Location {
                position: GeoPos::Map(MapPoint {
                    northing: norm_zero(y),
                    easting: norm_zero(x),
                }),
                height: None,
            })
            .collect()
    } else {
        ring_coords
            .iter()
            .map(|&(lon, lat)| Location {
                position: GeoPos::Geo(GeoPoint {
                    latitude: norm_zero(lat),
                    longitude: norm_zero(lon),
                }),
                height: None,
            })
            .collect()
    };

    let attributes = vec![
        GeoAttribute {
            name: "width".to_string(),
            value: GeoValue::Integer(i64::from(first.info.width)),
        },
        GeoAttribute {
            name: "height".to_string(),
            value: GeoValue::Integer(i64::from(first.info.height)),
        },
        GeoAttribute {
            name: "bands".to_string(),
            value: GeoValue::Integer(i64::from(first.info.samples_per_pixel)),
        },
        GeoAttribute {
            name: "bits_per_sample".to_string(),
            value: GeoValue::Integer(i64::from(first.info.bits_per_sample)),
        },
        GeoAttribute {
            name: "compression".to_string(),
            value: GeoValue::Integer(i64::from(first.info.compression)),
        },
        GeoAttribute {
            name: "pages".to_string(),
            value: GeoValue::Integer(file.pages.len() as i64),
        },
    ];

    Ok(GeoModel {
        crs: Some(crs_val),
        height,
        features: vec![Feature {
            id: "page-1".to_string(),
            kind: "raster_footprint".to_string(),
            code: None,
            name: None,
            attributes,
            geometries: vec![Geometry {
                lod: None,
                form: Form::Area {
                    outer,
                    holes: Vec::new(),
                },
            }],
            parent: None,
            source: first.info.source.clone(),
        }],
    })
}

pub(crate) fn lift_pointcloud(file: &TiffFile, cx: &mut Cx) -> Result<PointCloud, Error> {
    let Some(page) = file
        .pages
        .iter()
        .find(|p| p.is_dem_candidate())
        .or_else(|| file.pages.first())
    else {
        return Err(Error::Unsupported);
    };

    let PagePixels::Native(native) = &page.data else {
        cx.report(
            Diagnostic::new(codes::TIFF_GRID_UNSUPPORTED)
                .with("reason", "page-unsupported")
                .at(page.info.source.clone()),
        );
        return Err(Error::Unsupported);
    };

    if !page.is_dem_candidate() {
        cx.report(
            Diagnostic::new(codes::TIFF_GRID_UNSUPPORTED)
                .with("reason", "not-single-band-elevation")
                .at(page.info.source.clone()),
        );
        return Err(Error::Unsupported);
    }

    let Some(geo) = &page.info.geo else {
        cx.report(Diagnostic::new(codes::TIFF_CRS_MISSING).at(page.info.source.clone()));
        return Err(Error::Unsupported);
    };
    let Some(crs) = geo.crs else {
        if geo.unsupported_epsg.is_none() {
            cx.report(Diagnostic::new(codes::TIFF_CRS_MISSING).at(page.info.source.clone()));
        }
        return Err(Error::Unsupported);
    };
    if crs.zone().is_none() {
        cx.report(
            Diagnostic::new(codes::TIFF_GRID_UNSUPPORTED)
                .with("reason", "geographic-crs")
                .at(page.info.source.clone()),
        );
        return Err(Error::Unsupported);
    }
    let Some(tf) = geo.transform else {
        cx.report(
            Diagnostic::new(codes::TIFF_GRID_UNSUPPORTED)
                .with("reason", "missing-transform")
                .at(page.info.source.clone()),
        );
        return Err(Error::Unsupported);
    };

    let w = page.info.width;
    let h = page.info.height;
    let cell_count = u64::from(w).saturating_mul(u64::from(h));
    if cell_count.saturating_mul(24) > MAX_ALLOC {
        cx.report(
            Diagnostic::new(codes::TIFF_GRID_UNSUPPORTED)
                .with("reason", "grid-too-large")
                .at(page.info.source.clone()),
        );
        return Err(Error::Unsupported);
    }

    let init_cap = (cell_count as usize).min(65536);
    let mut northing = Vec::with_capacity(init_cap);
    let mut easting = Vec::with_capacity(init_cap);
    let mut height = Vec::with_capacity(init_cap);

    for row in 0..h {
        cx.check(Kind::Work, u64::from(w))
            .map_err(|s| stopped(cx, s, Some(page.info.source.clone())))?;
        for col in 0..w {
            let Some(elev) = read_dem_value(file.endian, page, native, col, row) else {
                continue;
            };
            let Some((x, y)) = tf.cell_center(col, row, geo.raster_type) else {
                continue;
            };
            cx.check(Kind::Elements, 1)
                .map_err(|s| stopped(cx, s, Some(page.info.source.clone())))?;
            cx.check(Kind::DecodedBytes, 24)
                .map_err(|s| stopped(cx, s, Some(page.info.source.clone())))?;
            northing.push(norm_zero(y));
            easting.push(norm_zero(x));
            height.push(norm_zero(elev));
        }
    }

    cx.check(
        Kind::DecodedBytes,
        (northing.len() as u64).saturating_mul(16),
    )
    .map_err(|s| stopped(cx, s, Some(page.info.source.clone())))?;
    let points = Points {
        northing,
        easting,
        height,
        intensity: None,
        classification: None,
        color: None,
        returns: None,
        time: None,
    };
    let octree =
        Octree::build(&points, cx).map_err(|s| stopped(cx, s, Some(page.info.source.clone())))?;

    Ok(PointCloud {
        crs: Some(crs),
        vertical: geo.vertical_crs,
        points,
        octree,
        source: page.info.source.clone(),
    })
}
