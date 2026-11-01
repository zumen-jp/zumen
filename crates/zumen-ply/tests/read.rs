//! 合成の PLY を読み、点群の領域モデルに移す（#373）。

use zumen_base::{Budget, CancelToken, Code, Cx, Diagnostics, Limits, codes};
use zumen_ply::{Encoding, Ply};
use zumen_pointcloud::PointCloud;
use zumen_sdk::{Codec, Error, Lift};

fn open(bytes: &[u8]) -> Result<(Encoding, PointCloud, Vec<Code>), Error> {
    let mut budget = Budget::new(Limits::default());
    let mut diag = Diagnostics::new();
    let cancel = CancelToken::new();
    let mut cx = Cx::new(&mut budget, &mut diag, &cancel);
    let file = Ply::read(bytes, &mut cx)?;
    let cloud = file.lift(&mut cx)?;
    Ok((
        file.encoding,
        cloud,
        diag.iter().map(|d| d.code()).collect(),
    ))
}

const HEADER: &str = "ply\nformat ascii 1.0\ncomment 試験\nelement vertex 2\nproperty float x\nproperty float y\nproperty float z\nproperty float nx\nproperty uchar red\nproperty uchar green\nproperty uchar blue\nproperty float scalar_Intensity\nelement face 1\nproperty list uchar int vertex_indices\nend_header\n";

#[test]
fn reads_ascii_vertices_and_drops_the_rest() {
    let text = format!("{HEADER}1 2 3 0 255 0 10 0.5\n4 5 6 1 0 128 255 1\n2 0 1\n");
    let (encoding, cloud, codes) = open(text.as_bytes()).unwrap();
    assert_eq!(encoding, Encoding::Ascii);
    assert_eq!(cloud.points.easting, [1.0, 4.0]);
    assert_eq!(cloud.points.northing, [2.0, 5.0]);
    assert_eq!(cloud.points.height, [3.0, 6.0]);
    assert_eq!(cloud.points.color.as_ref().unwrap()[0], [65535, 0, 2570]);
    assert_eq!(cloud.points.intensity.as_ref().unwrap(), &[32768, 65535]);
    assert_eq!(cloud.crs, None);
    assert_eq!(
        codes,
        [
            codes::PLY_PROPERTY_DROPPED,
            codes::PLY_ELEMENT_DROPPED,
            codes::PLY_CRS_MISSING
        ]
    );
}

fn binary(big: bool) -> Vec<u8> {
    let name = if big { "big" } else { "little" };
    let mut out = format!(
        "ply\nformat binary_{name}_endian 1.0\nelement vertex 2\nproperty double x\nproperty double y\nproperty double z\nproperty ushort classification\nelement face 1\nproperty list uchar int vertex_indices\nend_header\n"
    )
    .into_bytes();
    for (point, class) in [([1.5, 2.5, 3.5], 2u16), ([4.0, 5.0, 6.0], 6)] {
        for value in point {
            out.extend(if big {
                f64::to_be_bytes(value)
            } else {
                f64::to_le_bytes(value)
            });
        }
        out.extend(if big {
            class.to_be_bytes()
        } else {
            class.to_le_bytes()
        });
    }
    out.push(3);
    for index in [0i32, 1, 0] {
        out.extend(if big {
            index.to_be_bytes()
        } else {
            index.to_le_bytes()
        });
    }
    out
}

#[test]
fn reads_both_binary_byte_orders() {
    for big in [false, true] {
        let (encoding, cloud, _) = open(&binary(big)).unwrap();
        assert_eq!(encoding, if big { Encoding::Big } else { Encoding::Little });
        assert_eq!(cloud.points.easting, [1.5, 4.0]);
        assert_eq!(cloud.points.height, [3.5, 6.0]);
        assert_eq!(cloud.points.classification.as_ref().unwrap(), &[2, 6]);
    }
}

#[test]
fn rejects_a_short_body() {
    let mut bytes = binary(false);
    bytes.truncate(bytes.len() - 20);
    assert!(matches!(open(&bytes), Err(Error::Invalid)));
    assert!(matches!(
        open(b"ply\nformat ascii 1.0\n"),
        Err(Error::Invalid)
    ));
}
