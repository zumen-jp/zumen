//! XYZ（台帳の `xyz`、座標と属性を区切って並べた点群のテキスト）を読み、点群の領域モデル（決定 0232）に移す。
//!
//! 決まった仕様はないので、行ごとの数の列を推定で読む（`docs/formats/xyz.md`）。見分ける印がないので、判定の
//! 規則を持たず、`open` で形式を名指したときだけ読む。

use zumen_base::{Diagnostic, Kind, Ref, codes};
use zumen_pointcloud::{Octree, PointCloud, Points};
use zumen_sdk::{Codec, Cx, Error, FileModel, Lift, RecordInfo, Signature};

/// XYZ のコーデック
pub struct Xyz;

/// 読んだ XYZ。数の行を持つ
#[derive(Clone, Debug, Default, PartialEq)]
pub struct XyzFile {
    /// 列の数。最初の数の行で決める
    pub columns: usize,
    /// 値。行ごとに列の順で並べる
    pub values: Vec<f64>,
    /// 行の元のバイト範囲
    pub spans: Vec<(u64, u64)>,
}

impl FileModel for XyzFile {
    fn record_count(&self) -> usize {
        self.spans.len()
    }

    fn record(&self, index: usize) -> Option<RecordInfo<'_>> {
        let (start, end) = *self.spans.get(index)?;
        Some(RecordInfo {
            kind: b"point",
            bytes: Some(zumen_base::ByteRange { start, end }),
        })
    }
}

/// 列を区切る文字
fn separator(c: char) -> bool {
    c.is_ascii_whitespace() || c == ',' || c == ';'
}

impl Codec for Xyz {
    type File = XyzFile;

    const FORMAT: &'static str = "xyz";

    /// 見分ける印はない（決定 0232）
    const SIGNATURES: &'static [Signature] = &[];

    fn read(src: &[u8], cx: &mut Cx) -> Result<XyzFile, Error> {
        cx.check(Kind::InputBytes, src.len() as u64)?;
        let mut file = XyzFile::default();
        // 読めなかった行の数と、その最初の行の番号
        let (mut skipped, mut first_skipped) = (0u64, None);
        let mut at = 0usize;
        for (number, line) in src.split(|b| *b == b'\n').enumerate() {
            let start = at;
            at += line.len() + 1;
            cx.check(Kind::Work, 1)?;
            let text = String::from_utf8_lossy(line);
            let text = text.trim();
            // 空の行と注記（# か //）は飛ばす
            if text.is_empty() || text.starts_with('#') || text.starts_with("//") {
                continue;
            }
            let row: Option<Vec<f64>> = text
                .split(separator)
                .filter(|word| !word.is_empty())
                .map(|word| word.parse::<f64>().ok().filter(|v| v.is_finite()))
                .collect();
            let row = row
                .filter(|row| row.len() >= 3 && (file.columns == 0 || row.len() == file.columns));
            let Some(row) = row else {
                skipped += 1;
                first_skipped.get_or_insert(number as u64 + 1);
                continue;
            };
            if file.columns == 0 {
                file.columns = row.len();
            }
            cx.check(Kind::Records, 1)?;
            file.values.extend(row);
            file.spans.push((start as u64, (start + line.len()) as u64));
        }
        if let Some(first) = first_skipped {
            cx.report(
                Diagnostic::new(codes::XYZ_LINE_INVALID)
                    .with("line", first)
                    .with("lines", skipped),
            );
        }
        if file.columns == 0 {
            cx.report(Diagnostic::new(codes::XYZ_FILE_INVALID));
            return Err(Error::Invalid);
        }
        Ok(file)
    }

    /// 書き出しはまだしない
    fn write(_file: &XyzFile, _out: &mut Vec<u8>, _cx: &mut Cx) -> Result<(), Error> {
        Err(Error::Unsupported)
    }
}

impl Lift<PointCloud> for XyzFile {
    fn lift(&self, cx: &mut Cx) -> Result<PointCloud, Error> {
        let width = self.columns.max(1);
        let rows = self.values.chunks_exact(width);
        cx.check(Kind::Elements, rows.len() as u64)?;
        let get = |row: &[f64], index: usize| row.get(index).copied().unwrap_or(0.0);
        let mut points = Points {
            easting: rows.clone().map(|row| get(row, 0)).collect(),
            northing: rows.clone().map(|row| get(row, 1)).collect(),
            height: rows.clone().map(|row| get(row, 2)).collect(),
            ..Points::default()
        };
        // 列の数から属性を推定する: 4 は強度、6 は色、7 は強度と色
        let (intensity, color) = match self.columns {
            3 => (None, None),
            4 => (Some(3), None),
            6 => (None, Some(3)),
            7 => (Some(3), Some(4)),
            other => {
                cx.report(Diagnostic::new(codes::XYZ_COLUMN_DROPPED).with("columns", other as u64));
                (None, None)
            }
        };
        if let Some(index) = intensity {
            let values: Vec<f64> = rows.clone().map(|row| get(row, index)).collect();
            // 0〜1 に収まる強度は、16 ビットの幅に広げる
            let unit = values.iter().all(|v| (0.0..=1.0).contains(v))
                && values.iter().any(|v| v.fract() != 0.0);
            let scale = if unit { 65535.0 } else { 1.0 };
            points.intensity = Some(
                values
                    .iter()
                    .map(|v| libm::round(v * scale).clamp(0.0, 65535.0) as u16)
                    .collect(),
            );
        }
        if let Some(index) = color {
            let channels: Vec<[f64; 3]> = rows
                .clone()
                .map(|row| [get(row, index), get(row, index + 1), get(row, index + 2)])
                .collect();
            // どの値も 255 までなら 8 ビットの色とし、16 ビットに広げる
            let scale = if channels.iter().flatten().all(|v| *v <= 255.0) {
                257.0
            } else {
                1.0
            };
            points.color = Some(
                channels
                    .iter()
                    .map(|rgb| rgb.map(|v| libm::round(v * scale).clamp(0.0, 65535.0) as u16))
                    .collect(),
            );
        }
        cx.report(Diagnostic::new(codes::XYZ_CRS_MISSING));
        let octree = Octree::build(&points, cx)?;
        let end = self.spans.last().map_or(0, |span| span.1);
        Ok(PointCloud {
            crs: None,
            vertical: None,
            points,
            octree,
            source: Ref::at_bytes(self.spans.first().map_or(0, |span| span.0), end),
        })
    }
}
