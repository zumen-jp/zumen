//! 図面の実寸から地図への相似の変換（GEODATA の読み取りと対応点からの推定、#685）。
#![cfg(any(feature = "drawing", feature = "civil"))]
#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic)] // 自作入力と期待応答の検査。

use zumen_abi1::{Session, Status};
use zumen_base::CancelToken;
use zumen_json::{Value, Writer};

/// 写す図面の点（mm）
#[cfg(feature = "drawing")]
const DRAWING: [[f64; 2]; 4] = [
    [1200.0, 3400.0],
    [11200.0, 3400.0],
    [-5000.0, 20000.0],
    [0.0, 0.0],
];

/// ezdxf 1.4.5 の `GeoData.get_crs_transformation` で写した地図の点（東・北。m）
#[cfg(feature = "drawing")]
const EZDXF: [[f64; 2]; 4] = [
    [-12345.678, 23456.789],
    [-12337.017745962155, 23461.789],
    [-12359.347357503464, 23468.065021702823],
    [-12345.017230484542, 23453.244513627134],
];

#[cfg(feature = "drawing")]
const GEODATA_DXF: &str = concat!(
    "0\nSECTION\n2\nHEADER\n9\n$ACADVER\n1\nAC1024\n9\n$INSUNITS\n70\n4\n0\nENDSEC\n",
    "0\nSECTION\n2\nENTITIES\n0\nLINE\n5\n30\n8\n0\n10\n0.0\n20\n0.0\n11\n1200.0\n21\n3400.0\n0\nENDSEC\n",
    "0\nSECTION\n2\nOBJECTS\n0\nDICTIONARY\n5\nC\n330\n0\n100\nAcDbDictionary\n",
    "0\nGEODATA\n5\n20\n330\n0\n100\nAcDbGeoData\n",
    "90\n2\n330\n1F\n70\n1\n",
    "10\n1200.0\n20\n3400.0\n30\n0.0\n",
    "11\n-12345.678\n21\n23456.789\n31\n12.5\n",
    "40\n0.001\n91\n4\n41\n0.001\n92\n4\n",
    "210\n0.0\n220\n0.0\n230\n1.0\n",
    "12\n0.5\n22\n0.8660254037844386\n",
    "95\n1\n141\n1.0\n294\n0\n142\n0.0\n143\n0.0\n",
    "303\n<?xml version=\"1.0\" encoding=\"UTF-16\" standalone=\"no\" ?>^J",
    "<Dictionary version=\"1.0\" xmlns=\"http://www.osgeo.org/mapguide/coordinatesystem\">^J",
    "<ProjectedCoordinateSystem id=\"ZUMEN-TEST-IX\">^J<Name>ZUMEN-TEST-IX</Name>^J\n",
    "303\n<Axis uom=\"Meter\">^J<CoordinateSystemAxis><AxisOrder>1</AxisOrder>",
    "<AxisName>Easting</AxisName><AxisAbbreviation>E</AxisAbbreviation>",
    "<AxisDirection>East</AxisDirection></CoordinateSystemAxis>^J\n",
    "303\n<CoordinateSystemAxis><AxisOrder>2</AxisOrder><AxisName>Northing</AxisName>",
    "<AxisAbbreviation>N</AxisAbbreviation><AxisDirection>North</AxisDirection>",
    "</CoordinateSystemAxis>^J</Axis>^J</ProjectedCoordinateSystem>^J\n",
    "301\n<Alias id=\"6677\" type=\"CoordinateSystem\">^J<ObjectId>ZUMEN-TEST-IX</ObjectId>^J",
    "<Namespace>EPSG Code</Namespace>^J</Alias>^J</Dictionary>^J\n",
    "302\n\n93\n0\n96\n0\n0\nENDSEC\n0\nEOF\n",
);

#[cfg(feature = "drawing")]
const R2018_DWG: &[u8] = include_bytes!("../../zumen-acad/tests/fixtures/geodata-r2018.dwg");
#[cfg(feature = "drawing")]
const R2004_V1_DWG: &[u8] = include_bytes!("../../zumen-acad/tests/fixtures/geodata-r2004-v1.dwg");
#[cfg(feature = "drawing")]
const PUBLIC_GEOLOC_DXF: &[u8] = include_bytes!("../../../samples/dxf/acadsharp-geoloc.dxf");

fn call(session: &mut Session, request: &str) -> (Status, Value) {
    let reply = session.call(request.as_bytes(), &CancelToken::new());
    (reply.status, zumen_json::parse(&reply.response).unwrap())
}

fn json(value: &Value) -> String {
    let mut writer = Writer::new();
    writer.value(value);
    String::from_utf8(writer.into_bytes()).unwrap()
}

fn number(value: &Value, pointer: &str) -> f64 {
    match value.pointer(pointer) {
        Some(Value::Number(number)) => number.to_f64().unwrap(),
        other => panic!("{pointer}: {other:?} in {}", json(value)),
    }
}

fn text(value: &Value, pointer: &str) -> String {
    match value.pointer(pointer) {
        Some(Value::String(text)) => text.clone(),
        _ => String::new(),
    }
}

#[cfg(feature = "drawing")]
#[test]
fn reads_geodata_from_dxf_dwg_and_lifted_binary_and_maps_points() {
    let mut session = Session::new(b"").unwrap();
    session.push(b"plan.dxf", GEODATA_DXF.as_bytes()).unwrap();
    session.push(b"plan.dwg", R2018_DWG).unwrap();

    for doc in ["plan.dxf", "plan.dwg"] {
        let (st, _) = call(
            &mut session,
            &format!(r#"{{"v":1,"op":"open","input":"{doc}"}}"#),
        );
        assert_eq!(st, Status::Ok);

        let (st, dr) = call(
            &mut session,
            &format!(r#"{{"v":1,"op":"drawing","doc":"{doc}"}}"#),
        );
        assert_eq!(st, Status::Ok, "{}", json(&dr));
        let hash = text(&dr, "/result/hash");
        assert_eq!(number(&dr, "/result/georeference/crs"), 6677.0);
        assert!(
            (number(&dr, "/result/georeference/conversion/scale") - 0.001).abs() < 1e-12,
            "{}",
            json(&dr)
        );
        assert!(
            (number(&dr, "/result/georeference/conversion/orthogonal_height") - 12.5).abs() < 1e-12
        );

        let points_json = DRAWING
            .iter()
            .map(|[x, y]| format!(r#"{{"x":{x:?},"y":{y:?}}}"#))
            .collect::<Vec<_>>()
            .join(",");
        let (st, gr) = call(
            &mut session,
            &format!(
                r#"{{"v":1,"op":"georeference","doc":"{doc}","expected_hash":"{hash}","points":[{points_json}]}}"#
            ),
        );
        assert_eq!(st, Status::Ok, "{}", json(&gr));
        assert_eq!(text(&gr, "/result/hash"), hash);
        assert_eq!(number(&gr, "/result/georeference/crs"), 6677.0);
        assert!(gr.pointer("/result/residuals").is_none());
        for (i, [easting, northing]) in EZDXF.iter().enumerate() {
            let got_e = number(&gr, &format!("/result/mapped/{i}/easting"));
            let got_n = number(&gr, &format!("/result/mapped/{i}/northing"));
            assert!(
                (got_e - easting).abs() < 1e-9 && (got_n - northing).abs() < 1e-9,
                "{doc}[{i}]: ({got_e}, {got_n}) != ({easting}, {northing})"
            );
        }

        // 古い expected_hash は断る
        let (st, bad) = call(
            &mut session,
            &format!(
                r#"{{"v":1,"op":"georeference","doc":"{doc}","expected_hash":"v2:00000000000000000000000000000000"}}"#
            ),
        );
        assert_eq!(st, Status::Failed);
        assert_eq!(text(&bad, "/diagnostics/0/code"), "call.value.invalid");
        assert_eq!(
            text(&bad, "/diagnostics/0/params/pointer"),
            "/expected_hash"
        );
    }

    // 図面モデルのバイナリ（lift -> take -> open）も georeference を保つ
    let (st, lifted) = call(&mut session, r#"{"v":1,"op":"lift","doc":"plan.dwg"}"#);
    assert_eq!(st, Status::Ok, "{}", json(&lifted));
    let zmdr = session.take(0).unwrap();
    session.push(b"plan.zmdr", &zmdr).unwrap();
    let (st, _) = call(&mut session, r#"{"v":1,"op":"open","input":"plan.zmdr"}"#);
    assert_eq!(st, Status::Ok);
    let (st, zmdr_gr) = call(
        &mut session,
        r#"{"v":1,"op":"georeference","doc":"plan.zmdr","points":[{"x":1200,"y":3400}]}"#,
    );
    assert_eq!(st, Status::Ok, "{}", json(&zmdr_gr));
    assert_eq!(number(&zmdr_gr, "/result/georeference/crs"), 6677.0);
    assert!((number(&zmdr_gr, "/result/mapped/0/easting") - EZDXF[0][0]).abs() < 1e-9);
    assert!((number(&zmdr_gr, "/result/mapped/0/northing") - EZDXF[0][1]).abs() < 1e-9);

    // 新しい DXF への convert は georeference を落として診断する
    let (st, conv) = call(
        &mut session,
        r#"{"v":1,"op":"convert","doc":"plan.dwg","format":"dxf","to":"out.dxf"}"#,
    );
    assert_eq!(st, Status::Ok, "{}", json(&conv));
    let _ = session.take(0).unwrap();
    assert!(
        json(&conv).contains(r#""kind":"georeference""#),
        "{}",
        json(&conv)
    );
}

#[cfg(feature = "drawing")]
#[test]
fn unsupported_geodata_returns_diagnostics_without_conversion() {
    let mut session = Session::new(b"").unwrap();
    session.push(b"geoloc.dxf", PUBLIC_GEOLOC_DXF).unwrap();
    session.push(b"v1.dwg", R2004_V1_DWG).unwrap();

    let (st, _) = call(&mut session, r#"{"v":1,"op":"open","input":"geoloc.dxf"}"#);
    assert_eq!(st, Status::Ok);
    let (st, gr) = call(
        &mut session,
        r#"{"v":1,"op":"georeference","doc":"geoloc.dxf","points":[{"x":0,"y":0}]}"#,
    );
    assert_eq!(st, Status::Ok, "{}", json(&gr));
    assert!(gr.pointer("/result/georeference").is_none());
    assert!(gr.pointer("/result/mapped").is_none());
    assert_eq!(
        text(&gr, "/diagnostics/0/code"),
        "crs.geodata-crs.unsupported"
    );
    assert_eq!(text(&gr, "/diagnostics/0/params/name"), "WORLD-LM-CONIC");

    let (st, _) = call(&mut session, r#"{"v":1,"op":"open","input":"v1.dwg"}"#);
    assert_eq!(st, Status::Ok);
    let (st, v1) = call(
        &mut session,
        r#"{"v":1,"op":"georeference","doc":"v1.dwg"}"#,
    );
    assert_eq!(st, Status::Ok, "{}", json(&v1));
    assert!(v1.pointer("/result/georeference").is_none());
    assert_eq!(text(&v1, "/diagnostics/0/code"), "crs.geodata.unsupported");
    assert_eq!(text(&v1, "/diagnostics/0/params/reason"), "version");
}

#[test]
fn fits_similarity_conversion_from_control_points_and_validates_inputs() {
    let mut session = Session::new(b"").unwrap();

    // 既知の変換: easting = 0.0008 * x - 0.0006 * y + 1000, northing = 0.0006 * x + 0.0008 * y + 2000
    // (scale = 0.001, x_axis = (0.8, 0.6))
    let drawing_pts = [
        [0.0, 0.0],
        [10000.0, 0.0],
        [0.0, 20000.0],
        [15000.0, 25000.0],
        [-5000.0, 12000.0],
    ];
    let exact_map = |[x, y]: [f64; 2]| -> [f64; 2] {
        [
            0.0008 * x - 0.0006 * y + 1000.0,
            0.0006 * x + 0.0008 * y + 2000.0,
        ]
    };
    let pairs_json = drawing_pts
        .iter()
        .map(|&p| {
            let [e, n] = exact_map(p);
            format!(
                r#"{{"drawing":{{"x":{:?},"y":{:?}}},"map":{{"easting":{e:?},"northing":{n:?}}}}}"#,
                p[0], p[1]
            )
        })
        .collect::<Vec<_>>()
        .join(",");

    let (st, exact) = call(
        &mut session,
        &format!(
            r#"{{"v":1,"op":"georeference","crs":6677,"control_points":[{pairs_json}],"points":[{{"x":5000,"y":10000}}]}}"#
        ),
    );
    assert_eq!(st, Status::Ok, "{}", json(&exact));
    assert!(exact.pointer("/result/hash").is_none());
    assert_eq!(number(&exact, "/result/georeference/crs"), 6677.0);
    assert!((number(&exact, "/result/georeference/conversion/eastings") - 1000.0).abs() < 1e-9);
    assert!((number(&exact, "/result/georeference/conversion/northings") - 2000.0).abs() < 1e-9);
    assert!((number(&exact, "/result/georeference/conversion/scale") - 0.001).abs() < 1e-12);
    assert!(
        (number(&exact, "/result/georeference/conversion/x_axis_abscissa") - 0.8).abs() < 1e-12
    );
    assert!(
        (number(&exact, "/result/georeference/conversion/x_axis_ordinate") - 0.6).abs() < 1e-12
    );
    let [exp_e, exp_n] = exact_map([5000.0, 10000.0]);
    assert!((number(&exact, "/result/mapped/0/easting") - exp_e).abs() < 1e-9);
    assert!((number(&exact, "/result/mapped/0/northing") - exp_n).abs() < 1e-9);
    for i in 0..5 {
        let re = number(&exact, &format!("/result/residuals/{i}/easting"));
        let rn = number(&exact, &format!("/result/residuals/{i}/northing"));
        assert!(libm::hypot(re, rn) < 1e-11, "{i}: ({re}, {rn})");
    }

    // 2 番目（index = 2）の点だけずらした組では、その点の残差がもっとも大きくなる
    let shifted_json = drawing_pts
        .iter()
        .enumerate()
        .map(|(idx, &p)| {
            let [mut e, mut n] = exact_map(p);
            if idx == 2 {
                e += 0.8;
                n -= 0.6;
            }
            format!(
                r#"{{"drawing":{{"x":{:?},"y":{:?}}},"map":{{"easting":{e:?},"northing":{n:?}}}}}"#,
                p[0], p[1]
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    let (st, shifted) = call(
        &mut session,
        &format!(r#"{{"v":1,"op":"georeference","crs":6677,"control_points":[{shifted_json}]}}"#),
    );
    assert_eq!(st, Status::Ok, "{}", json(&shifted));
    let mut dists = [0.0; 5];
    for (i, d) in dists.iter_mut().enumerate() {
        let re = number(&shifted, &format!("/result/residuals/{i}/easting"));
        let rn = number(&shifted, &format!("/result/residuals/{i}/northing"));
        *d = libm::hypot(re, rn);
    }
    for (i, &d) in dists.iter().enumerate() {
        if i != 2 {
            assert!(dists[2] > d, "dists={dists:?}");
        }
    }

    // 1 点だけ・重なる 2 点・地理座標系・組み合わせ違反・予算停止
    for (req, code, param_ptr) in [
        (
            r#"{"v":1,"op":"georeference","crs":6677,"control_points":[{"drawing":{"x":0,"y":0},"map":{"easting":0,"northing":0}}]}"#,
            "crs.control-points.failed",
            "",
        ),
        (
            r#"{"v":1,"op":"georeference","crs":6677,"control_points":[{"drawing":{"x":5,"y":5},"map":{"easting":0,"northing":0}},{"drawing":{"x":5,"y":5},"map":{"easting":10,"northing":10}}]}"#,
            "crs.control-points.failed",
            "",
        ),
        (
            r#"{"v":1,"op":"georeference","crs":4326,"control_points":[{"drawing":{"x":0,"y":0},"map":{"easting":0,"northing":0}},{"drawing":{"x":1,"y":0},"map":{"easting":1,"northing":0}}]}"#,
            "call.value.invalid",
            "/crs",
        ),
        (
            r#"{"v":1,"op":"georeference","doc":"a.dxf","crs":6677}"#,
            "call.field.unsupported",
            "/crs",
        ),
        (
            r#"{"v":1,"op":"georeference","crs":6677}"#,
            "call.field.missing",
            "/control_points",
        ),
        (
            r#"{"v":1,"op":"georeference","crs":6677,"control_points":[{"drawing":{"x":0,"y":0},"map":{"easting":0,"northing":0}},{"drawing":{"x":1,"y":0},"map":{"easting":1,"northing":0}}],"budget":{"work":0}}"#,
            "budget.work.exceeded",
            "",
        ),
    ] {
        let (st, res) = call(&mut session, req);
        assert_eq!(st, Status::Failed, "{req}: {}", json(&res));
        assert!(res.pointer("/result").is_none());
        assert_eq!(text(&res, "/diagnostics/0/code"), code, "{req}");
        if !param_ptr.is_empty() {
            assert_eq!(text(&res, "/diagnostics/0/params/pointer"), param_ptr);
        }
    }
}
