//! GeoJSON の書き出しと読み取りを Shapely 2.2.0（GEOS）と照らす（`#876`）。

#![cfg(test)]

use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};

use zumen_base::{Budget, CancelToken, Cx, Diagnostics, Ref};
use zumen_crs::{Crs, GeoPoint, MapPoint, Zone};
use zumen_geo::{Attribute, Feature, Form, GeoModel, Geometry, Location, Position, Value};
use zumen_geojson::{GeoJson, lower_to_geojson};
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

#[test]
fn matches_shapely_across_wgs84_plane_and_closed_solid() {
    let Some(oracle) = oracle() else {
        return;
    };

    // 1. Shapely 2.2.0 が書いた 7 種の Geometry を読み、図形数を照合する
    let emitted = emit_shapely_geometries(&oracle);
    assert_eq!(emitted.len(), 7);
    let expected_counts = [1usize, 2, 1, 2, 1, 2, 2];
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
    assert!(rows[0].starts_with("pt\tPoint\t1\t"));
    assert!(rows[1].starts_with("holed\tPolygon\t1\t"));
    assert!(rows[2].starts_with("solid_box\tGeometryCollection\t2\t"));

    // 3. 平面直角座標系 IX 系の逆投影 GeoJSON も Shapely で有効な図形になること
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
    assert!(plane_summary.starts_with("zone9_area\tPolygon\t1\t"));
}
