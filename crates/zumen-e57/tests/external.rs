//! 公開のlibE57Format合成サンプルをpye57と照らす（#504）。外部ツールは配布物に入れない。

#![cfg(test)]

use zumen_base::{Budget, CancelToken, Cx, Diagnostics, codes};
use zumen_e57::{E57, E57File, Field};
use zumen_pointcloud::PointCloud;
use zumen_sdk::{Codec, Lift};

const DOUBLE: &[u8] = include_bytes!("../../../samples/e57/libe57-coloured-cube-double.e57");
const SINGLE: &[u8] = include_bytes!("../../../samples/e57/libe57-coloured-cube-float.e57");

fn open(bytes: &[u8]) -> (E57File, PointCloud) {
    let mut budget = Budget::default();
    let cancel = CancelToken::new();
    let mut diagnostics = Diagnostics::new();
    let mut cx = Cx::new(&mut budget, &mut diagnostics, &cancel);
    let file = E57::read(bytes, &mut cx).unwrap();
    let cloud = file.lift(&mut cx).unwrap();
    assert_eq!(
        diagnostics.iter().map(|d| d.code()).collect::<Vec<_>>(),
        [codes::E57_CRS_MISSING]
    );
    (file, cloud)
}

fn record(file: &E57File, cloud: &PointCloud) -> Vec<u8> {
    let mut output = b"zumen-e57 external 1\n".to_vec();
    output.extend_from_slice(&(file.scans.len() as u64).to_le_bytes());
    output.extend_from_slice(&(cloud.points.len() as u64).to_le_bytes());
    for [north, east, height] in cloud.points.bounds().unwrap() {
        for value in [east, north, height] {
            output.extend_from_slice(&value.to_le_bytes());
        }
    }
    let color = cloud.points.color.as_ref().unwrap();
    for (index, rgb) in color.iter().enumerate() {
        for value in [
            cloud.points.easting[index],
            cloud.points.northing[index],
            cloud.points.height[index],
        ] {
            output.extend_from_slice(&value.to_le_bytes());
        }
        for channel in rgb {
            output.extend_from_slice(&channel.to_le_bytes());
        }
    }
    output
}

#[test]
fn public_cubes_keep_all_points_colors_and_missing_crs() {
    for bytes in [DOUBLE, SINGLE] {
        let (file, cloud) = open(bytes);
        assert_eq!(file.scans.len(), 1);
        assert_eq!(file.images, 0);
        assert!(file.coordinates.is_none());
        let scan = &file.scans[0];
        assert_eq!(scan.records, 7680);
        assert_eq!(scan.read, 7680);
        assert_eq!(scan.columns.len(), 6);
        assert_eq!(scan.color_limits, Some([(0.0, 255.0); 3]));
        assert!(scan.pose.is_none());
        assert!(scan.dropped.is_empty());
        assert_eq!(cloud.points.len(), 7680);
        assert_eq!(cloud.points.attributes(), ["position", "color"]);
        assert_eq!(cloud.points.bounds(), Some([[-0.5; 3], [0.5; 3]]));
        assert!(cloud.crs.is_none() && cloud.vertical.is_none());
        for (field, actual) in [
            (Field::X, &cloud.points.easting),
            (Field::Y, &cloud.points.northing),
            (Field::Z, &cloud.points.height),
        ] {
            assert_eq!(
                scan.column(field)
                    .unwrap()
                    .values
                    .iter()
                    .map(|value| value.to_bits())
                    .collect::<Vec<_>>(),
                actual
                    .iter()
                    .map(|value| value.to_bits())
                    .collect::<Vec<_>>()
            );
        }
        for (index, rgb) in cloud.points.color.as_ref().unwrap().iter().enumerate() {
            for (channel, field) in [Field::Red, Field::Green, Field::Blue]
                .into_iter()
                .enumerate()
            {
                let raw = scan.column(field).unwrap().values[index];
                assert_eq!(raw, f64::from(rgb[channel] / 257));
                assert_eq!(rgb[channel] % 257, 0);
            }
        }
        assert_eq!(record(&file, &cloud).len(), 21 + 16 + 48 + 7680 * 30);
    }
}

#[cfg(not(target_arch = "wasm32"))]
mod external {
    use super::*;
    use std::io::Write;
    use std::path::PathBuf;
    use std::process::{Command, Stdio};

    // 検証用の外部ツールだけを指定する。コアの決定性の契約には含めない。
    #[allow(clippy::disallowed_methods, clippy::print_stderr)]
    fn oracle() -> Option<(PathBuf, PathBuf)> {
        let explicit = std::env::var_os("ZUMEN_PYE57_PYTHON");
        let python = explicit
            .clone()
            .map(PathBuf::from)
            .unwrap_or_else(|| "python".into());
        let script =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../xtask/verify/e57/read.py");
        let result = Command::new(&python)
            .arg("-I")
            .arg(&script)
            .arg("--check")
            .output();
        let missing = match &result {
            Ok(output) if output.status.success() => {
                assert_eq!(
                    String::from_utf8_lossy(&output.stdout)
                        .lines()
                        .collect::<Vec<_>>(),
                    ["pye57\t0.4.19", "numpy\t2.5.4", "pyquaternion\t0.9.9"]
                );
                return Some((python, script));
            }
            Ok(output) => output.status.code() == Some(77),
            Err(error) => error.kind() == std::io::ErrorKind::NotFound,
        };
        assert!(missing, "pye57の起動に失敗");
        assert!(
            explicit.is_none() && std::env::var_os("ZUMEN_PYE57_REQUIRED").is_none(),
            "pye57が必要"
        );
        eprintln!("pye57がないため外部照合を飛ばす（xtask/verify/e57/README.md）");
        None
    }

    fn compare(bytes: &[u8]) {
        let Some((python, script)) = oracle() else {
            return;
        };
        let (file, cloud) = open(bytes);
        let mut child = Command::new(python)
            .arg("-I")
            .arg(script)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        child.stdin.take().unwrap().write_all(bytes).unwrap();
        let output = child.wait_with_output().unwrap();
        assert!(
            output.status.success(),
            "pye57による公開サンプルの読み取りが失敗"
        );
        let ours = record(&file, &cloud);
        assert_eq!(ours.len(), output.stdout.len());
        assert!(
            ours == output.stdout,
            "pye57と点・順序・座標・色・範囲が一致しない"
        );
    }

    #[test]
    fn double_cube_matches_pye57() {
        compare(DOUBLE);
    }

    #[test]
    fn float_cube_matches_pye57() {
        compare(SINGLE);
    }
}
