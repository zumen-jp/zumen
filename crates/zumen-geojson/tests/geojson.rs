//! GeoJSON の読み書きと地理の領域モデル変換のテスト。

#![cfg(test)]

use zumen_base::{Budget, CancelToken, Cx, Diagnostics, Kind, Limit, Limits, Ref, codes};
use zumen_crs::{Crs, GeoPoint, MapPoint, Zone};
use zumen_geo::{Attribute, Feature, Form, GeoModel, Geometry, Location, Position, Value};
use zumen_geojson::{GeoJson, lower_to_geojson};
use zumen_geom3d::{Face, Shape, Transform};
use zumen_sdk::{Codec, Error, FileModel, Lift, testkit};

fn run_lift(bytes: &[u8]) -> (Result<GeoModel, Error>, Diagnostics) {
    let mut budget = Budget::default();
    let mut diagnostics = Diagnostics::new();
    let cancel = CancelToken::new();
    let mut cx = Cx::new(&mut budget, &mut diagnostics, &cancel);
    let result = GeoJson::read(bytes, &mut cx).and_then(|file| file.lift(&mut cx));
    (result, diagnostics)
}

fn run_lower(model: &GeoModel) -> (Result<Vec<u8>, Error>, Diagnostics) {
    let mut budget = Budget::default();
    let mut diagnostics = Diagnostics::new();
    let cancel = CancelToken::new();
    let mut cx = Cx::new(&mut budget, &mut diagnostics, &cancel);
    let result = lower_to_geojson(model, &mut cx).map(|l| l.bytes);
    (result, diagnostics)
}

#[test]
fn lifts_all_seven_geometry_types_and_3d_coordinates() {
    let src = r#"{
      "type": "FeatureCollection",
      "features": [
        {
          "type": "Feature",
          "id": "pt",
          "properties": {"name": "点", "count": 42, "ratio": 1.5, "ok": true},
          "geometry": {"type": "Point", "coordinates": [139.7, 35.6, 15.25, 999]}
        },
        {
          "type": "Feature",
          "id": 10,
          "properties": null,
          "geometry": {"type": "MultiPoint", "coordinates": [[139.0, 35.0], [140.0, 36.0]]}
        },
        {
          "type": "Feature",
          "properties": {"nested": {"a": "x", "b": -3}},
          "geometry": {"type": "LineString", "coordinates": [[139.0, 35.0], [139.5, 35.5]]}
        },
        {
          "type": "Feature",
          "properties": {},
          "geometry": {
            "type": "MultiLineString",
            "coordinates": [[[139.0, 35.0], [139.1, 35.1]], [[139.2, 35.2], [139.3, 35.3]]]
          }
        },
        {
          "type": "Feature",
          "properties": {},
          "geometry": {
            "type": "Polygon",
            "coordinates": [
              [[139.0, 35.0], [140.0, 35.0], [140.0, 36.0], [139.0, 36.0], [139.0, 35.0]],
              [[139.2, 35.2], [139.2, 35.8], [139.8, 35.8], [139.8, 35.2], [139.2, 35.2]]
            ]
          }
        },
        {
          "type": "Feature",
          "properties": {},
          "geometry": {
            "type": "MultiPolygon",
            "coordinates": [
              [[[139.0, 35.0], [139.1, 35.0], [139.1, 35.1], [139.0, 35.0]]]
            ]
          }
        },
        {
          "type": "Feature",
          "properties": {},
          "geometry": {
            "type": "GeometryCollection",
            "geometries": [
              {"type": "Point", "coordinates": [139.0, 35.0]},
              {"type": "LineString", "coordinates": [[139.0, 35.0], [139.1, 35.1]]}
            ]
          }
        }
      ]
    }"#
    .as_bytes();

    let (res, diags) = run_lift(src);
    assert!(diags.is_empty(), "{diags:?}");
    let model = res.unwrap();
    assert_eq!(model.crs, Some(Crs::Wgs84));
    assert_eq!(model.height, None);
    assert_eq!(model.features.len(), 7);

    let f0 = &model.features[0];
    assert_eq!(f0.id, "pt");
    assert_eq!(f0.name.as_deref(), Some("点"));
    assert_eq!(
        f0.geometries[0].form,
        Form::Point(Location {
            position: Position::Geo(GeoPoint {
                latitude: 35.6,
                longitude: 139.7,
            }),
            height: Some(15.25),
        })
    );

    assert_eq!(model.features[1].id, "10");
    assert_eq!(model.features[1].geometries.len(), 2);

    assert_eq!(model.features[2].id, "Feature/1");
    assert_eq!(
        model.features[2].attributes,
        vec![
            Attribute {
                name: "nested/a".to_string(),
                value: Value::Text("x".to_string()),
            },
            Attribute {
                name: "nested/b".to_string(),
                value: Value::Integer(-3),
            },
        ]
    );

    assert_eq!(model.features[3].geometries.len(), 2);
    if let Form::Area { outer, holes } = &model.features[4].geometries[0].form {
        assert_eq!(outer.len(), 4);
        assert_eq!(holes.len(), 1);
        assert_eq!(holes[0].len(), 4);
    } else {
        panic!("expected Area");
    }
    assert_eq!(model.features[5].geometries.len(), 1);
    assert_eq!(model.features[6].geometries.len(), 2);
}

#[test]
fn supports_bare_geometry_and_utf8_bom_and_file_records() {
    let mut bytes = b"\xEF\xBB\xBF".to_vec();
    bytes.extend_from_slice(br#"{"type":"Point","coordinates":[139.5,35.5]}"#);

    let mut budget = Budget::default();
    let mut diags = Diagnostics::new();
    let cancel = CancelToken::new();
    let mut cx = Cx::new(&mut budget, &mut diags, &cancel);
    let file = GeoJson::read(&bytes, &mut cx).unwrap();
    assert_eq!(file.record_count(), 1);
    let rec = file.record(0).unwrap();
    assert_eq!(rec.kind, b"Point");
    assert_eq!(rec.bytes.unwrap().start, 3);

    let model = file.lift(&mut cx).unwrap();
    assert_eq!(model.features.len(), 1);
    assert_eq!(model.features[0].id, "Point/1");
    assert_eq!(model.features[0].kind, "Point");
}

#[test]
fn reports_diagnostics_for_crs_null_properties_and_invalid_geometries() {
    let src = br#"{
      "type": "FeatureCollection",
      "crs": {"type": "name"},
      "features": [
        {"type": "Point", "coordinates": [139.0, 35.0]},
        {
          "type": "Feature",
          "properties": {"nil": null, "arr": [1, 2], "dup": "old", "dup": "new"},
          "geometry": {
            "type": "GeometryCollection",
            "geometries": [
              {"type": "Point", "coordinates": [139.0, 95.0]},
              {"type": "LineString", "coordinates": [[139.0, 35.0]]},
              {"type": "Polygon", "coordinates": [[[139.0, 35.0], [140.0, 35.0], [140.0, 36.0]]]},
              {"type": "Polygon", "coordinates": [[[139.0, 35.0], [140.0, 35.0], [140.0, 36.0], [139.0, 36.0]]]}
            ]
          }
        }
      ]
    }"#;

    let (res, diags) = run_lift(src);
    let model = res.unwrap();
    assert_eq!(model.features.len(), 1);
    assert_eq!(
        model.features[0].attributes,
        vec![Attribute {
            name: "dup".to_string(),
            value: Value::Text("new".to_string()),
        }]
    );

    let codes: Vec<_> = diags.iter().map(|d| d.code()).collect();
    assert!(codes.contains(&codes::GEOJSON_CRS_DROPPED));
    assert!(codes.contains(&codes::GEOJSON_TYPE_UNSUPPORTED));
    assert!(codes.contains(&codes::GEOJSON_PROPERTY_DROPPED));
    assert!(codes.contains(&codes::GEOJSON_GEOMETRY_INVALID));
}

#[test]
fn rejects_non_geojson_and_broken_json() {
    for bad in [
        &b""[..],
        &b"{"[..],
        &br#"{"type":"Topology"}"#[..],
        &br#"{"type":"FeatureCollection","features":{}}"#[..],
        &br#""hello""#[..],
        &br#"{"type":"Point","coordinates":[139,35]} trailing"#[..],
        &br#"{"type":"Feature","properties":{"bad":"\uD800"}}"#[..],
    ] {
        let (res, diags) = run_lift(bad);
        assert!(res.is_err(), "expected failure for {:?}", bad);
        assert_eq!(
            diags.iter().next().unwrap().code(),
            codes::GEOJSON_FILE_INVALID
        );
    }
}

#[test]
fn enforces_right_hand_rule_and_approximates_plane_and_solid() {
    // 時計回りの外周と反時計回りの穴を渡し、RFC 7946 3.1.6 の向きに直されることを確かめる
    let cw_outer = vec![
        Location {
            position: Position::Geo(GeoPoint {
                latitude: 35.0,
                longitude: 139.0,
            }),
            height: None,
        },
        Location {
            position: Position::Geo(GeoPoint {
                latitude: 36.0,
                longitude: 139.0,
            }),
            height: None,
        },
        Location {
            position: Position::Geo(GeoPoint {
                latitude: 36.0,
                longitude: 140.0,
            }),
            height: None,
        },
        Location {
            position: Position::Geo(GeoPoint {
                latitude: 35.0,
                longitude: 140.0,
            }),
            height: None,
        },
    ];
    let ccw_hole = vec![
        Location {
            position: Position::Geo(GeoPoint {
                latitude: 35.2,
                longitude: 139.2,
            }),
            height: None,
        },
        Location {
            position: Position::Geo(GeoPoint {
                latitude: 35.2,
                longitude: 139.8,
            }),
            height: None,
        },
        Location {
            position: Position::Geo(GeoPoint {
                latitude: 35.8,
                longitude: 139.8,
            }),
            height: None,
        },
        Location {
            position: Position::Geo(GeoPoint {
                latitude: 35.8,
                longitude: 139.2,
            }),
            height: None,
        },
    ];

    let solid = Shape::Mapped {
        transform: Transform::IDENTITY,
        shape: Box::new(Shape::FaceSet {
            faces: vec![Face {
                // GeoModel::Form::Solid の地理空間は [緯度, 経度, 標高]
                outer: vec![
                    [35.0, 139.0, 10.0],
                    [35.0, 139.1, 10.0],
                    [35.1, 139.1, 10.0],
                ],
                holes: Vec::new(),
            }],
            closed: false,
        }),
    };

    let model = GeoModel {
        crs: Some(Crs::Wgs84),
        height: None,
        features: vec![
            Feature {
                id: "poly".to_string(),
                kind: "bldg:Building".to_string(),
                code: Some("101".to_string()),
                name: Some("建物A".to_string()),
                attributes: Vec::new(),
                geometries: vec![Geometry {
                    lod: None,
                    form: Form::Area {
                        outer: cw_outer,
                        holes: vec![ccw_hole],
                    },
                }],
                parent: None,
                source: Ref::default(),
            },
            Feature {
                id: "roof".to_string(),
                kind: "Feature".to_string(),
                code: None,
                name: None,
                attributes: Vec::new(),
                geometries: vec![Geometry {
                    lod: Some(2),
                    form: Form::Solid(solid),
                }],
                parent: None,
                source: Ref::default(),
            },
        ],
    };

    let (bytes, diags) = run_lower(&model);
    let bytes = bytes.unwrap();
    assert_eq!(diags.len(), 1);
    assert_eq!(
        diags.iter().next().unwrap().code(),
        codes::GEOJSON_SOLID_APPROXIMATED
    );

    let (reopened, diags2) = run_lift(&bytes);
    assert!(diags2.is_empty());
    let reopened = reopened.unwrap();
    assert_eq!(reopened.features.len(), 2);
    if let Form::Area { outer, holes } = &reopened.features[0].geometries[0].form {
        assert_eq!(outer.len(), 4);
        assert_eq!(holes[0].len(), 4);
        // 外周は反時計回り（符号つき 2 倍面積 > 0）、穴は時計回り（< 0）
        assert!(ring_area2(outer) > 0.0);
        assert!(ring_area2(&holes[0]) < 0.0);
    } else {
        panic!("expected Area");
    }

    // 平面直角座標系 IX 系の原点 (0, 0) は (36°N, 139°50'E)
    let plane_model = GeoModel {
        crs: Some(Crs::Plane(Zone::new(9).unwrap())),
        height: None,
        features: vec![Feature {
            id: "origin".to_string(),
            kind: "Feature".to_string(),
            code: None,
            name: None,
            attributes: Vec::new(),
            geometries: vec![Geometry {
                lod: None,
                form: Form::Point(Location {
                    position: Position::Map(MapPoint {
                        northing: 0.0,
                        easting: 0.0,
                    }),
                    height: None,
                }),
            }],
            parent: None,
            source: Ref::default(),
        }],
    };
    let (plane_bytes, plane_diags) = run_lower(&plane_model);
    let plane_bytes = plane_bytes.unwrap();
    assert_eq!(
        plane_diags.iter().next().unwrap().code(),
        codes::GEOJSON_DATUM_APPROXIMATED
    );
    let (plane_lifted, _) = run_lift(&plane_bytes);
    let plane_lifted = plane_lifted.unwrap();
    if let Form::Point(loc) = plane_lifted.features[0].geometries[0].form {
        let Position::Geo(pt) = loc.position else {
            panic!("expected Geo");
        };
        assert!((pt.latitude - 36.0).abs() < 1e-9);
        assert!((pt.longitude - (139.0 + 50.0 / 60.0)).abs() < 1e-9);
    } else {
        panic!("expected Point");
    }
}

fn ring_area2(ring: &[Location]) -> f64 {
    let mut sum = 0.0;
    for pair in ring.windows(2) {
        if let [
            Location {
                position: Position::Geo(cur),
                ..
            },
            Location {
                position: Position::Geo(next),
                ..
            },
        ] = pair
        {
            sum += cur.longitude * next.latitude - next.longitude * cur.latitude;
        }
    }
    sum
}

#[test]
fn respects_budgets_and_determinism() {
    let sample = br#"{"type":"FeatureCollection","features":[{"type":"Feature","id":"a","properties":{"k":"v"},"geometry":{"type":"Point","coordinates":[139.7,35.6]}},{"type":"Feature","id":"b","properties":{"k":"w"},"geometry":{"type":"Point","coordinates":[139.8,35.7]}},{"type":"Feature","id":"c","properties":{"k":"z"},"geometry":{"type":"Point","coordinates":[139.9,35.8]}},{"type":"Feature","id":"d","properties":{"k":"q"},"geometry":{"type":"Point","coordinates":[140.0,35.9]}}]}"#;

    // L1 契約テスト（往復・レコード非重複・決定性・予算停止の一致・ID の一貫性）
    testkit::roundtrip::<GeoJson>(sample).unwrap();
    testkit::records_in_order::<GeoJson>(sample).unwrap();
    testkit::deterministic::<GeoJson>(sample).unwrap();
    testkit::stops_at_same_place::<GeoJson>(sample).unwrap();
    testkit::ids_match(sample, "a\nb\nc\nd\n", |src| {
        let (res, _) = run_lift(src);
        res.map_err(|e| format!("{e:?}"))
            .map(|m| m.features.into_iter().map(|f| f.id).collect())
    })
    .unwrap();

    testkit::survives_with(sample, |src, cx| {
        let file = GeoJson::read(src, cx)?;
        let model: GeoModel = file.lift(cx)?;
        lower_to_geojson(&model, cx)
    })
    .unwrap();

    // 6 種すべての予算単位と CancelToken で停止時に診断が必ず出ることを確かめる
    for (kind, lim) in [
        (Kind::InputBytes, 10),
        (Kind::DecodedBytes, 40),
        (Kind::Records, 2),
        (Kind::Elements, 2),
        (Kind::Depth, 1),
        (Kind::Work, 3),
    ] {
        let limits = Limits::default().with(kind, Limit::Finite(lim));
        let mut budget = Budget::new(limits);
        let mut diags = Diagnostics::new();
        let cancel = CancelToken::new();
        let mut cx = Cx::new(&mut budget, &mut diags, &cancel);
        let res = GeoJson::read(sample, &mut cx).and_then(|f| f.lift(&mut cx));
        assert!(
            matches!(res, Err(Error::Stopped(_))),
            "expected Stopped for {kind:?}"
        );
        assert!(
            !diags.is_empty(),
            "expected budget diagnostic for {kind:?}, got empty diagnostics"
        );
    }

    let mut budget = Budget::default();
    let mut diags = Diagnostics::new();
    let cancel = CancelToken::new();
    cancel.cancel();
    let mut cx = Cx::new(&mut budget, &mut diags, &cancel);
    let res = GeoJson::read(sample, &mut cx);
    assert!(matches!(res, Err(Error::Stopped(_))));
    assert!(!diags.is_empty());

    // 書き出し側も DecodedBytes 超過時に診断つきで止まること
    let (lifted, _) = run_lift(sample);
    let model = lifted.unwrap();
    let limits = Limits::default().with(Kind::DecodedBytes, Limit::Finite(24));
    let mut budget = Budget::new(limits);
    let mut diags = Diagnostics::new();
    let cancel = CancelToken::new();
    let mut cx = Cx::new(&mut budget, &mut diags, &cancel);
    let res = lower_to_geojson(&model, &mut cx);
    assert!(matches!(res, Err(Error::Stopped(_))));
    assert!(!diags.is_empty());
}

#[test]
fn handles_vertical_walls_invalid_holes_non_finite_attrs_and_single_crs_diag() {
    // 1. 単体 Point / 単体 Feature の旧 crs メンバーは 1 回だけ報告されること
    let single_pt_crs = br#"{"type":"Point","crs":{"type":"name"},"coordinates":[-0.0,35.0]}"#;
    let (pt_model, pt_diags) = run_lift(single_pt_crs);
    let pt_model = pt_model.unwrap();
    let crs_count = pt_diags
        .iter()
        .filter(|d| d.code() == codes::GEOJSON_CRS_DROPPED)
        .count();
    assert_eq!(crs_count, 1);
    if let Form::Point(loc) = pt_model.features[0].geometries[0].form
        && let Position::Geo(geo) = loc.position
    {
        assert!(!geo.longitude.is_sign_negative());
    }

    // 2. 穴が不正な Polygon は、穴だけ消した中実ポリゴンにせず Polygon 全体を捨てること
    let bad_hole = br#"{
      "type": "Polygon",
      "coordinates": [
        [[139.0, 35.0], [140.0, 35.0], [140.0, 36.0], [139.0, 36.0], [139.0, 35.0]],
        [[139.2, 35.2], [139.8, 35.2], [139.8, 35.8]]
      ]
    }"#;
    let (bad_hole_model, bad_hole_diags) = run_lift(bad_hole);
    let bad_hole_model = bad_hole_model.unwrap();
    assert!(bad_hole_model.features[0].geometries.is_empty());
    assert!(
        bad_hole_diags
            .iter()
            .any(|d| d.code() == codes::GEOJSON_GEOMETRY_INVALID)
    );

    // 3. 鉛直な 4 壁面＋底面＋上面を持つ閉じた直方体 Solid は、鉛直壁を省いて 2 面の GeometryCollection になり、
    //    i64::MAX は整数のまま保たれ、NaN 実数属性は geojson.attribute.dropped で省かれること
    let floor = Face {
        outer: vec![
            [35.0, 139.0, 0.0],
            [35.0, 139.1, 0.0],
            [35.1, 139.1, 0.0],
            [35.1, 139.0, 0.0],
        ],
        holes: Vec::new(),
    };
    let roof = Face {
        outer: vec![
            [35.0, 139.0, 10.0],
            [35.0, 139.1, 10.0],
            [35.1, 139.1, 10.0],
            [35.1, 139.0, 10.0],
        ],
        holes: Vec::new(),
    };
    let wall = Face {
        outer: vec![
            [35.0, 139.0, 0.0],
            [35.0, 139.1, 0.0],
            [35.0, 139.1, 10.0],
            [35.0, 139.0, 10.0],
        ],
        holes: Vec::new(),
    };
    let box_model = GeoModel {
        crs: Some(Crs::Wgs84),
        height: None,
        features: vec![Feature {
            id: "box1".to_string(),
            kind: "Feature".to_string(),
            code: None,
            name: None,
            attributes: vec![
                Attribute {
                    name: "big_id".to_string(),
                    value: Value::Integer(i64::MAX),
                },
                Attribute {
                    name: "bad_num".to_string(),
                    value: Value::Real(f64::NAN),
                },
            ],
            geometries: vec![Geometry {
                lod: Some(1),
                form: Form::Solid(Shape::FaceSet {
                    faces: vec![floor, roof, wall],
                    closed: true,
                }),
            }],
            parent: None,
            source: Ref::default(),
        }],
    };
    let (out_bytes, out_diags) = run_lower(&box_model);
    let out_bytes = out_bytes.unwrap();
    let out_codes: Vec<_> = out_diags.iter().map(|d| d.code()).collect();
    assert!(out_codes.contains(&codes::GEOJSON_SOLID_APPROXIMATED));
    assert!(out_codes.contains(&codes::GEOJSON_ATTRIBUTE_DROPPED));

    let (reopened, reread_diags) = run_lift(&out_bytes);
    assert!(
        reread_diags.is_empty(),
        "re-reading lowered GeoJSON should emit zero diagnostics, got {:?}",
        reread_diags.iter().collect::<Vec<_>>()
    );
    let reopened = reopened.unwrap();
    assert_eq!(reopened.features[0].geometries.len(), 2);
    assert_eq!(
        reopened.features[0].attributes,
        vec![Attribute {
            name: "big_id".to_string(),
            value: Value::Integer(i64::MAX),
        }]
    );
}
