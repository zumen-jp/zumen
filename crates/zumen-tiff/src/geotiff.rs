//! OGC GeoTIFF 1.1（`19-008r4`）のタグ解釈とアフィン変換。

use zumen_base::{Diagnostic, Ref, codes};
use zumen_crs::{Crs, VerticalCrs};
use zumen_sdk::Cx;

/// GeoTIFF のセル基準（`GTRasterTypeGeoKey` = 1025）
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RasterType {
    /// セルが面積を表す（`1`、既定）
    PixelIsArea,
    /// セルの格子点が標本値を表す（`2`）
    PixelIsPoint,
}

/// 画素座標 `(col, row)` からモデル座標 `(x, y)` への 2 次元アフィン変換。
///
/// `x = m[0] + m[1] * col + m[2] * row`（東・経度）、
/// `y = m[3] + m[4] * col + m[5] * row`（北・緯度）。
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GeoTransform {
    /// 6 係数 `[x0, dx_dcol, dx_drow, y0, dy_dcol, dy_drow]`
    pub matrix: [f64; 6],
}

impl GeoTransform {
    /// 連続画素座標 `(col, row)` をモデル座標 `(x, y)` に写す
    pub fn apply(self, col: f64, row: f64) -> Option<(f64, f64)> {
        let [m0, m1, m2, m3, m4, m5] = self.matrix;
        let x = norm_zero(m0 + m1 * col + m2 * row);
        let y = norm_zero(m3 + m4 * col + m5 * row);
        (x.is_finite() && y.is_finite()).then_some((x, y))
    }

    /// セル `(col, row)` の代表点のモデル座標 `(x, y)`
    pub fn cell_center(self, col: u32, row: u32, raster_type: RasterType) -> Option<(f64, f64)> {
        let shift = match raster_type {
            RasterType::PixelIsArea => 0.5,
            RasterType::PixelIsPoint => 0.0,
        };
        self.apply(f64::from(col) + shift, f64::from(row) + shift)
    }

    /// 画像全体（`width × height`）の外枠の四隅 `[左下, 右下, 右上, 左上]` のモデル座標 `(x, y)`。
    ///
    /// TIFF の画素行 `0` は上端、`height` は下端なので、
    /// - 左下（origin）は `(left, bottom)`
    /// - 右下（right）は `(right, bottom)`
    /// - 右上は `(right, top)`
    /// - 左上（top）は `(left, top)`
    pub fn footprint_corners(
        self,
        width: u32,
        height: u32,
        raster_type: RasterType,
    ) -> Option<[(f64, f64); 4]> {
        let (left, top_row, right, bottom_row) = match raster_type {
            RasterType::PixelIsArea => (0.0, 0.0, f64::from(width), f64::from(height)),
            RasterType::PixelIsPoint => {
                (-0.5, -0.5, f64::from(width) - 0.5, f64::from(height) - 0.5)
            }
        };
        let bl = self.apply(left, bottom_row)?;
        let br = self.apply(right, bottom_row)?;
        let tr = self.apply(right, top_row)?;
        let tl = self.apply(left, top_row)?;
        // 退化した外枠（面積 0）は除外する
        let area2 = (br.0 - bl.0) * (tl.1 - bl.1) - (br.1 - bl.1) * (tl.0 - bl.0);
        if !area2.is_finite() || libm::fabs(area2) <= 1e-18 {
            return None;
        }
        Some([bl, br, tr, tl])
    }

    /// TIFF の `Orientation`（`1..=8`）に合わせて、表示画素座標 `(disp_col, disp_row)` からモデル座標への変換に合成する
    pub(crate) fn with_orientation(
        self,
        orientation: u16,
        raw_w: u32,
        raw_h: u32,
        raster_type: RasterType,
    ) -> Option<Self> {
        if orientation <= 1 || orientation > 8 {
            return Some(self);
        }
        let max_w = match raster_type {
            RasterType::PixelIsArea => f64::from(raw_w),
            RasterType::PixelIsPoint => f64::from(raw_w) - 1.0,
        };
        let max_h = match raster_type {
            RasterType::PixelIsArea => f64::from(raw_h),
            RasterType::PixelIsPoint => f64::from(raw_h) - 1.0,
        };
        let (a0, a_u, a_v, b0, b_u, b_v) = match orientation {
            2 => (max_w, -1.0, 0.0, 0.0, 0.0, 1.0),
            3 => (max_w, -1.0, 0.0, max_h, 0.0, -1.0),
            4 => (0.0, 1.0, 0.0, max_h, 0.0, -1.0),
            5 => (0.0, 0.0, 1.0, 0.0, 1.0, 0.0),
            6 => (0.0, 0.0, 1.0, max_h, -1.0, 0.0),
            7 => (max_w, 0.0, -1.0, max_h, -1.0, 0.0),
            8 => (max_w, 0.0, -1.0, 0.0, 1.0, 0.0),
            _ => return Some(self),
        };
        let [m0, m1, m2, m3, m4, m5] = self.matrix;
        let matrix = [
            norm_zero(m0 + m1 * a0 + m2 * b0),
            norm_zero(m1 * a_u + m2 * b_u),
            norm_zero(m1 * a_v + m2 * b_v),
            norm_zero(m3 + m4 * a0 + m5 * b0),
            norm_zero(m4 * a_u + m5 * b_u),
            norm_zero(m4 * a_v + m5 * b_v),
        ];
        matrix
            .iter()
            .all(|v| v.is_finite())
            .then_some(Self { matrix })
    }
}

/// IFD から抽出した GeoTIFF メタデータ
#[derive(Clone, Debug, PartialEq)]
pub struct GeoInfo {
    /// 水平座標参照系
    pub crs: Option<Crs>,
    /// 未対応だった水平 EPSG コード（診断用）
    pub unsupported_epsg: Option<u32>,
    /// 鉛直座標参照系
    pub vertical_crs: Option<VerticalCrs>,
    /// セル基準
    pub raster_type: RasterType,
    /// アフィン変換
    pub transform: Option<GeoTransform>,
    /// `GDAL_NODATA`（42113）の数値
    pub nodata: Option<f64>,
}

/// IFD 内の生 GeoTIFF タグ群
#[derive(Default)]
pub(crate) struct RawGeoTags {
    pub pixel_scale: Option<Vec<f64>>,
    pub tiepoints: Option<Vec<f64>>,
    pub transformation: Option<Vec<f64>>,
    pub key_directory: Option<Vec<u16>>,
    pub double_params: Option<Vec<f64>>,
    pub ascii_params: Option<String>,
    pub gdal_nodata: Option<String>,
}

impl RawGeoTags {
    pub fn has_any(&self) -> bool {
        self.pixel_scale.is_some()
            || self.tiepoints.is_some()
            || self.transformation.is_some()
            || self.key_directory.is_some()
            || self.gdal_nodata.is_some()
    }

    pub fn parse(
        &self,
        orientation: u16,
        raw_w: u32,
        raw_h: u32,
        source: &Ref,
        cx: &mut Cx,
    ) -> Option<GeoInfo> {
        if !self.has_any() {
            return None;
        }

        let mut raster_type = RasterType::PixelIsArea;
        let mut model_type: Option<u16> = None;
        let mut projected_epsg: Option<u16> = None;
        let mut geographic_epsg: Option<u16> = None;
        let mut vertical_epsg: Option<u16> = None;

        if let Some(dir) = &self.key_directory
            && dir.len() >= 4
        {
            let num_keys = usize::from(dir.get(3).copied().unwrap_or(0));
            for i in 0..num_keys {
                let base = 4 + i.saturating_mul(4);
                let Some(entry) = dir.get(base..base.saturating_add(4)) else {
                    break;
                };
                let key_id = entry.first().copied().unwrap_or(0);
                let tiff_tag_location = entry.get(1).copied().unwrap_or(0);
                let count = entry.get(2).copied().unwrap_or(0);
                let value_offset = entry.get(3).copied().unwrap_or(0);

                match key_id {
                    1024 if tiff_tag_location == 0 && count == 1 => {
                        model_type = Some(value_offset);
                    }
                    1025 if tiff_tag_location == 0 && count == 1 => {
                        if value_offset == 2 {
                            raster_type = RasterType::PixelIsPoint;
                        } else if value_offset == 1 {
                            raster_type = RasterType::PixelIsArea;
                        }
                    }
                    2048 if tiff_tag_location == 0 && count == 1 => {
                        if value_offset != 0 && value_offset != 32767 {
                            geographic_epsg = Some(value_offset);
                        }
                    }
                    3072 if tiff_tag_location == 0 && count == 1 => {
                        if value_offset != 0 && value_offset != 32767 {
                            projected_epsg = Some(value_offset);
                        }
                    }
                    4096 if tiff_tag_location == 0 && count == 1 => {
                        if value_offset != 0 && value_offset != 32767 {
                            vertical_epsg = Some(value_offset);
                        }
                    }
                    // GeoTIFF Citation や単位などの標準キーは静かに保持・無視する
                    1026 | 2049 | 2050 | 2051 | 2052 | 2054 | 2056 | 2057 | 2058 | 2059 | 3073
                    | 3074 | 3075 | 3076 | 4097 | 4098 | 4099 => {
                        let _ = (&self.double_params, &self.ascii_params);
                    }
                    _ => {
                        cx.report(
                            Diagnostic::new(codes::TIFF_TAG_UNSUPPORTED)
                                .with("tag", u64::from(key_id))
                                .at(source.clone()),
                        );
                    }
                }
            }
        }

        // ModelType が 2（Geographic）なら経緯度キー、それ以外は投影キーを優先する
        let candidate_epsg = if model_type == Some(2) {
            geographic_epsg.or(projected_epsg)
        } else {
            projected_epsg.or(geographic_epsg)
        };

        let mut crs = None;
        let mut unsupported_epsg = None;
        if let Some(code16) = candidate_epsg {
            let code = u32::from(code16);
            if let Some(resolved) = Crs::from_epsg(code) {
                crs = Some(resolved);
            } else {
                unsupported_epsg = Some(code);
                cx.report(
                    Diagnostic::new(codes::TIFF_CRS_UNSUPPORTED)
                        .with("epsg", u64::from(code))
                        .at(source.clone()),
                );
            }
        }

        let vertical_crs = vertical_epsg.and_then(|c| VerticalCrs::from_epsg(u32::from(c)));
        let transform = parse_transform(
            self.transformation.as_deref(),
            self.pixel_scale.as_deref(),
            self.tiepoints.as_deref(),
        )
        .and_then(|tf| tf.with_orientation(orientation, raw_w, raw_h, raster_type));

        let nodata = if let Some(raw) = &self.gdal_nodata {
            let trimmed = raw.trim_matches(|c: char| c == '\0' || c.is_ascii_whitespace());
            if trimmed.is_empty() {
                None
            } else if let Ok(v) = trimmed.parse::<f64>() {
                Some(v)
            } else {
                cx.report(
                    Diagnostic::new(codes::TIFF_NODATA_INVALID)
                        .with("value", trimmed.to_string())
                        .at(source.clone()),
                );
                None
            }
        } else {
            None
        };

        Some(GeoInfo {
            crs,
            unsupported_epsg,
            vertical_crs,
            raster_type,
            transform,
            nodata,
        })
    }
}

fn parse_transform(
    transformation: Option<&[f64]>,
    pixel_scale: Option<&[f64]>,
    tiepoints: Option<&[f64]>,
) -> Option<GeoTransform> {
    if let Some(mat) = transformation
        && mat.len() >= 8
    {
        // OGC 19-008r4 Annex B: 4×4 行優先行列
        // X = m[0]*col + m[1]*row + m[3]
        // Y = m[4]*col + m[5]*row + m[7]
        let m0 = *mat.get(3)?;
        let m1 = *mat.first()?;
        let m2 = *mat.get(1)?;
        let m3 = *mat.get(7)?;
        let m4 = *mat.get(4)?;
        let m5 = *mat.get(5)?;
        let matrix = [m0, m1, m2, m3, m4, m5];
        if matrix.iter().all(|v| v.is_finite()) {
            let det = m1 * m5 - m2 * m4;
            if det.is_finite() && libm::fabs(det) > 1e-18 {
                return Some(GeoTransform { matrix });
            }
        }
    }

    let tp = tiepoints?;
    if tp.len() < 6 {
        return None;
    }
    let i0 = *tp.first()?;
    let j0 = *tp.get(1)?;
    let x0 = *tp.get(3)?;
    let y0 = *tp.get(4)?;

    if let Some(scale) = pixel_scale
        && scale.len() >= 2
    {
        let sx = *scale.first()?;
        let sy = *scale.get(1)?;
        if [i0, j0, x0, y0, sx, sy].iter().all(|v| v.is_finite()) && sx > 0.0 && sy > 0.0 {
            let origin_x = norm_zero(x0 - i0 * sx);
            let origin_y = norm_zero(y0 + j0 * sy);
            if origin_x.is_finite() && origin_y.is_finite() {
                return Some(GeoTransform {
                    matrix: [origin_x, sx, 0.0, origin_y, 0.0, -sy],
                });
            }
        }
    }

    // 3 点以上のタイポイントのみが与えられた場合の最小構成（軸平行の矩形タイポイント）
    if tp.len() >= 18 {
        let i1 = *tp.get(6)?;
        let j1 = *tp.get(7)?;
        let x1 = *tp.get(9)?;
        let y1 = *tp.get(10)?;
        let i2 = *tp.get(12)?;
        let j2 = *tp.get(13)?;
        let x2 = *tp.get(15)?;
        let y2 = *tp.get(16)?;
        let di1 = i1 - i0;
        let dj1 = j1 - j0;
        let di2 = i2 - i0;
        let dj2 = j2 - j0;
        let det = di1 * dj2 - di2 * dj1;
        if det.is_finite() && libm::fabs(det) > 1e-12 {
            let m1 = ((x1 - x0) * dj2 - (x2 - x0) * dj1) / det;
            let m2 = (di1 * (x2 - x0) - di2 * (x1 - x0)) / det;
            let m4 = ((y1 - y0) * dj2 - (y2 - y0) * dj1) / det;
            let m5 = (di1 * (y2 - y0) - di2 * (y1 - y0)) / det;
            let m0 = norm_zero(x0 - m1 * i0 - m2 * j0);
            let m3 = norm_zero(y0 - m4 * i0 - m5 * j0);
            let matrix = [m0, m1, m2, m3, m4, m5];
            if matrix.iter().all(|v| v.is_finite()) {
                return Some(GeoTransform { matrix });
            }
        }
    }

    None
}

pub(crate) fn norm_zero(v: f64) -> f64 {
    if v == 0.0 { 0.0 } else { v }
}
