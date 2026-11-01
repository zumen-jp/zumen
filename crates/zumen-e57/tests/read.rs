//! 合成の E57 を読み、点群の領域モデルに移す（#373）。

use std::io::Cursor;

use e57::{
    E57Writer, Quaternion, Record, RecordDataType, RecordName, RecordValue, Transform, Translation,
};
use zumen_base::{Budget, CancelToken, Code, Cx, Diagnostics, Limits, codes};
use zumen_crs::Crs;
use zumen_e57::E57;
use zumen_pointcloud::PointCloud;
use zumen_sdk::{Codec, Error, Lift};

#[cfg(test)]
fn open(bytes: &[u8]) -> Result<(PointCloud, Vec<Code>), (Error, Vec<Code>)> {
    let mut budget = Budget::new(Limits::default());
    let mut diag = Diagnostics::new();
    let cancel = CancelToken::new();
    let mut cx = Cx::new(&mut budget, &mut diag, &cancel);
    let cloud = E57::read(bytes, &mut cx).and_then(|file| file.lift(&mut cx));
    let found: Vec<Code> = diag.iter().map(|d| d.code()).collect();
    cloud
        .map(|cloud| (cloud, found.clone()))
        .map_err(|error| (error, found))
}

/// スキャンを 1 つ持つ E57 を作る
#[cfg(test)]
fn write(
    prototype: Vec<Record>,
    points: Vec<Vec<RecordValue>>,
    pose: Option<Transform>,
    crs: Option<&str>,
) -> Vec<u8> {
    let mut out = Cursor::new(Vec::new());
    {
        let mut writer = E57Writer::new(&mut out, "file").unwrap();
        writer.set_coordinate_metadata(crs.map(str::to_string));
        let mut scan = writer.add_pointcloud("scan", prototype).unwrap();
        scan.set_transform(pose);
        for point in points {
            scan.add_point(point).unwrap();
        }
        scan.finalize().unwrap();
        writer.finalize().unwrap();
    }
    out.into_inner()
}

#[test]
fn reads_cartesian_points_with_pose_and_attributes() {
    let prototype = vec![
        Record::CARTESIAN_X_F64,
        Record::CARTESIAN_Y_F64,
        Record::CARTESIAN_Z_F64,
        Record::CARTESIAN_INVALID_STATE,
        Record::INTENSITY_U16,
        Record::COLOR_RED_U8,
        Record::COLOR_GREEN_U8,
        Record::COLOR_BLUE_U8,
        Record {
            name: RecordName::RowIndex,
            data_type: RecordDataType::Integer { min: 0, max: 10 },
        },
    ];
    let point = |x: f64, state: i64, intensity: i64| {
        vec![
            RecordValue::Double(x),
            RecordValue::Double(2.0),
            RecordValue::Double(3.0),
            RecordValue::Integer(state),
            RecordValue::Integer(intensity),
            RecordValue::Integer(255),
            RecordValue::Integer(0),
            RecordValue::Integer(10),
            RecordValue::Integer(1),
        ]
    };
    let pose = Transform {
        rotation: Quaternion::default(),
        translation: Translation {
            x: 100.0,
            y: 200.0,
            z: 0.5,
        },
    };
    let bytes = write(
        prototype,
        vec![point(1.0, 0, 65535), point(9.0, 2, 0), point(4.0, 0, 0)],
        Some(pose),
        Some("EPSG:6677"),
    );
    let (cloud, found) = open(&bytes).unwrap();
    assert_eq!(cloud.points.easting, [101.0, 104.0]);
    assert_eq!(cloud.points.northing, [202.0, 202.0]);
    assert_eq!(cloud.points.height, [3.5, 3.5]);
    assert_eq!(cloud.points.intensity.as_deref(), Some(&[65535, 0][..]));
    assert_eq!(
        cloud.points.color.as_deref(),
        Some(&[[65535, 0, 2570], [65535, 0, 2570]][..])
    );
    assert_eq!(cloud.crs, Crs::from_epsg(6677));
    assert_eq!(
        found,
        [codes::E57_POINT_DROPPED, codes::E57_ATTRIBUTE_DROPPED]
    );
}

#[test]
fn converts_spherical_points_and_reports_the_missing_crs() {
    let prototype = vec![
        Record::SPHERICAL_RANGE_F64,
        Record::SPHERICAL_AZIMUTH_F64,
        Record::SPHERICAL_ELEVATION_F64,
    ];
    let half = core::f64::consts::FRAC_PI_2;
    let bytes = write(
        prototype,
        vec![
            vec![
                RecordValue::Double(2.0),
                RecordValue::Double(0.0),
                RecordValue::Double(0.0),
            ],
            vec![
                RecordValue::Double(3.0),
                RecordValue::Double(0.0),
                RecordValue::Double(half),
            ],
        ],
        None,
        None,
    );
    let (cloud, found) = open(&bytes).unwrap();
    assert_eq!(cloud.points.easting.first(), Some(&2.0));
    assert!(cloud.points.easting.get(1).unwrap().abs() < 1e-12);
    assert_eq!(cloud.points.height, [0.0, 3.0]);
    assert_eq!(cloud.points.intensity, None);
    assert_eq!(found, [codes::E57_CRS_MISSING]);
}

#[test]
fn reports_a_broken_file() {
    let mut bytes = b"ASTM-E57".to_vec();
    bytes.resize(2048, 0);
    let (error, found) = open(&bytes).unwrap_err();
    assert_eq!(error, Error::Invalid);
    assert_eq!(found, [codes::E57_FILE_INVALID]);
}
