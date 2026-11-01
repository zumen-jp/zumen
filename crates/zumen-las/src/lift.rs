//! 点のレコードを、点群の領域モデルの列にする（決定 0232）。
//!
//! X を東（`easting`）、Y を北（`northing`）、Z を標高にし、倍率とずれを当てる。座標系は、WKT の印があれば WKT の
//! レコード（`LASF_Projection` の 2112）、なければ GeoTIFF のキーのレコード（同じく 34735）から EPSG のコードを読む。

use zumen_base::{Diagnostic, Kind, Ref, codes};
use zumen_crs::{Crs, VerticalCrs};
use zumen_pointcloud::{Octree, PointCloud, Points};
use zumen_sdk::{Cx, Error};

use crate::{LasFile, base_length, f64_at, u16_at};

/// 座標系のレコードの利用者の ID
const PROJECTION: &str = "LASF_Projection";
/// GeoTIFF のキーの表のレコード
const GEO_KEYS: u16 = 34735;
/// 座標系の WKT のレコード
const WKT: u16 = 2112;

/// GeoTIFF のキー: 投影の座標系・経緯度の座標系・高さの座標系
const PROJECTED_KEY: u16 = 3072;
const GEOGRAPHIC_KEY: u16 = 2048;
const VERTICAL_KEY: u16 = 4096;

pub(crate) fn lift(file: &LasFile, cx: &mut Cx<'_>) -> Result<PointCloud, Error> {
    let header = &file.header;
    let format = header.point_format;
    let length = usize::from(header.record_length);
    let count = file.points.len() / length.max(1);
    cx.check(Kind::Elements, count as u64)?;
    let legacy = format <= 5;
    let time_at = match format {
        1 | 3..=5 => Some(20),
        6..=10 => Some(22),
        _ => None,
    };
    let color_at = match format {
        2 => Some(20),
        3 | 5 => Some(28),
        7 | 8 | 10 => Some(30),
        _ => None,
    };
    let mut points = Points {
        intensity: Some(Vec::with_capacity(count)),
        classification: Some(Vec::with_capacity(count)),
        returns: Some(Vec::with_capacity(count)),
        time: time_at.map(|_| Vec::with_capacity(count)),
        color: color_at.map(|_| Vec::with_capacity(count)),
        ..Points::default()
    };
    let [sx, sy, sz] = header.scale;
    let [ox, oy, oz] = header.offset;
    for record in file.points.chunks_exact(length.max(1)) {
        cx.check(Kind::Work, 1)?;
        let i32_at = |offset| crate::at(record, offset).map(i32::from_le_bytes);
        let (Some(x), Some(y), Some(z)) = (i32_at(0), i32_at(4), i32_at(8)) else {
            continue;
        };
        points.easting.push(f64::from(x) * sx + ox);
        points.northing.push(f64::from(y) * sy + oy);
        points.height.push(f64::from(z) * sz + oz);
        let byte = |offset: usize| record.get(offset).copied().unwrap_or(0);
        if let Some(column) = &mut points.intensity {
            column.push(u16_at(record, 12).unwrap_or(0));
        }
        let flags = byte(14);
        if let Some(column) = &mut points.returns {
            column.push(if legacy {
                [flags & 0x07, (flags >> 3) & 0x07]
            } else {
                [flags & 0x0f, flags >> 4]
            });
        }
        if let Some(column) = &mut points.classification {
            column.push(if legacy { byte(15) & 0x1f } else { byte(16) });
        }
        if let (Some(column), Some(offset)) = (&mut points.time, time_at) {
            column.push(f64_at(record, offset).unwrap_or(0.0));
        }
        if let (Some(column), Some(offset)) = (&mut points.color, color_at) {
            column.push([0, 2, 4].map(|step| u16_at(record, offset + step).unwrap_or(0)));
        }
    }
    dropped(file, cx);
    let (crs, vertical) = crs(file, cx);
    let octree = Octree::build(&points, cx)?;
    Ok(PointCloud {
        crs,
        vertical,
        points,
        octree,
        source: Ref::at_bytes(
            u64::from(header.point_offset),
            u64::from(header.point_offset) + file.points.len() as u64,
        ),
    })
}

/// 点群の領域モデルに持たない属性を知らせる
fn dropped(file: &LasFile, cx: &mut Cx<'_>) {
    let format = file.header.point_format;
    if file.points.is_empty() {
        return;
    }
    let mut names = vec!["scan-angle", "user-data", "point-source", "flags"];
    if matches!(format, 8 | 10) {
        names.push("near-infrared");
    }
    if matches!(format, 4 | 5 | 9 | 10) {
        names.push("waveform");
    }
    if base_length(format).is_some_and(|base| file.header.record_length > base) {
        names.push("extra-bytes");
    }
    for name in names {
        cx.report(Diagnostic::new(codes::LAS_ATTRIBUTE_DROPPED).with("attribute", name));
    }
}

/// 水平と高さの座標系。扱えなければ知らせて `None` にする
fn crs(file: &LasFile, cx: &mut Cx<'_>) -> (Option<Crs>, Option<VerticalCrs>) {
    let codes_found = if file.header.wkt() {
        file.vlr(PROJECTION, WKT)
            .map(|vlr| zumen_crs::wkt_codes(&String::from_utf8_lossy(&vlr.data)))
    } else {
        file.vlr(PROJECTION, GEO_KEYS)
            .map(|vlr| key_codes(&vlr.data))
    };
    let Some((horizontal, vertical)) = codes_found else {
        cx.report(Diagnostic::new(codes::LAS_CRS_MISSING));
        return (None, None);
    };
    // 点は地図の空間に置くので、経緯度の座標系は受けない
    let plane = horizontal
        .iter()
        .find_map(|code| Crs::from_epsg(*code).filter(|crs| matches!(crs, Crs::Plane(_))));
    let crs = match (plane, horizontal.first()) {
        (Some(crs), _) => Some(crs),
        (None, Some(code)) => {
            cx.report(Diagnostic::new(codes::LAS_CRS_UNSUPPORTED).with("epsg", u64::from(*code)));
            None
        }
        (None, None) => {
            cx.report(Diagnostic::new(codes::LAS_CRS_MISSING));
            None
        }
    };
    let height = vertical
        .iter()
        .find_map(|code| VerticalCrs::from_epsg(*code));
    if let (None, Some(code)) = (height, vertical.first()) {
        cx.report(
            Diagnostic::new(codes::LAS_VERTICAL_CRS_UNSUPPORTED).with("epsg", u64::from(*code)),
        );
    }
    (crs, height)
}

/// GeoTIFF のキーの表から、水平と高さの EPSG のコード
fn key_codes(data: &[u8]) -> (Vec<u32>, Vec<u32>) {
    let (mut horizontal, mut vertical) = (Vec::new(), Vec::new());
    let keys = u16_at(data, 6).unwrap_or(0);
    for index in 0..usize::from(keys) {
        let at = 8 + index * 8;
        let (Some(id), Some(location), Some(value)) =
            (u16_at(data, at), u16_at(data, at + 2), u16_at(data, at + 6))
        else {
            break;
        };
        // 値がキーの中にあるもの（位置が 0）だけを読む。32767 は利用者が決めた値
        if location != 0 || value == 0 || value == 32767 {
            continue;
        }
        match id {
            PROJECTED_KEY | GEOGRAPHIC_KEY => horizontal.push(u32::from(value)),
            VERTICAL_KEY => vertical.push(u32::from(value)),
            _ => {}
        }
    }
    // 投影の座標系を、経緯度の座標系より先にする
    horizontal.sort_by_key(|code| Crs::from_epsg(*code) == Some(Crs::Geographic));
    (horizontal, vertical)
}
