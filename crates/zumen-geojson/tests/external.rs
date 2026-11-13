//! GeoJSON の書き出しと読み取りを Shapely 2.2.0（GEOS）と照らす（`#876`）。

#![cfg(test)]

use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};

use zumen_base::{Budget, CancelToken, Cx, Diagnostics, Ref};
use zumen_crs::{Crs, GeoPoint, MapPoint, Zone};
use zumen_geo::{Attribute, Feature, Form, GeoModel, Geometry, Location, Position, Value};
use zumen_geojson::{lower_to_geojson, GeoJson};
use zumen_geom3d::{Face, Shape};
use zumen_sdk::{Codec, Lift};

#[allow(clippy::disallowed_methods, clippy::print_stderr)]
fn oracle() -> Option<(PathBuf, PathBuf)> {
    let explicit = std::env::var_os("ZUMEN_SHAPELY_PYTHON");
    let python = explicit
        .clone()
        .map(PathBuf::from)
        .unwrap_or_else(|| "python".into());
    let script =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../xtask/verify/shapely/verify.py");
    let result = Command::new(&python)
        .arg("-I")
        .arg(&script)
        .arg("--check")
        .output();
    let missing = match &result {
        Ok(output) if output.status.success() => {
            assert_eq!(
                String::from_utf8_lossy(&output.stdout).trim(),
                "shapely\t2.2.0"
            );
            return Some((python, script));
        }
        Ok(output) => output.status.code() == Some(77),
        Err(error) => error.kind() == std::io::ErrorKind::NotFound,
    };
    assert!(missing, "Shapely の起動に失敗: {result:?}");
    assert!(
        explicit.is_none() && std::env::var_os("ZUMEN_SHAPELY_REQUIRED").is_none(),
        "Shapely が必要だが、入っていない"
    );
    eprintln!("Shapely がないため外部の照合を飛ばす（xtask/verify/shapely/README.md）");
    None
}

#[allow(clippy::disallowed_methods)]
fn verify_written(oracle: &(PathBuf, PathBuf), bytes: &[u8]) -> String {
    let mut child = Command::new(&oracle.0)
        .arg("-I")
        .arg(&oracle.1)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(bytes).unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "Shapely の検査に失敗: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap()
}

#[allow(clippy::disallowed_methods)]
fn emit_shapely_geometries(oracle: &(PathBuf, PathBuf)) -> Vec<String> {
    let output = Command::new(&oracle.0)
        .arg("-I")
        .arg(&oracle.1)
        .arg("--emit")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "Shapely の出力に失敗: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout)
        .unwrap()
        .lines()
        .map(str::to_string)
        .collect()
}

fn geo_loc(lon: f64, lat: f64, height: Option<f64>) -> Location {
    Location {
        position: Position::Geo(GeoPoint {
            latitude: lat,
            longitude: lon,
        }),
        height,
    }
}

fn assert_loc_eq(loc: &Location, lon: f64, lat: f64, height: Option<f64>) {
    match loc.position {
        Position::Geo(p) => {
            assert!(
                (p.longitude - lon).abs() < 1e-12 && (p.latitude - lat).abs() < 1e-12,
                "座標不一致: got ({}, {}), expected ({lon}, {lat})",
                p.longitude,
                p.latitude
            );
        }
        Position::Map(_) => panic!("経緯度を期待した"),
    }
    match (loc.height, height) {
        (Some(a), Some(b)) => assert!((a - b).abs() < 1e-12, "標高不一致: {a} != {b}"),
        (None, None) => {}
        other => panic!("標高の有無が不一致: {other:?}"),
    }
}

fn parse_coords(raw: &str) -> Vec<Vec<f64>> {
    let trimmed = raw
        .trim()
        .strip_prefix('[')
        .unwrap()
        .strip_suffix(']')
        .unwrap();
    if trimmed.is_empty() {
        return Vec::new();
    }
    trimmed
        .split("],[")
        .map(|row| {
            row.trim_start_matches('[')
                .trim_end_matches(']')
                .split(',')
                .map(|num| num.parse::<f64>().unwrap())
                .collect()
        })
        .collect()
}

#[test]
fn matches_shapely_across_wgs84_plane_and_closed_solid() {
    let Some(oracle) = oracle() else {
        return;
    };

    // 1. Shapely 2.2.0 が書いた 7 種の Geometry を読み、図形数と全頂点座標（Z 値を含む）を照合する
    let emitted = emit_shapely_geometries(&oracle);
    assert_eq!(emitted.len(), 7);
    let expected_counts = [1usize, 2, 1, 2, 1, 2, 2];
    let mut read_geoms = Vec::new();
    for (line, expected) in emitted.iter().zip(expected_counts) {
        let mut budget = Budget::default();
        let mut diag = Diagnostics::new();
        let cancel = CancelToken::new();
        let mut cx = Cx::new(&mut budget, &mut diag, &cancel);
        let file = GeoJson::read(line.as_bytes(), &mut cx).unwrap();
        let model: GeoModel = file.lift(&mut cx).unwrap();
        assert!(diag.is_empty());
        assert_eq!(model.features.len(), 1);
        assert_eq!(model.features[0].geometries.len(), expected);
        read_geoms.push(model.features[0].geometries.clone());
    }
    match &read_geoms[0][0].form {
        Form::Point(loc) => assert_loc_eq(loc, 139.767125, 35.681236, Some(12.5)),
        other => panic!("expected Point, got {other:?}"),
    }
    match (&read_geoms[1][0].form, &read_geoms[1][1].form) {
        (Form::Point(a), Form::Point(b)) => {
            assert_loc_eq(a, 139.0, 35.0, None);
            assert_loc_eq(b, 140.0, 36.0, Some(5.0));
        }
        other => panic!("expected MultiPoint, got {other:?}"),
    }
    match &read_geoms[2][0].form {
        Form::Line(pts) => {
            assert_eq!(pts.len(), 3);
            assert_loc_eq(&pts[0], 139.0, 35.0, Some(1.0));
            assert_loc_eq(&pts[1], 139.5, 35.5, Some(2.0));
            assert_loc_eq(&pts[2], 140.0, 36.0, Some(3.0));
        }
        other => panic!("expected LineString, got {other:?}"),
    }
    match (&read_geoms[3][0].form, &read_geoms[3][1].form) {
        (Form::Line(a), Form::Line(b)) => {
            assert_loc_eq(&a[0], 139.0, 35.0, None);
            assert_loc_eq(&a[1], 139.1, 35.1, None);
            assert_loc_eq(&b[0], 139.2, 35.2, None);
            assert_loc_eq(&b[1], 139.3, 35.3, None);
        }
        other => panic!("expected MultiLineString, got {other:?}"),
    }
    match &read_geoms[4][0].form {
        Form::Area { outer, holes } => {
            assert_eq!(outer.len(), 4);
            assert_loc_eq(&outer[0], 139.0, 35.0, None);
            assert_loc_eq(&outer[2], 140.0, 36.0, None);
            assert_eq!(holes.len(), 1);
            assert_eq!(holes[0].len(), 4);
            assert_loc_eq(&holes[0][0], 139.2, 35.2, None);
            assert_loc_eq(&holes[0][2], 139.8, 35.8, None);
        }
        other => panic!("expected Polygon with hole, got {other:?}"),
    }
    match (&read_geoms[5][0].form, &read_geoms[5][1].form) {
        (Form::Area { outer: a, .. }, Form::Area { outer: b, .. }) => {
            assert_loc_eq(&a[0], 139.0, 35.0, None);
            assert_loc_eq(&a[2], 139.1, 35.1, None);
            assert_loc_eq(&b[0], 139.5, 35.5, None);
            assert_loc_eq(&b[2], 139.6, 35.6, None);
        }
        other => panic!("expected MultiPolygon, got {other:?}"),
    }
    match (&read_geoms[6][0].form, &read_geoms[6][1].form) {
        (Form::Point(pt), Form::Line(line)) => {
            assert_loc_eq(pt, 139.7, 35.6, None);
            assert_loc_eq(&line[0], 139.0, 35.0, None);
            assert_loc_eq(&line[1], 140.0, 36.0, None);
        }
        other => panic!("expected GeometryCollection, got {other:?}"),
    }

    // 2. WGS 84（逆回転の穴あき面・MultiPolygon・鉛直壁4枚＋上面＋底面の閉じた直方体 Solid・属性）を書いて Shapely で検査する
    let cw_outer = vec![
        geo_loc(139.0, 35.0, None),
        geo_loc(139.0, 36.0, None),
        geo_loc(140.0, 36.0, None),
        geo_loc(140.0, 35.0, None),
    ];
    let ccw_hole = vec![
        geo_loc(139.2, 35.2, None),
        geo_loc(139.8, 35.2, None),
        geo_loc(139.8, 35.8, None),
        geo_loc(139.2, 35.8, None),
    ];
    let box_faces = vec![
        Face {
            outer: vec![
                [35.0, 139.0, 0.0],
                [35.0, 139.1, 0.0],
                [35.1, 139.1, 0.0],
                [35.1, 139.0, 0.0],
            ],
            holes: Vec::new(),
        },
        Face {
            outer: vec![
                [35.0, 139.0, 15.0],
                [35.1, 139.0, 15.0],
                [35.1, 139.1, 15.0],
                [35.0, 139.1, 15.0],
            ],
            holes: Vec::new(),
        },
        Face {
            outer: vec![
                [35.0, 139.0, 0.0],
                [35.0, 139.1, 0.0],
                [35.0, 139.1, 15.0],
                [35.0, 139.0, 15.0],
            ],
            holes: Vec::new(),
        },
        Face {
            outer: vec![
                [35.0, 139.1, 0.0],
                [35.1, 139.1, 0.0],
                [35.1, 139.1, 15.0],
                [35.0, 139.1, 15.0],
            ],
            holes: Vec::new(),
        },
    ];
    let wgs_model = GeoModel {
        crs: Some(Crs::Wgs84),
        height: None,
        features: vec![
            Feature {
                id: "pt".to_string(),
                kind: "Feature".to_string(),
                code: None,
                name: Some("基準点".to_string()),
                attributes: vec![Attribute {
                    name: "floors".to_string(),
                    value: Value::Integer(5),
                }],
                geometries: vec![Geometry {
                    lod: None,
                    form: Form::Point(geo_loc(139.75, 35.68, Some(10.0))),
                }],
                parent: None,
                source: Ref::default(),
            },
            Feature {
                id: "holed".to_string(),
                kind: "Feature".to_string(),
                code: None,
                name: None,
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
                id: "solid_box".to_string(),
                kind: "bldg:Building".to_string(),
                code: Some("401".to_string()),
                name: None,
                attributes: Vec::new(),
                geometries: vec![Geometry {
                    lod: Some(1),
                    form: Form::Solid(Shape::FaceSet {
                        faces: box_faces,
                        closed: true,
                    }),
                }],
                parent: None,
                source: Ref::default(),
            },
        ],
    };

    let mut budget = Budget::default();
    let mut diag = Diagnostics::new();
    let cancel = CancelToken::new();
    let mut cx = Cx::new(&mut budget, &mut diag, &cancel);
    let lowered = lower_to_geojson(&wgs_model, &mut cx).unwrap();
    let summary = verify_written(&oracle, &lowered.bytes);
    let rows: Vec<&str> = summary.lines().collect();
    assert_eq!(rows.len(), 3);

    let pt_cols: Vec<&str> = rows[0].split('\t').collect();
    assert_eq!(
        pt_cols[0..4],
        ["pt", "Point", "1", r#"{"floors":5,"name":"基準点"}"#]
    );
    let pt_coords = parse_coords(pt_cols[4]);
    assert_eq!(pt_coords, vec![vec![139.75, 35.68, 10.0]]);

    let holed_cols: Vec<&str> = rows[1].split('\t').collect();
    assert_eq!(holed_cols[0..4], ["holed", "Polygon", "1", "{}"]);
    let holed_coords = parse_coords(holed_cols[4]);
    assert_eq!(
        holed_coords,
        vec![
            vec![140.0, 35.0],
            vec![140.0, 36.0],
            vec![139.0, 36.0],
            vec![139.0, 35.0],
            vec![140.0, 35.0],
            vec![139.2, 35.8],
            vec![139.8, 35.8],
            vec![139.8, 35.2],
            vec![139.2, 35.2],
            vec![139.2, 35.8],
        ]
    );

    let solid_cols: Vec<&str> = rows[2].split('\t').collect();
    assert_eq!(
        solid_cols[0..4],
        [
            "solid_box",
            "GeometryCollection",
            "2",
            r#"{"code":"401","kind":"bldg:Building"}"#,
        ]
    );
    let solid_coords = parse_coords(solid_cols[4]);
    assert_eq!(solid_coords.len(), 10);
    assert!(solid_coords[0..5]
        .iter()
        .all(|c| (c[2] - 0.0).abs() < 1e-12));
    assert!(solid_coords[5..10]
        .iter()
        .all(|c| (c[2] - 15.0).abs() < 1e-12));

    // 3. 平面直角座標系 IX 系の逆投影 GeoJSON も Shapely で有効な図形になり、座標が原点（139°50′E, 36°N）と一致すること
    let plane_model = GeoModel {
        crs: Some(Crs::Plane(Zone::new(9).unwrap())),
        height: None,
        features: vec![Feature {
            id: "zone9_area".to_string(),
            kind: "Feature".to_string(),
            code: None,
            name: None,
            attributes: Vec::new(),
            geometries: vec![Geometry {
                lod: None,
                form: Form::Area {
                    outer: vec![
                        Location {
                            position: Position::Map(MapPoint {
                                northing: 0.0,
                                easting: 0.0,
                            }),
                            height: None,
                        },
                        Location {
                            position: Position::Map(MapPoint {
                                northing: 100.0,
                                easting: 0.0,
                            }),
                            height: None,
                        },
                        Location {
                            position: Position::Map(MapPoint {
                                northing: 100.0,
                                easting: 100.0,
                            }),
                            height: None,
                        },
                        Location {
                            position: Position::Map(MapPoint {
                                northing: 0.0,
                                easting: 100.0,
                            }),
                            height: None,
                        },
                    ],
                    holes: Vec::new(),
                },
            }],
            parent: None,
            source: Ref::default(),
        }],
    };
    let mut budget = Budget::default();
    let mut diag = Diagnostics::new();
    let cancel = CancelToken::new();
    let mut cx = Cx::new(&mut budget, &mut diag, &cancel);
    let plane_lowered = lower_to_geojson(&plane_model, &mut cx).unwrap();
    let plane_summary = verify_written(&oracle, &plane_lowered.bytes);
    let plane_cols: Vec<&str> = plane_summary.lines().next().unwrap().split('\t').collect();
    assert_eq!(plane_cols[0..4], ["zone9_area", "Polygon", "1", "{}"]);
    let plane_coords = parse_coords(plane_cols[4]);
    assert_eq!(plane_coords.len(), 5);
    let zone9 = Zone::new(9).unwrap();
    let expected_ccw = [
        MapPoint {
            northing: 0.0,
            easting: 100.0,
        },
        MapPoint {
            northing: 100.0,
            easting: 100.0,
        },
        MapPoint {
            northing: 100.0,
            easting: 0.0,
        },
        MapPoint {
            northing: 0.0,
            easting: 0.0,
        },
        MapPoint {
            northing: 0.0,
            easting: 100.0,
        },
    ];
    for (got, mp) in plane_coords.iter().zip(expected_ccw) {
        let gp = zumen_crs::to_geographic(zone9, mp)
            .expect("IX 系の原点付近は厳密に逆投影できるはず")
            .point;
        assert!((got[0] - gp.longitude).abs() < 1e-12);
        assert!((got[1] - gp.latitude).abs() < 1e-12);
    }
}
