//! TIFF・GeoTIFF の読み取り・PNG 復号・領域モデル変換を Pillow 13.0.0（libtiff・libpng）と tifffile 2026.11.12 と照らす（`#877`）。

#![cfg(test)]

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use zumen_base::{Budget, CancelToken, Cx, Diagnostics};
use zumen_crs::{Crs, VerticalCrs};
use zumen_geo::{Form, GeoModel, Position};
use zumen_json::Value;
use zumen_pointcloud::PointCloud;
use zumen_sdk::{Codec, Lift};
use zumen_tiff::Tiff;

#[allow(clippy::disallowed_methods, clippy::print_stderr)]
fn oracle() -> Option<(PathBuf, PathBuf)> {
    let explicit = std::env::var_os("ZUMEN_TIFF_PYTHON");
    let python = explicit
        .clone()
        .map(PathBuf::from)
        .unwrap_or_else(|| "python".into());
    let script =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../xtask/verify/tiff/verify.py");
    let result = Command::new(&python)
        .arg("-I")
        .arg(&script)
        .arg("--check")
        .output();
    let missing = match &result {
        Ok(output) if output.status.success() => {
            assert_eq!(
                String::from_utf8_lossy(&output.stdout).trim(),
                "pillow\t13.0.0\ttifffile\t2026.11.12"
            );
            return Some((python, script));
        }
        Ok(output) => output.status.code() == Some(77),
        Err(error) => error.kind() == std::io::ErrorKind::NotFound,
    };
    assert!(missing, "Pillow・tifffile の起動に失敗: {result:?}");
    assert!(
        explicit.is_none() && std::env::var_os("ZUMEN_TIFF_REQUIRED").is_none(),
        "Pillow・tifffile が必要だが、入っていない"
    );
    eprintln!("Pillow・tifffile がないため外部の照合を飛ばす（xtask/verify/tiff/README.md）");
    None
}

#[allow(clippy::disallowed_methods)]
fn emit_suite(oracle: &(PathBuf, PathBuf), dir: &Path) {
    let output = Command::new(&oracle.0)
        .arg("-I")
        .arg(&oracle.1)
        .arg("--emit")
        .arg(dir)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "verify.py --emit に失敗: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[allow(clippy::disallowed_methods)]
fn verify_png(
    oracle: &(PathBuf, PathBuf),
    png_path: &Path,
    expected_raw_path: &Path,
    width: u32,
    height: u32,
) {
    let output = Command::new(&oracle.0)
        .arg("-I")
        .arg(&oracle.1)
        .arg("--verify-png")
        .arg(png_path)
        .arg(expected_raw_path)
        .arg(width.to_string())
        .arg(height.to_string())
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "PNG の画素照合に失敗: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn as_str<'a>(v: &'a Value, ptr: &str) -> &'a str {
    match v.pointer(ptr) {
        Some(Value::String(s)) => s.as_str(),
        other => panic!("{ptr}: {other:?}"),
    }
}

fn as_u64(v: &Value, ptr: &str) -> u64 {
    match v.pointer(ptr) {
        Some(Value::Number(n)) => n.to_safe_integer().unwrap() as u64,
        other => panic!("{ptr}: {other:?}"),
    }
}

fn as_f64(v: &Value) -> f64 {
    match v {
        Value::Number(n) => n.as_str().parse::<f64>().unwrap(),
        other => panic!("expected number, got {other:?}"),
    }
}

#[test]
#[allow(clippy::disallowed_methods)]
fn matches_libtiff_and_tifffile_across_compressions_layouts_and_geotiff_dem() {
    let Some(oracle) = oracle() else {
        return;
    };

    let tmp_dir = std::env::temp_dir().join(format!("zumen-tiff-verify-{}", std::process::id()));
    let _ = fs::remove_dir_all(&tmp_dir);
    fs::create_dir_all(&tmp_dir).unwrap();

    emit_suite(&oracle, &tmp_dir);
    let manifest_bytes = fs::read(tmp_dir.join("manifest.json")).unwrap();
    let manifest = zumen_json::parse(&manifest_bytes).unwrap();

    // 1. 9 種の画像ケース（LZW・PackBits・CCITT Group 4・Deflate・タイル Predictor=2・16-bit 面分離・8-bit パレット・マルチページ）
    let Some(Value::Array(cases)) = manifest.pointer("/cases") else {
        panic!("missing /cases");
    };
    assert_eq!(cases.len(), 9);
    for case in cases {
        let name = as_str(case, "/name");
        let file_rel = as_str(case, "/file");
        let exp_rel = as_str(case, "/expected_rgba");
        let width = as_u64(case, "/width") as u32;
        let height = as_u64(case, "/height") as u32;
        let page = as_u64(case, "/page") as usize;
        let pages = as_u64(case, "/pages") as usize;

        let tiff_bytes = fs::read(tmp_dir.join(file_rel)).unwrap();
        let mut budget = Budget::default();
        let mut diag = Diagnostics::new();
        let cancel = CancelToken::new();
        let mut cx = Cx::new(&mut budget, &mut diag, &cancel);

        let tiff = Tiff::read(&tiff_bytes, &mut cx).unwrap_or_else(|e| panic!("{name}: {e:?}"));
        assert_eq!(tiff.page_count(), pages, "{name}");

        let png = tiff
            .encode_raster_png(page, None, &mut cx)
            .unwrap_or_else(|e| panic!("{name}: {e:?}"));
        assert_eq!((png.width, png.height), (width, height), "{name}");

        let out_png = tmp_dir.join(format!("{name}_out.png"));
        fs::write(&out_png, &png.bytes).unwrap();
        verify_png(&oracle, &out_png, &tmp_dir.join(exp_rel), width, height);
    }

    // 2. GeoTIFF オルソ画像の外周座標照合
    let ortho_file = as_str(&manifest, "/ortho/file");
    let ortho_bytes = fs::read(tmp_dir.join(ortho_file)).unwrap();
    {
        let mut budget = Budget::default();
        let mut diag = Diagnostics::new();
        let cancel = CancelToken::new();
        let mut cx = Cx::new(&mut budget, &mut diag, &cancel);
        let tiff = Tiff::read(&ortho_bytes, &mut cx).unwrap();
        let geo: GeoModel = tiff.lift(&mut cx).unwrap();
        assert_eq!(geo.crs, Crs::from_epsg(6677));
        assert_eq!(geo.features.len(), 1);
        match &geo.features[0].geometries[0].form {
            Form::Area { outer, .. } => {
                assert_eq!(outer.len(), 5);
                match (outer[0].position, outer[2].position) {
                    (Position::Map(bl), Position::Map(tr)) => {
                        assert!((bl.easting - (-12500.0)).abs() < 1e-9);
                        assert!((bl.northing - (-34006.0)).abs() < 1e-9);
                        assert!((tr.easting - (-12492.0)).abs() < 1e-9);
                        assert!((tr.northing - (-34000.0)).abs() < 1e-9);
                    }
                    other => panic!("unexpected positions: {other:?}"),
                }
            }
            other => panic!("unexpected form: {other:?}"),
        }
    }

    // 3. GeoTIFF Float32 DEM（PixelIsPoint・GDAL_NODATA・NaN 除外）の点群座標・標高照合
    let dem_file = as_str(&manifest, "/dem/file");
    let dem_bytes = fs::read(tmp_dir.join(dem_file)).unwrap();
    let Some(Value::Array(exp_pts)) = manifest.pointer("/dem/points") else {
        panic!("missing /dem/points");
    };
    {
        let mut budget = Budget::default();
        let mut diag = Diagnostics::new();
        let cancel = CancelToken::new();
        let mut cx = Cx::new(&mut budget, &mut diag, &cancel);
        let tiff = Tiff::read(&dem_bytes, &mut cx).unwrap();
        let cloud: PointCloud = tiff.lift(&mut cx).unwrap();
        assert_eq!(cloud.crs, Crs::from_epsg(6677));
        assert_eq!(cloud.vertical, Some(VerticalCrs::Jgd2011));
        assert_eq!(cloud.points.len(), exp_pts.len());
        for (i, item) in exp_pts.iter().enumerate() {
            let Value::Array(triple) = item else {
                panic!("expected array triple");
            };
            let exp_n = as_f64(&triple[0]);
            let exp_e = as_f64(&triple[1]);
            let exp_z = as_f64(&triple[2]);
            assert!((cloud.points.northing[i] - exp_n).abs() < 1e-9);
            assert!((cloud.points.easting[i] - exp_e).abs() < 1e-9);
            assert!((cloud.points.height[i] - exp_z).abs() < 1e-5);
        }
    }

    let _ = fs::remove_dir_all(&tmp_dir);
}
