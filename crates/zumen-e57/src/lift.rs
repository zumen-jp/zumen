//! E57 のスキャンを、点群の領域モデル（決定 0232）に移す。

use std::collections::BTreeSet;

use zumen_base::{Diagnostic, Kind, Ref, codes};
use zumen_crs::{Crs, VerticalCrs};
use zumen_pointcloud::{Octree, PointCloud, Points};
use zumen_sdk::{Cx, Error, Lift};

use crate::{Column, E57File, Field, Scan};

/// 姿勢の回転の行列と移動
struct Pose {
    rotation: [[f64; 3]; 3],
    translation: [f64; 3],
}

impl Pose {
    /// 四元数を正規化して、回転の行列にする。長さが 0 なら回転しない
    fn new(([w, x, y, z], translation): ([f64; 4], [f64; 3])) -> Pose {
        let length = libm::sqrt(w * w + x * x + y * y + z * z);
        let (w, x, y, z) = if length > 0.0 && length.is_finite() {
            (w / length, x / length, y / length, z / length)
        } else {
            (1.0, 0.0, 0.0, 0.0)
        };
        Pose {
            rotation: [
                [
                    1.0 - 2.0 * (y * y + z * z),
                    2.0 * (x * y - z * w),
                    2.0 * (x * z + y * w),
                ],
                [
                    2.0 * (x * y + z * w),
                    1.0 - 2.0 * (x * x + z * z),
                    2.0 * (y * z - x * w),
                ],
                [
                    2.0 * (x * z - y * w),
                    2.0 * (y * z + x * w),
                    1.0 - 2.0 * (x * x + y * y),
                ],
            ],
            translation,
        }
    }

    fn apply(&self, point: [f64; 3]) -> [f64; 3] {
        let [a, b, c] = self.rotation;
        let dot = |row: [f64; 3]| row[0] * point[0] + row[1] * point[1] + row[2] * point[2];
        let [tx, ty, tz] = self.translation;
        [dot(a) + tx, dot(b) + ty, dot(c) + tz]
    }
}

/// 列の `index` 番目の値
fn value(column: Option<&Column>, index: usize) -> Option<f64> {
    column?.values.get(index).copied()
}

/// 状態の列が 0 でなければ無効
fn flagged(column: Option<&Column>, index: usize) -> bool {
    value(column, index).is_some_and(|state| state != 0.0)
}

/// 幅の中の値を、16 ビットの幅に広げる
fn widen(value: f64, (min, max): (f64, f64)) -> u16 {
    if !value.is_finite() {
        return 0;
    }
    let unit = if max > min {
        (value - min) / (max - min)
    } else {
        value
    };
    libm::round(unit * 65535.0).clamp(0.0, 65535.0) as u16
}

/// 列の幅。決めた幅がなければ型の幅、それもなければ `default`
fn span(limits: Option<(f64, f64)>, column: Option<&Column>, default: (f64, f64)) -> (f64, f64) {
    limits
        .or(column.and_then(|column| column.range))
        .filter(|(min, max)| min.is_finite() && max.is_finite() && max > min)
        .unwrap_or(default)
}

/// 1 つのスキャンの点を、座標と属性の列に足す
#[derive(Default)]
struct Merged {
    points: Points,
    intensity: Vec<u16>,
    color: Vec<[u16; 3]>,
    returns: Vec<[u8; 2]>,
    time: Vec<f64>,
}

/// スキャンの座標。直交の座標がなければ球の座標から求める。点の座標がなければ `None`
fn position(scan: &Scan, index: usize) -> Option<Option<[f64; 3]>> {
    if let (Some(x), Some(y), Some(z)) = (
        scan.column(Field::X),
        scan.column(Field::Y),
        scan.column(Field::Z),
    ) {
        let invalid = flagged(scan.column(Field::CartesianInvalid), index);
        let point = [
            value(Some(x), index)?,
            value(Some(y), index)?,
            value(Some(z), index)?,
        ];
        return Some((!invalid).then_some(point));
    }
    let (Some(range), Some(azimuth), Some(elevation)) = (
        scan.column(Field::Range),
        scan.column(Field::Azimuth),
        scan.column(Field::Elevation),
    ) else {
        return None;
    };
    let invalid = flagged(scan.column(Field::SphericalInvalid), index);
    let (r, a, e) = (
        value(Some(range), index)?,
        value(Some(azimuth), index)?,
        value(Some(elevation), index)?,
    );
    let flat = r * libm::cos(e);
    let point = [flat * libm::cos(a), flat * libm::sin(a), r * libm::sin(e)];
    Some((!invalid).then_some(point))
}

impl Lift<PointCloud> for E57File {
    fn lift(&self, cx: &mut Cx) -> Result<PointCloud, Error> {
        let total: u64 = self.scans.iter().map(|scan| scan.read).sum();
        cx.check(Kind::Elements, total)?;
        // 属性は、座標を持つどのスキャンも持つときだけ移す
        let placed: Vec<(usize, &Scan)> = self
            .scans
            .iter()
            .enumerate()
            .filter(|(index, scan)| {
                let spherical = [Field::Range, Field::Azimuth, Field::Elevation];
                let cartesian = [Field::X, Field::Y, Field::Z];
                let has = |fields: [Field; 3]| fields.iter().all(|f| scan.column(*f).is_some());
                if has(cartesian) || has(spherical) {
                    return true;
                }
                cx.report(
                    Diagnostic::new(codes::E57_COORDINATES_MISSING).with("scan", *index as u64),
                );
                false
            })
            .collect();
        let every = |field: Field| {
            !placed.is_empty() && placed.iter().all(|(_, scan)| scan.column(field).is_some())
        };
        let any = |field: Field| placed.iter().any(|(_, scan)| scan.column(field).is_some());
        let has_intensity = every(Field::Intensity);
        let has_color = every(Field::Red) && every(Field::Green) && every(Field::Blue);
        let has_returns = every(Field::ReturnIndex) && every(Field::ReturnCount);
        let has_time = every(Field::Time);
        let mut dropped: BTreeSet<String> = BTreeSet::new();
        for (name, kept, field) in [
            ("intensity", has_intensity, Field::Intensity),
            ("color", has_color, Field::Red),
            ("returns", has_returns, Field::ReturnIndex),
            ("time", has_time, Field::Time),
        ] {
            if !kept && any(field) {
                dropped.insert(name.into());
            }
        }
        for (_, scan) in &placed {
            dropped.extend(scan.dropped.iter().cloned());
        }
        let mut merged = Merged::default();
        let mut skipped = 0u64;
        for (_, scan) in &placed {
            let pose = scan.pose.map(Pose::new);
            let intensity_span = span(
                scan.intensity_limits,
                scan.column(Field::Intensity),
                (0.0, 1.0),
            );
            let channels = [Field::Red, Field::Green, Field::Blue];
            let color_spans: [(f64, f64); 3] = core::array::from_fn(|channel| {
                span(
                    scan.color_limits
                        .and_then(|limits| limits.get(channel).copied()),
                    channels.get(channel).and_then(|field| scan.column(*field)),
                    (0.0, 255.0),
                )
            });
            for index in 0..usize::try_from(scan.read).unwrap_or(usize::MAX) {
                cx.check(Kind::Work, 1)?;
                let Some(Some(point)) = position(scan, index) else {
                    skipped += 1;
                    continue;
                };
                if !point.iter().all(|v| v.is_finite()) {
                    skipped += 1;
                    continue;
                }
                let [x, y, z] = pose.as_ref().map_or(point, |pose| pose.apply(point));
                // X を東、Y を北にする（LAS と同じ）
                merged.points.easting.push(x);
                merged.points.northing.push(y);
                merged.points.height.push(z);
                if has_intensity {
                    let raw = value(scan.column(Field::Intensity), index).unwrap_or(f64::NAN);
                    let invalid = flagged(scan.column(Field::IntensityInvalid), index);
                    merged.intensity.push(if invalid {
                        0
                    } else {
                        widen(raw, intensity_span)
                    });
                }
                if has_color {
                    let invalid = flagged(scan.column(Field::ColorInvalid), index);
                    let rgb: [u16; 3] = core::array::from_fn(|channel| {
                        let field = channels.get(channel).copied().unwrap_or(Field::Red);
                        let raw = value(scan.column(field), index).unwrap_or(f64::NAN);
                        let range = color_spans.get(channel).copied().unwrap_or((0.0, 255.0));
                        if invalid { 0 } else { widen(raw, range) }
                    });
                    merged.color.push(rgb);
                }
                if has_returns {
                    let to_u8 = |v: f64| {
                        if v.is_finite() {
                            libm::round(v).clamp(0.0, 255.0) as u8
                        } else {
                            0
                        }
                    };
                    // E57 の番号は 0 から、領域モデルは LAS と同じく 1 から数える
                    let number =
                        value(scan.column(Field::ReturnIndex), index).map_or(0, |v| to_u8(v + 1.0));
                    let count = value(scan.column(Field::ReturnCount), index).map_or(0, to_u8);
                    merged.returns.push([number, count]);
                }
                if has_time {
                    let raw = value(scan.column(Field::Time), index).unwrap_or(0.0);
                    let invalid = flagged(scan.column(Field::TimeInvalid), index);
                    merged.time.push(if invalid || !raw.is_finite() {
                        0.0
                    } else {
                        raw
                    });
                }
            }
        }
        if skipped > 0 {
            cx.report(Diagnostic::new(codes::E57_POINT_DROPPED).with("points", skipped));
        }
        for name in dropped {
            cx.report(Diagnostic::new(codes::E57_ATTRIBUTE_DROPPED).with("attribute", name));
        }
        if self.images > 0 {
            cx.report(Diagnostic::new(codes::E57_IMAGE_DROPPED).with("images", self.images as u64));
        }
        let Merged {
            mut points,
            intensity,
            color,
            returns,
            time,
        } = merged;
        points.intensity = has_intensity.then_some(intensity);
        points.color = has_color.then_some(color);
        points.returns = has_returns.then_some(returns);
        points.time = has_time.then_some(time);
        let (crs, vertical) = crs(self.coordinates.as_deref(), cx);
        let octree = Octree::build(&points, cx)?;
        Ok(PointCloud {
            crs,
            vertical,
            points,
            octree,
            source: Ref::at_bytes(0, self.size),
        })
    }
}

/// 水平と高さの座標系。扱えなければ知らせて `None` にする
fn crs(text: Option<&str>, cx: &mut Cx<'_>) -> (Option<Crs>, Option<VerticalCrs>) {
    let Some(text) = text.map(str::trim).filter(|text| !text.is_empty()) else {
        cx.report(Diagnostic::new(codes::E57_CRS_MISSING));
        return (None, None);
    };
    let (mut horizontal, vertical) = zumen_crs::wkt_codes(text);
    // WKT でなければ、EPSG のコードの書き方（`EPSG:6677` など）として読む
    if horizontal.is_empty() && vertical.is_empty() {
        if let Some(crs) = Crs::from_name(text) {
            horizontal.push(crs.epsg());
        } else if let Some(height) = VerticalCrs::from_name(text) {
            return (None, Some(height));
        }
    }
    // 点は地図の空間に置くので、経緯度の座標系は受けない
    let plane = horizontal
        .iter()
        .find_map(|code| Crs::from_epsg(*code).filter(|crs| matches!(crs, Crs::Plane(_))));
    let crs = match (plane, horizontal.first()) {
        (Some(crs), _) => Some(crs),
        (None, Some(code)) => {
            cx.report(Diagnostic::new(codes::E57_CRS_UNSUPPORTED).with("epsg", u64::from(*code)));
            None
        }
        (None, None) => {
            cx.report(Diagnostic::new(codes::E57_CRS_INVALID));
            None
        }
    };
    let height = vertical
        .iter()
        .find_map(|code| VerticalCrs::from_epsg(*code));
    if let (None, Some(code)) = (height, vertical.first()) {
        cx.report(
            Diagnostic::new(codes::E57_VERTICAL_CRS_UNSUPPORTED).with("epsg", u64::from(*code)),
        );
    }
    (crs, height)
}
