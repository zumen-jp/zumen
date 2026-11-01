//! 合成の XYZ を読み、点群の領域モデルに移す（#373）。

use zumen_base::{Budget, CancelToken, Code, Cx, Diagnostics, Limits, codes};
use zumen_pointcloud::PointCloud;
use zumen_sdk::{Codec, Error, Lift};
use zumen_xyz::Xyz;

fn open(bytes: &[u8]) -> Result<(PointCloud, Vec<Code>), Error> {
    let mut budget = Budget::new(Limits::default());
    let mut diag = Diagnostics::new();
    let cancel = CancelToken::new();
    let mut cx = Cx::new(&mut budget, &mut diag, &cancel);
    let file = Xyz::read(bytes, &mut cx)?;
    let cloud = file.lift(&mut cx)?;
    Ok((cloud, diag.iter().map(|d| d.code()).collect()))
}

#[test]
fn reads_intensity_and_color_columns() {
    let text = "// x y z i r g b\nX,Y,Z,I,R,G,B\n1,2,3,100,255,0,0\n4;5;6;200;0;255;0\n7 8 9 300 0 0 255\n1 2\n";
    let (cloud, codes) = open(text.as_bytes()).unwrap();
    assert_eq!(cloud.points.easting, [1.0, 4.0, 7.0]);
    assert_eq!(cloud.points.northing, [2.0, 5.0, 8.0]);
    assert_eq!(cloud.points.intensity.as_ref().unwrap(), &[100, 200, 300]);
    assert_eq!(cloud.points.color.as_ref().unwrap()[1], [0, 65535, 0]);
    // 見出しの行と短い行を読まない
    assert_eq!(codes, [codes::XYZ_LINE_INVALID, codes::XYZ_CRS_MISSING]);
}

#[test]
fn reads_plain_coordinates_and_tells_unknown_columns() {
    let (cloud, codes) = open(b"1 2 3\r\n4 5 6\r\n").unwrap();
    assert_eq!(cloud.points.height, [3.0, 6.0]);
    assert_eq!(cloud.points.attributes(), ["position"]);
    assert_eq!(codes, [codes::XYZ_CRS_MISSING]);
    let (_, codes) = open(b"1 2 3 4 5\n").unwrap();
    assert_eq!(codes, [codes::XYZ_COLUMN_DROPPED, codes::XYZ_CRS_MISSING]);
    assert!(matches!(open(b"no numbers\n"), Err(Error::Invalid)));
}
