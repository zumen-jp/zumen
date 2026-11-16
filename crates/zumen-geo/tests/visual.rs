//! 合成の同じ地物について、図面と ZMS3 の軸・単位・穴・出所・停止を照合する。
#![cfg(all(feature = "drawing", feature = "scene3d"))]
#![allow(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    clippy::panic,
    reason = "既知の合成入力の検査"
)]
#[cfg(test)]
use zumen_base::codes;
use zumen_base::{Budget, CancelToken, Cx, Diagnostics, Kind, Limit, Limits, Ref};
use zumen_crs::{Crs, GeoPoint, MapPoint, VerticalCrs, Zone};
#[cfg(test)]
use zumen_geo::Selection;
use zumen_geo::{Feature, Form, GeoModel, Geometry, Location, Position, drawing, scene3d};
use zumen_geom3d::Failure;
#[cfg(test)]
use zumen_geom3d::{Face, Shape, Transform};

fn context<T>(f: impl FnOnce(&mut Cx) -> T) -> (T, Diagnostics, Budget) {
    let mut budget = Budget::default();
    let mut diagnostics = Diagnostics::new();
    let cancel = CancelToken::new();
    let result = f(&mut Cx::new(&mut budget, &mut diagnostics, &cancel));
    (result, diagnostics, budget)
}
fn location(east: f64, north: f64, height: Option<f64>) -> Location {
    Location {
        position: Position::Map(MapPoint {
            easting: east,
            northing: north,
        }),
        height,
    }
}
fn feature(id: &str, form: Form) -> Feature {
    Feature {
        id: id.into(),
        kind: "test:Area".into(),
        name: Some("面".into()),
        code: Some("10".into()),
        geometries: vec![Geometry { lod: None, form }],
        source: Ref::at_bytes(10, 20),
        ..Feature::default()
    }
}
fn model(features: Vec<Feature>) -> GeoModel {
    GeoModel {
        features,
        crs: Some(Crs::Plane(Zone::new(9).unwrap())),
        height: Some(VerticalCrs::from_epsg(6695).unwrap()),
    }
}
fn area(east: f64, north: f64) -> Form {
    let ring = |points: &[[f64; 2]]| {
        points
            .iter()
            .map(|p| location(east + p[0], north + p[1], Some(12.0)))
            .collect()
    };
    Form::Area {
        outer: ring(&[
            [0.0, 0.0],
            [10.0, 0.0],
            [10.0, 10.0],
            [0.0, 10.0],
            [0.0, 0.0],
        ]),
        holes: vec![ring(&[
            [3.0, 3.0],
            [3.0, 7.0],
            [7.0, 7.0],
            [7.0, 3.0],
            [3.0, 3.0],
        ])],
    }
}
fn plan(model: &GeoModel, cx: &mut Cx) -> Result<drawing::Output, Failure> {
    drawing::build(
        model,
        drawing::Options {
            fallback_layer: Some("map"),
            ..Default::default()
        },
        cx,
    )
}
fn scene(model: &GeoModel, cx: &mut Cx) -> Result<scene3d::Output, Failure> {
    scene3d::build(
        model,
        scene3d::Options {
            tolerance: Some(0.001),
            ..Default::default()
        },
        cx,
    )
}
fn restored(data: &zumen_scene3d::Data, instance: usize) -> Vec<[f64; 3]> {
    let origin = data
        .chunks
        .iter()
        .rev()
        .find(|c| c.instances as usize <= instance)
        .unwrap()
        .origin;
    let item = &data.instances[instance];
    let m = item.transform.map(f64::from);
    data.meshes[item.mesh as usize]
        .vertices
        .iter()
        .map(|p| {
            let [x, y, z] = p.map(f64::from);
            [
                origin[0] + m[0] * x + m[1] * y + m[2] * z + m[3],
                origin[1] + m[4] * x + m[5] * y + m[6] * z + m[7],
                origin[2] + m[8] * x + m[9] * y + m[10] * z + m[11],
            ]
        })
        .collect()
}

#[test]
fn the_same_area_keeps_east_north_meters_holes_and_source_ids() {
    for (east, north) in [(100.0, 200.0), (1_000_000_000.0, -1_000_000_000.0)] {
        let model = model(vec![feature("area/元", area(east, north))]);
        let before = model.clone();
        let (drawing, _, _) = context(|cx| plan(&model, cx).unwrap());
        let (scene, _, _) = context(|cx| scene(&model, cx).unwrap());
        assert_eq!(model, before);
        assert_eq!(drawing.drawing.spaces[0].unit, zumen_drawing::Unit::Meter);
        assert_eq!(
            drawing.drawing.spaces[0].axes,
            zumen_drawing::Axes::EastNorth
        );
        assert_eq!(drawing.sources.len(), 2);
        assert_eq!(
            drawing.sources.iter().map(|p| p.ring).collect::<Vec<_>>(),
            vec![Some(0), Some(1)]
        );
        let data = &scene.scene.data;
        assert_eq!(data.elements.len(), 1);
        assert_eq!(
            (
                &*data.elements[0].id,
                &*data.elements[0].class,
                &*data.elements[0].name
            ),
            ("area/元", "test:Area", "面")
        );
        assert_eq!(data.elements[0].storey, None);
        let positions = restored(data, 0);
        for (element, source) in drawing.drawing.spaces[0]
            .elements
            .iter()
            .zip(&drawing.sources)
        {
            assert_eq!(source.feature, "area/元");
            assert_eq!(element.source.id(), Some("area/元"));
            assert_eq!(element.source.bytes(), model.features[0].source.bytes());
            let zumen_drawing::Shape::Polyline(line) = &element.shape else {
                panic!("polyline")
            };
            assert!(line.closed);
            assert_eq!(line.vertices.len(), 4);
            for vertex in &line.vertices {
                assert!(positions.contains(&[vertex.point.x, vertex.point.y, 12.0]));
            }
        }
        let mesh = &data.meshes[data.instances[0].mesh as usize];
        let area: f64 = mesh
            .triangles
            .iter()
            .map(|t| {
                let [a, b, c] = t.map(|i| positions[i as usize]);
                zumen_geom3d::length(zumen_geom3d::cross(
                    zumen_geom3d::sub(b, a),
                    zumen_geom3d::sub(c, a),
                )) / 2.0
            })
            .sum();
        assert!((area - 84.0).abs() < 1e-9, "{area}");
        let (_, _, _) = context(|cx| {
            let bytes = zumen_scene3d::encode(data, cx).unwrap();
            zumen_scene3d::read(&bytes, cx).unwrap();
        });
    }
}

#[test]
fn layer_vocabulary_and_feature_selection_preserve_the_original_ids() {
    let model = model(vec![
        feature("first", Form::Point(location(10.0, 20.0, None))),
        feature(
            "second",
            Form::Line(vec![location(0.0, 1.0, None), location(2.0, 3.0, None)]),
        ),
    ]);
    let rules = [
        drawing::LayerRule {
            kind: "test:Area",
            code: None,
            layer: "generic",
        },
        drawing::LayerRule {
            kind: "test:Area",
            code: Some("10"),
            layer: "specific",
        },
    ];
    let ids = vec!["second".to_owned()];
    let (out, diag, _) = context(|cx| {
        drawing::build(
            &model,
            drawing::Options {
                layers: &rules,
                selection: Selection {
                    ids: Some(&ids),
                    ..Default::default()
                },
                ..Default::default()
            },
            cx,
        )
        .unwrap()
    });
    assert!(diag.is_empty());
    assert_eq!(out.drawing.layers[0].name, "specific");
    assert_eq!(out.sources[0].feature, "second");
    let (_, diag, _) = context(|cx| drawing::build(&model, Default::default(), cx).unwrap());
    assert!(
        diag.iter()
            .any(|d| d.code() == codes::GEO_GEOMETRY_UNSUPPORTED && d.count() == 2)
    );
}

#[test]
fn the_geographic_origin_has_known_axes_and_reports_an_assumed_zone() {
    for number in 1..=19 {
        let zone = Zone::new(number).unwrap();
        let origin = zone.origin();
        let mut model = model(vec![feature(
            "origin",
            Form::Point(Location {
                position: Position::Geo(origin),
                height: None,
            }),
        )]);
        model.crs = Some(Crs::Geographic);
        let (out, diag, _) = context(|cx| plan(&model, cx).unwrap());
        assert_eq!(out.zone, zone);
        assert!(diag.iter().any(|d| d.code() == codes::GEO_ZONE_ASSUMED));
        let zumen_drawing::Shape::Point(point) = &out.drawing.spaces[0].elements[0].shape else {
            panic!("point")
        };
        assert!(point.position.x.abs() < 1e-6 && point.position.y.abs() < 1e-6);
        let (_, diag, _) = context(|cx| {
            drawing::build(
                &model,
                drawing::Options {
                    zone: Some(zone),
                    fallback_layer: Some("map"),
                    ..Default::default()
                },
                cx,
            )
            .unwrap()
        });
        assert!(diag.is_empty());
    }
}

#[test]
fn mapped_faces_are_projected_before_meshes_and_one_lod_is_selected() {
    let face = Shape::FaceSet {
        faces: vec![Face {
            outer: vec![
                [0.0, 0.0, 1.0],
                [4.0, 0.0, 1.0],
                [4.0, 4.0, 1.0],
                [0.0, 4.0, 1.0],
            ],
            holes: vec![],
        }],
        closed: false,
    };
    let mapped = Shape::Mapped {
        transform: Transform::translation([100.0, 200.0, 3.0]),
        shape: Box::new(face),
    };
    let mut f = feature("mapped", Form::Solid(mapped));
    f.geometries[0].lod = Some(2);
    f.geometries.push(Geometry {
        lod: Some(1),
        form: area(0.0, 0.0),
    });
    let model = model(vec![f]);
    let (out, _, _) = context(|cx| scene(&model, cx).unwrap());
    assert_eq!(out.lod, Some(2));
    assert_eq!(out.scene.data.instances.len(), 1);
    assert_eq!(
        out.scene.extent,
        Some([[100.0, 200.0, 4.0], [104.0, 204.0, 4.0]])
    );
    let (out, _, _) = context(|cx| {
        scene3d::build(
            &model,
            scene3d::Options {
                region: Some([[0.0, 0.0, 0.0], [10.0, 10.0, 10.0]]),
                ..Default::default()
            },
            cx,
        )
        .unwrap()
    });
    assert!(out.scene.data.elements.is_empty());
}

#[test]
fn missing_heights_and_unsupported_shapes_keep_feature_rows_and_locations() {
    let model = model(vec![
        feature(
            "line",
            Form::Line(vec![
                location(0.0, 0.0, Some(0.0)),
                location(10.0, 0.0, None),
            ]),
        ),
        feature("point", Form::Point(location(0.0, 0.0, Some(3.0)))),
    ]);
    let (out, diag, _) = context(|cx| scene(&model, cx).unwrap());
    assert_eq!(out.scene.data.elements.len(), 2);
    assert!(out.scene.data.instances.is_empty());
    let mut ids: Vec<_> = diag
        .iter()
        .flat_map(|d| d.places())
        .filter_map(Ref::id)
        .collect();
    ids.sort();
    assert_eq!(ids, vec!["line", "point"]);
    assert!(
        diag.iter()
            .all(|d| d.code() == codes::GEO_GEOMETRY_UNSUPPORTED)
    );
}

fn multipart() -> Feature {
    let mut f = feature("multipart", area(100.0, 200.0));
    f.geometries.push(Geometry {
        lod: None,
        form: area(1000.0, 200.0),
    });
    f
}

#[test]
fn one_feature_keeps_all_its_meshes_in_one_chunk_and_precision_failure_is_atomic() {
    let mut model = model(vec![multipart()]);
    let (out, _, _) = context(|cx| {
        scene3d::build(
            &model,
            scene3d::Options {
                region: Some([[100.0, 200.0, 12.0], [101.0, 201.0, 12.0]]),
                tolerance: Some(0.001),
                ..Default::default()
            },
            cx,
        )
        .unwrap()
    });
    let data = &out.scene.data;
    assert_eq!(
        (data.elements.len(), data.chunks.len(), data.meshes.len()),
        (1, 1, 2)
    );
    assert_eq!(data.instances.len(), 2);
    assert!(data.instances.iter().all(|i| i.element == 0));
    assert!(restored(data, 0).contains(&[100.0, 200.0, 12.0]));
    assert!(restored(data, 1).contains(&[1000.0, 200.0, 12.0]));
    context(|cx| {
        let bytes = zumen_scene3d::encode(data, cx).unwrap();
        zumen_scene3d::read(&bytes, cx).unwrap();
    });

    model.features[0].geometries[1].form = area(1_000_000_000.1, 200.0);
    let (out, diag, _) = context(|cx| scene(&model, cx).unwrap());
    assert_eq!(out.scene.data.elements[0].id, "multipart");
    assert!(out.scene.data.meshes.is_empty());
    assert!(out.scene.data.instances.is_empty());
    assert!(out.scene.data.chunks.is_empty());
    assert!(out.scene.data.colors.is_empty());
    assert_eq!(out.scene.extent, None);
    assert!(
        diag.iter()
            .any(|d| d.code() == codes::GEO_GEOMETRY_UNSUPPORTED
                && d.places().iter().any(|p| p.id() == Some("multipart")))
    );
}

#[test]
fn invalid_holes_are_not_dropped_and_duplicate_ids_are_rejected() {
    let mut model = model(vec![feature("area", area(0.0, 0.0))]);
    if let Form::Area { holes, .. } = &mut model.features[0].geometries[0].form {
        holes[0].truncate(2);
    }
    let (out, diag, _) = context(|cx| plan(&model, cx).unwrap());
    assert!(out.drawing.spaces[0].elements.is_empty());
    assert!(out.sources.is_empty());
    assert!(
        diag.iter()
            .any(|d| d.code() == codes::GEO_GEOMETRY_UNSUPPORTED)
    );
    model.features.push(feature("area", area(20.0, 20.0)));
    assert!(matches!(
        context(|cx| plan(&model, cx)).0,
        Err(Failure::Invalid("feature_id"))
    ));
    assert!(matches!(
        context(|cx| scene(&model, cx)).0,
        Err(Failure::Invalid("feature_id"))
    ));
}

#[test]
fn budgets_and_cancellation_do_not_return_a_partial_drawing_or_scene() {
    let model = model(vec![feature("area", area(0.0, 0.0))]);
    for kind in [
        Kind::Work,
        Kind::Elements,
        Kind::DecodedBytes,
        Kind::Segments,
    ] {
        for mode in [false, true] {
            let mut budget = Budget::new(Limits::default().with(kind, Limit::Finite(0)));
            let mut diagnostics = Diagnostics::new();
            let cancel = CancelToken::new();
            let mut cx = Cx::new(&mut budget, &mut diagnostics, &cancel);
            let error = if mode {
                scene(&model, &mut cx).unwrap_err()
            } else {
                plan(&model, &mut cx).unwrap_err()
            };
            assert!(
                matches!(error, Failure::Stopped(_)),
                "{kind:?} {mode} {error:?}"
            );
        }
    }
    let cancel = CancelToken::new();
    cancel.cancel();
    let mut budget = Budget::default();
    let mut diagnostics = Diagnostics::new();
    assert!(matches!(
        scene(&model, &mut Cx::new(&mut budget, &mut diagnostics, &cancel)),
        Err(Failure::Stopped(_))
    ));
}

#[test]
fn different_datums_are_not_assumed_to_be_jgd2011() {
    for crs in [
        None,
        Some(Crs::Wgs84),
        Some(Crs::Tokyo(Zone::new(9).unwrap())),
        Some(Crs::Jgd2000(Zone::new(9).unwrap())),
    ] {
        let mut model = model(vec![feature(
            "point",
            Form::Point(Location {
                position: Position::Geo(GeoPoint {
                    latitude: 36.0,
                    longitude: 140.0,
                }),
                height: None,
            }),
        )]);
        model.crs = crs;
        assert!(context(|cx| plan(&model, cx)).0.is_err());
    }
}

/// 点の値・出所・診断・停止を比較する固定入力。実行環境による文字列や時刻は含めない。
pub fn snapshot() -> Vec<u8> {
    let plane = model(vec![feature(
        "area",
        area(1_000_000_000.0, -1_000_000_000.0),
    )]);
    let mut geo = model(vec![feature("geo", area(0.0, 0.0))]);
    geo.crs = Some(Crs::Geographic);
    if let Form::Area { outer, holes } = &mut geo.features[0].geometries[0].form {
        for ring in std::iter::once(outer).chain(holes) {
            for p in ring {
                let Position::Map(map) = p.position else {
                    panic!("map")
                };
                p.position = Position::Geo(GeoPoint {
                    latitude: 36.0 + map.northing / 100_000.0,
                    longitude: 139.0 + 50.0 / 60.0 + map.easting / 100_000.0,
                });
            }
        }
    }
    let invalid = model(vec![feature(
        "height",
        Form::Line(vec![
            location(0.0, 0.0, None),
            location(10.0, 0.0, Some(1.0)),
        ]),
    )]);
    let mut out = b"zumen-geo visual 1\n".to_vec();
    for model in [plane, geo, invalid, model(vec![multipart()])] {
        for three in [false, true] {
            let run = |limits, cancelled| {
                let mut budget = Budget::new(limits);
                let mut diagnostics = Diagnostics::new();
                let cancel = CancelToken::new();
                if cancelled {
                    cancel.cancel();
                }
                let mut cx = Cx::new(&mut budget, &mut diagnostics, &cancel);
                let result = if three {
                    scene(&model, &mut cx).and_then(|scene| {
                        let mut bytes = format!(
                            "{:?} {:?} {:?} {:?}\n",
                            scene.zone, scene.height, scene.lod, scene.scene.extent
                        )
                        .into_bytes();
                        bytes.extend(zumen_scene3d::encode(&scene.scene.data, &mut cx).map_err(
                            |error| match error {
                                zumen_scene3d::Error::Stopped(stop) => Failure::Stopped(stop),
                                zumen_scene3d::Error::Invalid(reason) => Failure::Invalid(reason),
                            },
                        )?);
                        Ok(bytes)
                    })
                } else {
                    plan(&model, &mut cx).map(|drawing| format!("{drawing:?}").into_bytes())
                };
                (result, diagnostics, budget)
            };
            let (baseline, diag, budget) = run(Limits::default(), false);
            let baseline = baseline.unwrap();
            out.extend(format!("{three} {} {diag:?}\n", baseline.len()).bytes());
            out.extend(&baseline);
            for kind in Kind::ALL {
                let used = budget.used(kind);
                out.extend(format!("\n{kind:?}:{used}\n").bytes());
                if kind == Kind::Depth {
                    for limit in [0, 1, 8] {
                        let (result, diag, _) =
                            run(Limits::default().with(kind, Limit::Finite(limit)), false);
                        out.extend(
                            format!("depth {limit}: {:?} {diag:?}\n", result.map(|_| ())).bytes(),
                        );
                    }
                    continue;
                }
                let (exact, exact_diag, _) =
                    run(Limits::default().with(kind, Limit::Finite(used)), false);
                assert_eq!(exact.unwrap(), baseline);
                assert_eq!(format!("{exact_diag:?}"), format!("{diag:?}"));
                if used > 0 {
                    let (result, diag, _) =
                        run(Limits::default().with(kind, Limit::Finite(used - 1)), false);
                    assert!(matches!(result, Err(Failure::Stopped(_))));
                    out.extend(
                        format!("below {kind:?}: {:?} {diag:?}\n", result.map(|_| ())).bytes(),
                    );
                }
            }
            let (cancelled, diag, _) = run(Limits::default(), true);
            assert!(matches!(cancelled, Err(Failure::Stopped(_))));
            out.extend(format!("cancelled: {:?} {diag:?}\n", cancelled.map(|_| ())).bytes());
        }
    }
    out
}

#[test]
fn the_complete_record_and_budget_boundaries_are_repeatable() {
    assert_eq!(snapshot(), snapshot());
}
