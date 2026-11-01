//! 合成の LAS・LAZ を読み、点群の領域モデルに移す（#371）。

use std::io::Cursor;

use zumen_base::{Budget, CancelToken, Code, Cx, Diagnostics, Limits, codes};
use zumen_crs::{Crs, VerticalCrs, Zone};
use zumen_las::{Las, LasFile};
use zumen_pointcloud::{PointCloud, Query, select};
use zumen_sdk::ranged::{Missing, Ranged, Source};
use zumen_sdk::{Codec, Error, Lift};

/// 1 つの VLR
fn vlr(user: &str, record: u16, data: &[u8]) -> Vec<u8> {
    let mut out = vec![0, 0];
    let mut name = user.as_bytes().to_vec();
    name.resize(16, 0);
    out.extend(name);
    out.extend(record.to_le_bytes());
    out.extend((data.len() as u16).to_le_bytes());
    out.extend([0; 32]);
    out.extend(data);
    out
}

/// GeoTIFF のキーの表（投影の座標系と高さの座標系）
fn geo_keys(projected: u16, vertical: u16) -> Vec<u8> {
    [1, 1, 0, 2, 3072, 0, 1, projected, 4096, 0, 1, vertical]
        .iter()
        .flat_map(|value: &u16| value.to_le_bytes())
        .collect()
}

/// 点の形式 3 のレコード。倍率 0.01、ずれ（X 1000・Y 2000・Z 0）
fn record(point: [i32; 3], intensity: u16, class: u8, color: [u16; 3]) -> Vec<u8> {
    let mut out = Vec::new();
    for value in point {
        out.extend(value.to_le_bytes());
    }
    out.extend(intensity.to_le_bytes());
    // 1 番目の反射、反射の数 2
    out.push(1 | (2 << 3));
    out.push(class);
    out.extend([0, 0, 0, 0]);
    out.extend(12.5f64.to_le_bytes());
    for value in color {
        out.extend(value.to_le_bytes());
    }
    out
}

/// 見出し。`minor` が 4 なら 1.4 の見出しにする
fn header(minor: u8, format: u8, length: u16, count: u32, vlrs: &[Vec<u8>], wkt: bool) -> Vec<u8> {
    let size: u16 = if minor >= 4 { 375 } else { 227 };
    let vlr_bytes: usize = vlrs.iter().map(Vec::len).sum();
    let mut out = b"LASF".to_vec();
    out.extend([0, 0]);
    out.extend((if wkt { 0x10u16 } else { 0 }).to_le_bytes());
    out.extend([0; 16]);
    out.extend([1, minor]);
    out.extend([0; 64]);
    out.extend([0; 4]);
    out.extend(size.to_le_bytes());
    out.extend((u32::from(size) + vlr_bytes as u32).to_le_bytes());
    out.extend((vlrs.len() as u32).to_le_bytes());
    out.push(format);
    out.extend(length.to_le_bytes());
    out.extend(count.to_le_bytes());
    out.extend([0; 20]);
    for value in [
        0.01, 0.01, 0.01, 1000.0, 2000.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0,
    ] {
        out.extend(f64::to_le_bytes(value));
    }
    if minor >= 4 {
        out.extend([0; 8]);
        out.extend([0; 8]);
        out.extend([0; 4]);
        out.extend(u64::from(count).to_le_bytes());
        out.extend([0; 120]);
    }
    assert_eq!(out.len(), usize::from(size));
    for v in vlrs {
        out.extend(v);
    }
    out
}

#[cfg(test)]
fn records() -> Vec<u8> {
    (0..100)
        .flat_map(|i: i32| {
            record(
                [i * 10, i * 20, i],
                u16::try_from(i).unwrap(),
                if i % 2 == 0 { 2 } else { 6 },
                [256, 512, 1024],
            )
        })
        .collect()
}

fn las(minor: u8, vlrs: &[Vec<u8>], wkt: bool) -> Vec<u8> {
    let mut out = header(minor, 3, 34, 100, vlrs, wkt);
    out.extend(records());
    out
}

#[cfg(test)]
fn laz() -> Vec<u8> {
    let items = laz::LazItemRecordBuilder::default_for_point_format_id(3, 0).unwrap();
    let vlr = laz::LazVlrBuilder::new(items).build();
    let mut data = Vec::new();
    vlr.write_to(&mut data).unwrap();
    let vlrs = [
        vlr_record(laz::LazVlr::USER_ID, laz::LazVlr::RECORD_ID, &data),
        vlr_record("LASF_Projection", 34735, &geo_keys(6677, 6695)),
    ];
    let mut out = header(2, 3 | 0x80, 34, 100, &vlrs, false);
    let start = out.len() as u64;
    let mut cursor = Cursor::new(std::mem::take(&mut out));
    cursor.set_position(start);
    laz::compress_buffer(&mut cursor, &records(), vlr).unwrap();
    cursor.into_inner()
}

fn vlr_record(user: &str, record: u16, data: &[u8]) -> Vec<u8> {
    vlr(user, record, data)
}

fn open(bytes: &[u8]) -> Result<(LasFile, PointCloud, Vec<Code>), Error> {
    let mut budget = Budget::new(Limits::default());
    let mut diag = Diagnostics::new();
    let cancel = CancelToken::new();
    let mut cx = Cx::new(&mut budget, &mut diag, &cancel);
    let file = Las::read(bytes, &mut cx)?;
    let cloud = file.lift(&mut cx)?;
    Ok((file, cloud, diag.iter().map(|d| d.code()).collect()))
}

fn dropped() -> [Code; 4] {
    [codes::LAS_ATTRIBUTE_DROPPED; 4]
}

#[test]
fn reads_points_with_scale_offset_and_attributes() {
    let bytes = las(
        2,
        &[vlr("LASF_Projection", 34735, &geo_keys(6677, 6695))],
        false,
    );
    let (file, cloud, codes) = open(&bytes).unwrap();
    assert_eq!(file.header.version, (1, 2));
    assert_eq!(cloud.points.len(), 100);
    assert_eq!(codes, dropped());
    assert_eq!(cloud.crs, Some(Crs::Plane(Zone::new(9).unwrap())));
    assert_eq!(cloud.vertical, Some(VerticalCrs::Jgd2011));
    // X を東、Y を北にする
    assert!((cloud.points.easting[3] - 1000.3).abs() < 1e-9);
    assert!((cloud.points.northing[3] - 2000.6).abs() < 1e-9);
    assert!((cloud.points.height[3] - 0.03).abs() < 1e-9);
    assert_eq!(cloud.points.classification.as_ref().unwrap()[3], 6);
    assert_eq!(cloud.points.returns.as_ref().unwrap()[3], [1, 2]);
    assert_eq!(cloud.points.color.as_ref().unwrap()[3], [256, 512, 1024]);
    assert_eq!(cloud.points.time.as_ref().unwrap()[3], 12.5);
    assert_eq!(
        cloud.points.attributes(),
        [
            "position",
            "intensity",
            "classification",
            "color",
            "returns",
            "time"
        ]
    );
    // 八分木の節は、どの点も 1 度ずつ持つ
    let mut all: Vec<u32> = cloud
        .octree
        .nodes
        .iter()
        .flat_map(|node| node.points.clone())
        .collect();
    all.sort_unstable();
    assert_eq!(all, (0..100).collect::<Vec<_>>());
}

#[test]
fn reads_laz_like_las() {
    let (file, cloud, codes) = open(&laz()).unwrap();
    assert!(file.header.compressed);
    let (_, plain, _) = open(&las(
        2,
        &[vlr("LASF_Projection", 34735, &geo_keys(6677, 6695))],
        false,
    ))
    .unwrap();
    assert_eq!(cloud.points, plain.points);
    assert_eq!(codes, dropped());
}

#[test]
fn reads_the_crs_from_wkt() {
    let wkt = br#"COMPD_CS["JGD2011 / Japan Plane Rectangular CS IX + JGD2011 (vertical) height",PROJCS["JGD2011 / Japan Plane Rectangular CS IX",GEOGCS["JGD2011",AUTHORITY["EPSG","6668"]],AUTHORITY["EPSG","6677"]],VERT_CS["JGD2011 (vertical) height",VERT_DATUM["Japanese Standard Levelling Datum 2011",2005,AUTHORITY["EPSG","1131"]],AUTHORITY["EPSG","6695"]]]"#;
    let bytes = las(4, &[vlr("LASF_Projection", 2112, wkt)], true);
    let (file, cloud, codes) = open(&bytes).unwrap();
    assert_eq!(file.header.version, (1, 4));
    assert_eq!(cloud.points.len(), 100);
    assert_eq!(codes, dropped());
    assert_eq!(cloud.crs, Some(Crs::Plane(Zone::new(9).unwrap())));
    assert_eq!(cloud.vertical, Some(VerticalCrs::Jgd2011));
}

#[test]
fn tells_unknown_crs_and_short_records() {
    let mut bytes = las(
        2,
        &[vlr("LASF_Projection", 34735, &geo_keys(32654, 5773))],
        false,
    );
    bytes.truncate(bytes.len() - 34 * 10 - 5);
    let (_, cloud, codes) = open(&bytes).unwrap();
    assert_eq!(cloud.points.len(), 89);
    assert_eq!(cloud.crs, None);
    let mut expected = vec![codes::LAS_POINTS_INVALID];
    expected.extend(dropped());
    expected.extend([
        codes::LAS_CRS_UNSUPPORTED,
        codes::LAS_VERTICAL_CRS_UNSUPPORTED,
    ]);
    assert_eq!(codes, expected);
    let (_, _, codes) = open(&las(2, &[], false)).unwrap();
    assert_eq!(codes.last(), Some(&codes::LAS_CRS_MISSING));
    assert!(matches!(
        open(b"LASF but not a header"),
        Err(Error::Invalid)
    ));
}

#[test]
fn asks_for_ranges_and_reads_them() {
    let bytes = laz();
    let mut budget = Budget::new(Limits::default());
    let mut diag = Diagnostics::new();
    let cancel = CancelToken::new();
    let mut cx = Cx::new(&mut budget, &mut diag, &cancel);
    let size = bytes.len() as u64;
    // 初めの 100 バイトだけでは、見出しが足りない
    let first = (0, &bytes[..100]);
    let Err(Missing::Need(needs)) = Las::read_ranged(
        &Source {
            size,
            parts: &[first],
        },
        &mut cx,
    ) else {
        panic!("見出しを求めない");
    };
    assert_eq!((needs[0].start, needs[0].end), (0, 375.min(size)));
    let head = (0, &bytes[..400]);
    let Err(Missing::Need(needs)) = Las::read_ranged(
        &Source {
            size,
            parts: &[head],
        },
        &mut cx,
    ) else {
        panic!("点の範囲を求めない");
    };
    let parts: Vec<(u64, &[u8])> = std::iter::once(head)
        .chain(needs.iter().map(|range| {
            (
                range.start,
                &bytes[range.start as usize..range.end as usize],
            )
        }))
        .collect();
    let file = Las::read_ranged(
        &Source {
            size,
            parts: &parts,
        },
        &mut cx,
    )
    .unwrap();
    assert_eq!(file.points.len(), 3400);
}

#[test]
fn selects_points_by_box_and_limit() {
    let (_, cloud, _) = open(&las(2, &[], false)).unwrap();
    let mut budget = Budget::new(Limits::default());
    let mut diag = Diagnostics::new();
    let cancel = CancelToken::new();
    let mut cx = Cx::new(&mut budget, &mut diag, &cancel);
    let all = select(
        &cloud,
        &Query {
            limit: 1000,
            ..Query::default()
        },
        &mut cx,
    )
    .unwrap();
    assert_eq!(all.points.len(), 100);
    assert!(!all.truncated);
    // 北 2000〜2010.05・東 1000〜1100 の箱は、0〜50 番の点
    let boxed = select(
        &cloud,
        &Query {
            bounds: Some([[2000.0, 1000.0, -1.0], [2010.05, 1100.0, 10.0]]),
            limit: 1000,
            ..Query::default()
        },
        &mut cx,
    )
    .unwrap();
    let mut points = boxed.points.clone();
    points.sort_unstable();
    assert_eq!(points, (0..=50).collect::<Vec<_>>());
    let few = select(
        &cloud,
        &Query {
            limit: 10,
            ..Query::default()
        },
        &mut cx,
    )
    .unwrap();
    assert_eq!(few.points.len(), 10);
    assert!(few.truncated);
}
