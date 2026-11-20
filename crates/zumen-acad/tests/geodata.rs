//! DXF・DWG の GEODATA から、図面モデル（L2）の地図変換を作る試験（#685）。
#![cfg(feature = "drawing")]
#![allow(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    clippy::panic,
    reason = "既知の合成入力と公開サンプルの検査"
)]

use zumen_acad::{AcadLive, ConversionFidelity, Dwg, Dxf, lower_to_dxf_with_report};
use zumen_base::{Budget, CancelToken, Cx, Diagnostics, codes};
use zumen_crs::{Crs, ProjectPoint, Zone};
use zumen_sdk::{Codec, Lift};

/// 写す図面の点（mm）
const DRAWING: [[f64; 3]; 4] = [
    [1200.0, 3400.0, 0.0],
    [11200.0, 3400.0, 500.0],
    [-5000.0, 20000.0, 1000.0],
    [0.0, 0.0, 0.0],
];

/// ezdxf 1.4.5 の `GeoData.get_crs_transformation` で写した地図の点（東・北。m）
const EZDXF: [[f64; 2]; 4] = [
    [-12345.678, 23456.789],
    [-12337.017745962155, 23461.789],
    [-12359.347357503464, 23468.065021702823],
    [-12345.017230484542, 23453.244513627134],
];

const R2004_V1: &[u8] = include_bytes!("fixtures/geodata-r2004-v1.dwg");
const R2010: &[u8] = include_bytes!("fixtures/geodata-r2010.dwg");
const R2013: &[u8] = include_bytes!("fixtures/geodata-r2013.dwg");
const R2018: &[u8] = include_bytes!("fixtures/geodata-r2018.dwg");
const PUBLIC_GEOLOC: &[u8] = include_bytes!("../../../samples/dxf/acadsharp-geoloc.dxf");

fn run<T>(op: impl FnOnce(&mut Cx) -> T) -> (T, Diagnostics) {
    let mut budget = Budget::default();
    let mut diagnostics = Diagnostics::new();
    let cancel = CancelToken::new();
    let out = op(&mut Cx::new(&mut budget, &mut diagnostics, &cancel));
    (out, diagnostics)
}

#[test]
fn dwg_geodata_lifts_to_georeference_matching_ezdxf_and_live_engine() {
    for (name, bytes) in [("r2010", R2010), ("r2013", R2013), ("r2018", R2018)] {
        let (drawing, diag) = run(|cx| Dwg::read(bytes, cx).unwrap().lift(cx).unwrap());
        assert!(diag.is_empty(), "{name}: {diag:?}");
        let geo = drawing.georeference.expect(name);
        assert_eq!(geo.crs, Crs::Plane(Zone::new(9).unwrap()), "{name}");

        for ([x, y, z], [easting, northing]) in DRAWING.into_iter().zip(EZDXF) {
            let mapped = geo.conversion.to_map(ProjectPoint { x, y, z }).unwrap();
            assert!(
                (mapped.point.easting - easting).abs() < 1e-9,
                "{name}: easting {} != {easting}",
                mapped.point.easting
            );
            assert!(
                (mapped.point.northing - northing).abs() < 1e-9,
                "{name}: northing {} != {northing}",
                mapped.point.northing
            );
        }

        // 常駐文書エンジンも同じ変換と診断を返す
        let (live, live_open_diag) = run(|cx| AcadLive::open(bytes, cx).unwrap());
        assert!(live_open_diag.is_empty(), "{name}: {live_open_diag:?}");
        let (live_drawing, live_diag) = run(|cx| live.drawing(cx).unwrap());
        assert!(live_diag.is_empty(), "{name}: {live_diag:?}");
        assert_eq!(live_drawing, drawing, "{name}");

        // 新しい DXF への書き出しは GEODATA を書かず、落としたことを報告する
        let (lowered, lower_diag) =
            run(|cx| lower_to_dxf_with_report(&drawing, &zumen_acad::Reseat, 16, cx).unwrap());
        assert!(
            lower_diag.iter().any(|d| {
                d.code() == codes::CONVERT_ELEMENT_UNSUPPORTED
                    && d.param("kind") == Some(&zumen_base::Value::Key("georeference"))
            }),
            "{name}: {lower_diag:?}"
        );
        assert!(
            lowered
                .report
                .counts
                .iter()
                .any(|item| item.kind == "georeference"
                    && item.fidelity == ConversionFidelity::Dropped),
            "{name}: {:?}",
            lowered.report.counts
        );
    }
}

#[test]
fn dwg_v1_and_public_unknown_crs_report_diagnostics_with_live_parity() {
    let (v1_drawing, v1_diag) = run(|cx| Dwg::read(R2004_V1, cx).unwrap().lift(cx).unwrap());
    assert_eq!(v1_drawing.georeference, None);
    assert_eq!(
        v1_diag.iter().map(|d| d.code()).collect::<Vec<_>>(),
        [codes::CRS_GEODATA_UNSUPPORTED]
    );
    let first = v1_diag.iter().next().unwrap();
    assert_eq!(
        first.param("reason"),
        Some(&zumen_base::Value::Key("version"))
    );
    assert!(first.places()[0].bytes().is_some());

    let (v1_live, _) = run(|cx| AcadLive::open(R2004_V1, cx).unwrap());
    let (v1_live_drawing, v1_live_diag) = run(|cx| v1_live.drawing(cx).unwrap());
    assert_eq!(v1_live_drawing, v1_drawing);
    assert_eq!(
        v1_live_diag.iter().map(|d| d.code()).collect::<Vec<_>>(),
        [codes::CRS_GEODATA_UNSUPPORTED]
    );

    let (pub_drawing, pub_diag) = run(|cx| Dxf::read(PUBLIC_GEOLOC, cx).unwrap().lift(cx).unwrap());
    assert_eq!(pub_drawing.georeference, None);
    assert!(
        pub_diag.iter().any(|d| {
            d.code() == codes::CRS_GEODATA_CRS_UNSUPPORTED
                && d.param("name") == Some(&"WORLD-LM-CONIC".into())
        }),
        "{pub_diag:?}"
    );
    let (pub_live, _) = run(|cx| AcadLive::open(PUBLIC_GEOLOC, cx).unwrap());
    let (pub_live_drawing, pub_live_diag) = run(|cx| pub_live.drawing(cx).unwrap());
    assert_eq!(pub_live_drawing, pub_drawing);
    assert_eq!(
        pub_live_diag.iter().collect::<Vec<_>>(),
        pub_diag.iter().collect::<Vec<_>>()
    );
}
