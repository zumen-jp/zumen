//! テストの中で作る GEODATA つきの DXF を `zumen-acad` で読み、図面の点を地図へ写す（#685）。
//!
//! 期待する値は、同じ DXF を ezdxf 1.4.5（MIT）の `GeoData.get_crs_transformation` で読んで写したもの
//! （`tests/geodata_ezdxf.py`。`GEODATA` の表をこのファイルから読む）。座標系の定義の XML は Autodesk の座標系の
//! 定義の形をまねて作ったもので、`ZUMEN-TEST-IX` の名前は架空。
#![allow(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    clippy::panic,
    reason = "既知の合成入力の検査"
)]

use zumen_acad::{Data, Dxf, Object, Origin};
use zumen_base::{
    Budget, CancelToken, Code, Cx, Diagnostic, Diagnostics, Kind, Limit, Limits, Ref, codes,
};
use zumen_crs::{Crs, GeoData, GeoValue, Georeference, ProjectPoint, Zone};
use zumen_sdk::Codec;

/// GEODATA の `AcDbGeoData` の値。図面は mm、地図は EPSG:6677（平面直角座標系 IX 系）で、北を 30 度回す
const GEODATA: &[(i16, &str)] = &[
    (90, "2"),
    (330, "1F"),
    (70, "1"),
    (10, "1200.0"),
    (20, "3400.0"),
    (30, "0.0"),
    (11, "-12345.678"),
    (21, "23456.789"),
    (31, "12.5"),
    (40, "0.001"),
    (91, "4"),
    (41, "0.001"),
    (92, "4"),
    (210, "0.0"),
    (220, "0.0"),
    (230, "1.0"),
    (12, "0.5"),
    (22, "0.8660254037844386"),
    (95, "1"),
    (141, "1.0"),
    (294, "0"),
    (142, "0.0"),
    (143, "0.0"),
    (
        303,
        r#"<?xml version="1.0" encoding="UTF-16" standalone="no" ?>^J<Dictionary version="1.0" xmlns="http://www.osgeo.org/mapguide/coordinatesystem">^J<ProjectedCoordinateSystem id="ZUMEN-TEST-IX">^J<Name>ZUMEN-TEST-IX</Name>^J"#,
    ),
    (
        303,
        r#"<Axis uom="Meter">^J<CoordinateSystemAxis><AxisOrder>1</AxisOrder><AxisName>Easting</AxisName><AxisAbbreviation>E</AxisAbbreviation><AxisDirection>East</AxisDirection></CoordinateSystemAxis>^J"#,
    ),
    (
        303,
        r#"<CoordinateSystemAxis><AxisOrder>2</AxisOrder><AxisName>Northing</AxisName><AxisAbbreviation>N</AxisAbbreviation><AxisDirection>North</AxisDirection></CoordinateSystemAxis>^J</Axis>^J</ProjectedCoordinateSystem>^J"#,
    ),
    (
        301,
        r#"<Alias id="6677" type="CoordinateSystem">^J<ObjectId>ZUMEN-TEST-IX</ObjectId>^J<Namespace>EPSG Code</Namespace>^J</Alias>^J</Dictionary>^J"#,
    ),
    (302, ""),
    (93, "0"),
    (96, "0"),
];

/// 写す図面の点（mm）
const DRAWING: [[f64; 3]; 4] = [
    [1200.0, 3400.0, 0.0],
    [11200.0, 3400.0, 500.0],
    [-5000.0, 20000.0, 1000.0],
    [0.0, 0.0, 0.0],
];

/// ezdxf 1.4.5 で写した地図の点（東・北。m）
const EZDXF: [[f64; 2]; 4] = [
    [-12345.678, 23456.789],
    [-12337.017745962155, 23461.789],
    [-12359.347357503464, 23468.065021702823],
    [-12345.017230484542, 23453.244513627134],
];

/// GEODATA のレコードの番号（HEADER・OBJECTS の SECTION と根の辞書の後）
const RECORD: u64 = 4;

fn dxf(values: &[(i16, String)]) -> Vec<u8> {
    let mut out = String::from(
        "0\nSECTION\n2\nHEADER\n9\n$ACADVER\n1\nAC1024\n9\n$INSUNITS\n70\n4\n0\nENDSEC\n0\nSECTION\n2\nOBJECTS\n0\nDICTIONARY\n5\nC\n330\n0\n100\nAcDbDictionary\n0\nGEODATA\n5\n20\n330\n0\n100\nAcDbGeoData\n",
    );
    for (code, value) in values {
        out.push_str(&format!("{code}\n{value}\n"));
    }
    out.push_str("0\nENDSEC\n0\nEOF\n");
    out.into_bytes()
}

fn tags() -> Vec<(i16, String)> {
    GEODATA
        .iter()
        .map(|(code, value)| (*code, (*value).to_owned()))
        .collect()
}

fn geo_value(data: &Data) -> GeoValue<'_> {
    match data {
        Data::Int(value) => GeoValue::Int(*value),
        Data::Real(value) => GeoValue::Real(*value),
        Data::Bool(value) => GeoValue::Bool(*value),
        Data::Text(text) => GeoValue::Text(text),
        Data::Handle(_) | Data::Binary(_) => GeoValue::Other,
    }
}

fn place(object: &Object) -> Ref {
    match object.origin {
        Origin::Dxf { record } => Ref::at_record(record as u64),
        _ => Ref::default(),
    }
}

/// DXF を読み、GEODATA の値と変換を返す
fn read(
    values: &[(i16, String)],
    limits: Limits,
) -> (Option<GeoData>, Option<Georeference>, Diagnostics) {
    let mut budget = Budget::new(limits);
    let mut diagnostics = Diagnostics::new();
    let cancel = CancelToken::new();
    let mut cx = Cx::new(&mut budget, &mut diagnostics, &cancel);
    let db = Dxf::read(&dxf(values), &mut cx)
        .unwrap()
        .objects(&mut cx)
        .unwrap();
    let object = db
        .objects()
        .iter()
        .find(|object| object.kind == b"GEODATA")
        .unwrap();
    let at = place(object);
    let values = object
        .values
        .iter()
        .map(|value| (value.code, geo_value(&value.data)));
    let data = GeoData::read(values, &at, &mut cx);
    let georeference = data
        .as_ref()
        .and_then(|data| data.georeference(&at, &mut cx));
    (data, georeference, diagnostics)
}

fn codes_of(diagnostics: &Diagnostics) -> Vec<Code> {
    diagnostics.iter().map(Diagnostic::code).collect()
}

#[test]
fn maps_drawing_points_like_ezdxf() {
    let (data, georeference, diagnostics) = read(&tags(), Limits::default());
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    // ezdxf 1.4.5 で読んだ値
    let data = data.unwrap();
    assert_eq!(data.version, 2);
    assert_eq!(data.coordinate_type, 1);
    assert_eq!(data.design_point, [1200.0, 3400.0, 0.0]);
    assert_eq!(data.reference_point, [-12345.678, 23456.789, 12.5]);
    assert_eq!(data.horizontal_unit_scale, 0.001);
    assert_eq!(data.vertical_unit_scale, 0.001);
    assert_eq!(data.north_direction, [0.5, 0.8660254037844386]);
    assert_eq!(data.up_direction, [0.0, 0.0, 1.0]);
    assert_eq!(data.scale_estimation_method, 1);
    assert_eq!(data.user_scale_factor, 1.0);
    assert!(!data.sea_level_correction);
    assert_eq!((data.mesh_points, data.mesh_faces), (0, 0));
    let definition = String::from_utf8(data.definition).unwrap();
    assert_eq!(definition.len(), 746);
    assert!(definition.starts_with(r#"<?xml version="1.0" encoding="UTF-16""#));
    assert!(definition.contains("<Namespace>EPSG Code</Namespace>\n</Alias>"));

    let georeference = georeference.unwrap();
    assert_eq!(georeference.crs, Crs::Plane(Zone::new(9).unwrap()));
    let conversion = georeference.conversion;
    for (point, expected) in DRAWING.iter().zip(EZDXF) {
        let [x, y, z] = *point;
        let map = conversion.to_map(ProjectPoint { x, y, z }).unwrap();
        assert!(
            (map.point.easting - expected[0]).abs() < 1e-9
                && (map.point.northing - expected[1]).abs() < 1e-9,
            "{point:?}: {map:?} と {expected:?}"
        );
        // 高さは基準の点の高さに、設計の点からの差を m にして足す（推定）
        assert!((map.height - (12.5 + z * 0.001)).abs() < 1e-12, "{map:?}");
    }
}

/// 書き換える GEODATA の値と、期待する診断の理由。
type Case<'a> = (&'a [(i16, &'a str)], &'a [&'a str]);

#[test]
fn reports_unsupported_settings_at_the_record() {
    let cases: &[Case] = &[
        (&[(70, "3")], &["coordinate-type"]),
        (&[(95, "2"), (141, "1.0002")], &["scale-estimation"]),
        (&[(294, "1")], &["sea-level"]),
        (&[(220, "0.1")], &["up-direction"]),
        (&[(41, "1.0")], &["vertical-scale"]),
        (&[(90, "1")], &["version"]),
        (
            &[(70, "0"), (142, "10.0")],
            &["coordinate-type", "sea-level"],
        ),
    ];
    for (edits, reasons) in cases {
        let mut values = tags();
        for (code, value) in *edits {
            let at = values.iter().position(|(c, _)| c == code).unwrap();
            values[at].1 = (*value).to_owned();
        }
        let (data, georeference, diagnostics) = read(&values, Limits::default());
        assert!(data.is_some());
        assert_eq!(georeference, None, "{edits:?}");
        let found: Vec<_> = diagnostics
            .iter()
            .map(|d| {
                assert_eq!(d.code(), codes::CRS_GEODATA_UNSUPPORTED);
                assert_eq!(d.places()[0].record(), Some(RECORD));
                d.param("reason").cloned().unwrap()
            })
            .collect();
        let expected: Vec<_> = reasons.iter().map(|r| zumen_base::Value::Key(r)).collect();
        assert_eq!(found, expected, "{edits:?}");
    }
}

#[test]
fn mesh_faces_are_not_used_but_mesh_points_alone_are_ignored() {
    // 版 2 のメッシュは、元の点が 13・23、先の点が 14・24、面が 97・98・99
    let mesh = [
        (93, "3"),
        (13, "0.0"),
        (23, "0.0"),
        (14, "-12346.0"),
        (24, "23452.0"),
        (13, "10000.0"),
        (23, "0.0"),
        (14, "-12337.0"),
        (24, "23457.0"),
        (13, "0.0"),
        (23, "10000.0"),
        (14, "-12351.0"),
        (24, "23461.0"),
    ];
    let mut values = tags();
    let at = values.iter().position(|(c, _)| *c == 93).unwrap();
    values.splice(at..=at, mesh.iter().map(|(c, v)| (*c, (*v).to_owned())));
    let (data, georeference, diagnostics) = read(&values, Limits::default());
    assert_eq!(data.unwrap().mesh_points, 3);
    assert!(georeference.is_some());
    assert!(diagnostics.is_empty(), "{diagnostics:?}");

    let faces = [(96, "1"), (97, "0"), (98, "1"), (99, "2")];
    let at = values.iter().position(|(c, _)| *c == 96).unwrap();
    values.splice(at..=at, faces.iter().map(|(c, v)| (*c, (*v).to_owned())));
    let (data, georeference, diagnostics) = read(&values, Limits::default());
    assert_eq!(data.unwrap().mesh_faces, 1);
    assert_eq!(georeference, None);
    assert_eq!(codes_of(&diagnostics), [codes::CRS_GEODATA_UNSUPPORTED]);
    assert_eq!(
        diagnostics.iter().next().unwrap().param("reason"),
        Some(&zumen_base::Value::Key("mesh"))
    );
}

#[test]
fn reports_coordinate_systems_it_cannot_use() {
    let edit = |from: &str, to: &str| {
        let mut values = tags();
        for (_, value) in &mut values {
            *value = value.replace(from, to);
        }
        values
    };
    let cases = [
        // 扱わない EPSG のコード
        (edit(r#"id="6677""#, r#"id="27700""#), "EPSG:27700"),
        // EPSG のコードの別名がない
        (
            edit(
                "<Namespace>EPSG Code</Namespace>",
                "<Namespace>Other</Namespace>",
            ),
            "ZUMEN-TEST-IX",
        ),
        // XML でない
        (edit("<Dictionary", "Dictionary"), ""),
    ];
    for (values, name) in cases {
        let (_, georeference, diagnostics) = read(&values, Limits::default());
        assert_eq!(georeference, None);
        let found = diagnostics
            .iter()
            .find(|d| d.code() == codes::CRS_GEODATA_CRS_UNSUPPORTED)
            .unwrap();
        assert_eq!(found.param("name"), Some(&name.into()), "{name}");
        assert_eq!(found.places()[0].record(), Some(RECORD));
    }
    // 北が最初の軸
    let values = edit(">E</AxisAbbreviation>", ">X</AxisAbbreviation>");
    let values: Vec<_> = values
        .into_iter()
        .map(|(c, v)| {
            (
                c,
                v.replace(">N</AxisAbbreviation>", ">E</AxisAbbreviation>")
                    .replace(">X</AxisAbbreviation>", ">N</AxisAbbreviation>"),
            )
        })
        .collect();
    let (_, georeference, diagnostics) = read(&values, Limits::default());
    assert_eq!(georeference, None);
    assert_eq!(codes_of(&diagnostics), [codes::CRS_GEODATA_UNSUPPORTED]);
    assert_eq!(
        diagnostics.iter().next().unwrap().param("reason"),
        Some(&zumen_base::Value::Key("axis-order"))
    );
}

#[test]
fn reports_broken_values_by_group_code() {
    for (code, value) in [(40, "0.0"), (12, "0.0"), (10, "abc"), (90, "x")] {
        let mut values = tags();
        if code == 12 {
            let at = values.iter().position(|(c, _)| *c == 22).unwrap();
            values[at].1 = "0.0".to_owned();
        }
        let at = values.iter().position(|(c, _)| *c == code).unwrap();
        values[at].1 = value.to_owned();
        let (data, georeference, diagnostics) = read(&values, Limits::default());
        assert_eq!((data, georeference), (None, None));
        let found = diagnostics
            .iter()
            .find(|d| d.code() == codes::CRS_GEODATA_INVALID)
            .unwrap();
        assert_eq!(
            found.param("code"),
            Some(&u64::try_from(code).unwrap().into())
        );
        assert_eq!(found.places()[0].record(), Some(RECORD));
    }
}

#[test]
fn reading_counts_values_in_the_budget() {
    let values: Vec<_> = tags()
        .into_iter()
        .map(|(code, value)| {
            (
                code,
                value
                    .parse()
                    .map_or(Data::Text(value.into_bytes()), Data::Real),
            )
        })
        .collect();
    let mut budget = Budget::new(Limits::default().with(Kind::Work, Limit::Finite(10)));
    let mut diagnostics = Diagnostics::new();
    let cancel = CancelToken::new();
    let mut cx = Cx::new(&mut budget, &mut diagnostics, &cancel);
    let read = GeoData::read(
        values.iter().map(|(code, data)| (*code, geo_value(data))),
        &Ref::at_record(RECORD),
        &mut cx,
    );
    assert_eq!(read, None);
    assert_eq!(codes_of(&diagnostics), [codes::BUDGET_WORK_EXCEEDED]);
    assert_eq!(
        diagnostics.iter().next().unwrap().places()[0].record(),
        Some(RECORD)
    );
}

#[test]
fn reading_counts_decoded_bytes_in_the_budget() {
    // タグ配列の確保前計上（1 件あたり 32 バイト）と、301・303 の座標系定義文字列の確保前計上を確かめる
    for limit in [64, 512] {
        let mut budget =
            Budget::new(Limits::default().with(Kind::DecodedBytes, Limit::Finite(limit)));
        let mut diagnostics = Diagnostics::new();
        let cancel = CancelToken::new();
        let mut cx = Cx::new(&mut budget, &mut diagnostics, &cancel);
        let read = GeoData::read(
            [
                (100, GeoValue::Text(b"AcDbGeoData")),
                (90, GeoValue::Int(2)),
                (70, GeoValue::Int(1)),
                (301, GeoValue::Text(&[b'x'; 600])),
            ],
            &Ref::at_record(RECORD),
            &mut cx,
        );
        assert_eq!(read, None, "limit {limit}");
        assert_eq!(
            codes_of(&diagnostics),
            [codes::BUDGET_DECODED_BYTES_EXCEEDED],
            "limit {limit}"
        );
        assert_eq!(
            diagnostics.iter().next().unwrap().places()[0].record(),
            Some(RECORD)
        );
    }
}

#[test]
fn public_autocad_sample_reads_like_ezdxf_and_names_its_coordinate_system() {
    // ACadSharp（MIT）の geoloc.dxf。値は ezdxf 1.4.5 で読んだもの。座標系の定義に EPSG のコードの別名がないので、
    // ezdxf も変換を作らない
    let src = include_bytes!("../../../samples/dxf/acadsharp-geoloc.dxf");
    let mut budget = Budget::default();
    let mut diagnostics = Diagnostics::new();
    let cancel = CancelToken::new();
    let mut cx = Cx::new(&mut budget, &mut diagnostics, &cancel);
    let db = Dxf::read(src, &mut cx).unwrap().objects(&mut cx).unwrap();
    let object = db
        .objects()
        .iter()
        .find(|object| object.kind == b"GEODATA")
        .unwrap();
    let at = place(object);
    let data = GeoData::read(
        object
            .values
            .iter()
            .map(|value| (value.code, geo_value(&value.data))),
        &at,
        &mut cx,
    )
    .unwrap();
    assert_eq!(data.version, 3);
    assert_eq!(data.coordinate_type, 1);
    assert_eq!(
        data.design_point,
        [11.24751811939982, 17.27339827573371, 0.0]
    );
    assert_eq!(
        data.reference_point,
        [-315551.0219150639, 4702004.167534534, 50.0]
    );
    assert_eq!(data.horizontal_unit_scale, 0.0254);
    assert_eq!(data.vertical_unit_scale, 0.0254);
    assert_eq!(data.north_direction, [2e-16, 1.0]);
    assert_eq!((data.mesh_points, data.mesh_faces), (12, 0));
    assert_eq!(data.definition.len(), 3195);

    assert_eq!(data.georeference(&at, &mut cx), None);
    assert_eq!(codes_of(&diagnostics), [codes::CRS_GEODATA_CRS_UNSUPPORTED]);
    let found = diagnostics.iter().next().unwrap();
    assert_eq!(found.param("name"), Some(&"WORLD-LM-CONIC".into()));
    assert_eq!(found.places()[0], at);
}
