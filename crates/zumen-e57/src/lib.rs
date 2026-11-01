//! E57（台帳の `e57`、ASTM E2807 の点群）を読み、点群の領域モデル（決定 0232）に移す。
//!
//! ページ・XML・点の圧縮は `e57` crate（MIT）で読み、点は生の値で受け取る。球の座標の換算・姿勢の当て方・強度と色の
//! 幅の換算は、どの環境でも同じ値になるよう `libm` で求める（`docs/formats/e57.md`）。

mod lift;

use std::io::Cursor;

use e57::{E57Reader, Record, RecordDataType, RecordName, RecordValue};
use zumen_base::{Diagnostic, Kind, codes};
use zumen_sdk::{Codec, Cx, Error, FileModel, RecordInfo, Signature};

/// E57 のコーデック
pub struct E57;

/// 点の値の欄
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Field {
    /// 直交の座標
    X,
    /// 直交の座標
    Y,
    /// 直交の座標
    Z,
    /// 直交の座標の状態。0 は有効、1 は向きだけ、2 は無効
    CartesianInvalid,
    /// 球の座標の距離
    Range,
    /// 球の座標の方位角（ラジアン）
    Azimuth,
    /// 球の座標の仰角（ラジアン）
    Elevation,
    /// 球の座標の状態。0 は有効
    SphericalInvalid,
    /// 強度
    Intensity,
    /// 強度の状態。0 は有効
    IntensityInvalid,
    /// 色の赤
    Red,
    /// 色の緑
    Green,
    /// 色の青
    Blue,
    /// 色の状態。0 は有効
    ColorInvalid,
    /// 反射の番号（0 から）
    ReturnIndex,
    /// 反射の数
    ReturnCount,
    /// 時刻
    Time,
    /// 時刻の状態。0 は有効
    TimeInvalid,
}

impl Field {
    /// 欄の名前の欄。領域モデルに持たない欄は、移さない属性の名前を `Err` で返す
    fn from_name(name: &RecordName) -> Result<Field, String> {
        Ok(match name {
            RecordName::CartesianX => Field::X,
            RecordName::CartesianY => Field::Y,
            RecordName::CartesianZ => Field::Z,
            RecordName::CartesianInvalidState => Field::CartesianInvalid,
            RecordName::SphericalRange => Field::Range,
            RecordName::SphericalAzimuth => Field::Azimuth,
            RecordName::SphericalElevation => Field::Elevation,
            RecordName::SphericalInvalidState => Field::SphericalInvalid,
            RecordName::Intensity => Field::Intensity,
            RecordName::IsIntensityInvalid => Field::IntensityInvalid,
            RecordName::ColorRed => Field::Red,
            RecordName::ColorGreen => Field::Green,
            RecordName::ColorBlue => Field::Blue,
            RecordName::IsColorInvalid => Field::ColorInvalid,
            RecordName::ReturnIndex => Field::ReturnIndex,
            RecordName::ReturnCount => Field::ReturnCount,
            RecordName::TimeStamp => Field::Time,
            RecordName::IsTimeStampInvalid => Field::TimeInvalid,
            RecordName::RowIndex => return Err("row-index".into()),
            RecordName::ColumnIndex => return Err("column-index".into()),
            RecordName::Unknown { namespace, name } => return Err(format!("{namespace}:{name}")),
        })
    }
}

/// 点の値の列
#[derive(Clone, Debug, PartialEq)]
pub struct Column {
    /// 欄
    pub field: Field,
    /// 型の最小と最大。浮動小数の型で書かなければ `None`
    pub range: Option<(f64, f64)>,
    /// 点ごとの値
    pub values: Vec<f64>,
}

/// 読んだスキャン（E57 の `data3D` の 1 つ）
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Scan {
    /// 名前
    pub name: Option<String>,
    /// 見出しの点の数
    pub records: u64,
    /// 読めた点の数
    pub read: u64,
    /// 値の列
    pub columns: Vec<Column>,
    /// 領域モデルに持たない欄の名前
    pub dropped: Vec<String>,
    /// 姿勢。回転の四元数（w・x・y・z）と移動（x・y・z）
    pub pose: Option<([f64; 4], [f64; 3])>,
    /// 強度の幅（最小と最大）
    pub intensity_limits: Option<(f64, f64)>,
    /// 色の幅（赤・緑・青ごとの最小と最大）
    pub color_limits: Option<[(f64, f64); 3]>,
    /// 点のデータの始まりの位置
    pub offset: u64,
}

impl Scan {
    /// 欄の列
    pub fn column(&self, field: Field) -> Option<&Column> {
        self.columns.iter().find(|column| column.field == field)
    }
}

/// 読んだ E57
#[derive(Clone, Debug, Default, PartialEq)]
pub struct E57File {
    /// スキャン
    pub scans: Vec<Scan>,
    /// 座標系の記述（`coordinateMetadata`。WKT か EPSG のコード）
    pub coordinates: Option<String>,
    /// 画像の数
    pub images: usize,
    /// ファイルの大きさ
    pub size: u64,
}

impl FileModel for E57File {
    fn record_count(&self) -> usize {
        self.scans.len()
    }

    fn record(&self, index: usize) -> Option<RecordInfo<'_>> {
        let scan = self.scans.get(index)?;
        Some(RecordInfo {
            kind: b"data3D",
            bytes: Some(zumen_base::ByteRange {
                start: scan.offset,
                end: scan.offset,
            }),
        })
    }
}

/// 型の最小と最大
fn type_range(data_type: &RecordDataType) -> Option<(f64, f64)> {
    match *data_type {
        RecordDataType::Single { min, max } => Some((f64::from(min?), f64::from(max?))),
        RecordDataType::Double { min, max } => Some((min?, max?)),
        RecordDataType::ScaledInteger {
            min,
            max,
            scale,
            offset,
        } => Some((min as f64 * scale + offset, max as f64 * scale + offset)),
        RecordDataType::Integer { min, max } => Some((min as f64, max as f64)),
    }
}

/// 幅の値を、その欄の型で f64 にする
fn limit(value: Option<&RecordValue>, prototype: &[Record], name: &RecordName) -> Option<f64> {
    let value = value?;
    let data_type = prototype
        .iter()
        .find(|record| &record.name == name)
        .map(|record| record.data_type.clone())
        .unwrap_or(RecordDataType::F64);
    match value {
        RecordValue::Single(v) => Some(f64::from(*v)),
        RecordValue::Double(v) => Some(*v),
        RecordValue::Integer(v) => Some(*v as f64),
        RecordValue::ScaledInteger(_) => value.to_f64(&data_type).ok(),
    }
}

/// 読めないファイルを知らせる
fn invalid(cx: &mut Cx<'_>, error: &e57::Error) -> Error {
    cx.report(Diagnostic::new(codes::E57_FILE_INVALID).with("reason", error.to_string()));
    Error::Invalid
}

impl Codec for E57 {
    type File = E57File;

    const FORMAT: &'static str = "e57";

    const SIGNATURES: &'static [Signature] = &[Signature::Bytes {
        offset: 0,
        bytes: b"ASTM-E57",
    }];

    fn read(src: &[u8], cx: &mut Cx) -> Result<E57File, Error> {
        cx.check(Kind::InputBytes, src.len() as u64)?;
        let mut reader = E57Reader::new(Cursor::new(src)).map_err(|error| invalid(cx, &error))?;
        let mut file = E57File {
            coordinates: reader.coordinate_metadata().map(str::to_string),
            images: reader.images().len(),
            size: src.len() as u64,
            ..E57File::default()
        };
        for cloud in reader.pointclouds() {
            // 確保する前に、見出しの点の数を数える
            cx.check(Kind::Records, cloud.records)?;
            let mut scan = Scan {
                name: cloud.name.clone(),
                records: cloud.records,
                offset: cloud.file_offset,
                pose: cloud.transform.as_ref().map(|pose| {
                    let (q, t) = (&pose.rotation, &pose.translation);
                    ([q.w, q.x, q.y, q.z], [t.x, t.y, t.z])
                }),
                ..Scan::default()
            };
            let prototype = &cloud.prototype;
            if let Some(limits) = &cloud.intensity_limits {
                let name = RecordName::Intensity;
                scan.intensity_limits = limit(limits.intensity_min.as_ref(), prototype, &name)
                    .zip(limit(limits.intensity_max.as_ref(), prototype, &name));
            }
            if let Some(limits) = &cloud.color_limits {
                let channel = |min: Option<&RecordValue>, max, name| {
                    limit(min, prototype, &name).zip(limit(max, prototype, &name))
                };
                let red = channel(
                    limits.red_min.as_ref(),
                    limits.red_max.as_ref(),
                    RecordName::ColorRed,
                );
                let green = channel(
                    limits.green_min.as_ref(),
                    limits.green_max.as_ref(),
                    RecordName::ColorGreen,
                );
                let blue = channel(
                    limits.blue_min.as_ref(),
                    limits.blue_max.as_ref(),
                    RecordName::ColorBlue,
                );
                scan.color_limits = red.zip(green).zip(blue).map(|((r, g), b)| [r, g, b]);
            }
            // 値の並びの位置ごとに、入れる列の位置
            let mut slots = Vec::with_capacity(prototype.len());
            for record in prototype {
                match Field::from_name(&record.name) {
                    Ok(field) => {
                        slots.push(Some(scan.columns.len()));
                        scan.columns.push(Column {
                            field,
                            range: type_range(&record.data_type),
                            values: Vec::new(),
                        });
                    }
                    Err(name) => {
                        slots.push(None);
                        scan.dropped.push(name);
                    }
                }
            }
            let points = reader
                .pointcloud_raw(&cloud)
                .map_err(|error| invalid(cx, &error))?;
            for point in points {
                let Ok(values) = point else {
                    break;
                };
                cx.check(Kind::Work, 1)?;
                for ((value, slot), record) in values.iter().zip(&slots).zip(prototype) {
                    let Some(column) = slot.and_then(|slot| scan.columns.get_mut(slot)) else {
                        continue;
                    };
                    column
                        .values
                        .push(value.to_f64(&record.data_type).unwrap_or(f64::NAN));
                }
                scan.read += 1;
            }
            if scan.read < scan.records {
                cx.report(
                    Diagnostic::new(codes::E57_POINTS_INVALID)
                        .with("expected", scan.records)
                        .with("found", scan.read),
                );
            }
            file.scans.push(scan);
        }
        Ok(file)
    }

    /// 書き出しはまだしない
    fn write(_file: &E57File, _out: &mut Vec<u8>, _cx: &mut Cx) -> Result<(), Error> {
        Err(Error::Unsupported)
    }
}
