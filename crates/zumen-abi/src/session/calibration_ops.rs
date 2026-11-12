//! 校正を選んだ、用紙上の距離計測。原点と差分を別々にコアへ渡す。

use super::*;
use crate::messages::{
    CalibrationItem, CalibrationsRequest, CalibrationsResult, MeasureDistanceRequest,
    MeasureDistanceResult, SplitPoint2,
};
use zumen_drawing::{Calibration, MeasurementError, SheetId, SplitPaperPoint};

const PAGE_LIMIT: u64 = 1000;
const SHEET_BYTES: u64 = 1024;
const ITEM_BYTES: u64 = 2048;
// 固定長の二進整数の領域比較・差分・丸めを、計算前に多めに数える。
const MEASUREMENT_WORK: u64 = 4096;

impl Session {
    pub(super) fn calibrations(
        &self,
        request: &CalibrationsRequest,
        cx: &mut Cx<'_>,
    ) -> Result<CalibrationsResult, Failed> {
        let (file, hash) = self.drawing_file(&request.doc, cx)?;
        let hash = checked_hash(hash, request.expected_hash.as_deref(), cx)?;
        let drawing = file.lift(cx).map_err(|_| Failed)?;
        let (sheets, wanted) = sheet_names(&drawing, request.from.as_deref(), cx)?;
        let calibrations = drawing.calibrations(cx).map_err(|stop| stopped(cx, stop))?;
        let offset = request.offset.unwrap_or(0);
        let end = offset.saturating_add(request.limit.unwrap_or(PAGE_LIMIT));
        let mut total = 0_u64;
        let mut items = Vec::new();
        for calibration in calibrations {
            cx.check(Kind::Work, 1).map_err(|stop| stopped(cx, stop))?;
            if wanted.is_some_and(|sheet| sheet != calibration.sheet) {
                continue;
            }
            if offset <= total && total < end {
                cx.check(Kind::DecodedBytes, ITEM_BYTES)
                    .map_err(|stop| stopped(cx, stop))?;
                let from = sheets
                    .get(&calibration.sheet)
                    .ok_or_else(|| fail(cx, invalid_value("/from")))?
                    .clone();
                items.push(CalibrationItem {
                    id: calibration.id,
                    name: calibration.name,
                    from,
                    bounds: calibration.bounds.to_vec(),
                    matrix: calibration.matrix.to_vec(),
                    unit: unit_of(calibration.unit),
                    unit_label: calibration.unit_label,
                    source: source(&calibration.source),
                });
            }
            total = total.saturating_add(1);
        }
        cx.check(Kind::Work, 0).map_err(|stop| stopped(cx, stop))?;
        Ok(CalibrationsResult {
            hash,
            items,
            total,
            next_offset: (end < total).then_some(end),
        })
    }

    pub(super) fn measure_distance(
        &self,
        request: &MeasureDistanceRequest,
        cx: &mut Cx<'_>,
    ) -> Result<MeasureDistanceResult, Failed> {
        let (file, hash) = self.drawing_file(&request.doc, cx)?;
        let hash = checked_hash(hash, request.expected_hash.as_deref(), cx)?;
        let drawing = file.lift(cx).map_err(|_| Failed)?;
        let (_, sheet) = sheet_names(&drawing, Some(&request.from), cx)?;
        let sheet = sheet.ok_or_else(|| fail(cx, invalid_value("/from")))?;
        let calibrations = drawing.calibrations(cx).map_err(|stop| stopped(cx, stop))?;
        let mut selected = None;
        for calibration in calibrations {
            cx.check(Kind::Work, 1 + (calibration.id.len() as u64).div_ceil(16))
                .map_err(|stop| stopped(cx, stop))?;
            if calibration.id == request.calibration {
                selected = Some(calibration);
                break;
            }
        }
        let calibration = selected.ok_or_else(|| fail(cx, invalid_value("/calibration")))?;
        if calibration.sheet != sheet {
            return Err(measurement_failed(
                &calibration,
                MeasurementError::SheetMismatch,
                cx,
            )?);
        }
        cx.check(Kind::Work, MEASUREMENT_WORK)
            .map_err(|stop| stopped(cx, stop))?;
        let point = |value: &SplitPoint2| SplitPaperPoint {
            sheet,
            origin: [value.origin.x, value.origin.y],
            offset: [value.offset.x, value.offset.y],
        };
        let distance = match calibration.measure_split(point(&request.start), point(&request.end)) {
            Ok(distance) => distance,
            Err(error) => return Err(measurement_failed(&calibration, error, cx)?),
        };
        cx.check(Kind::DecodedBytes, ITEM_BYTES)
            .map_err(|stop| stopped(cx, stop))?;
        cx.check(Kind::Work, 0).map_err(|stop| stopped(cx, stop))?;
        Ok(MeasureDistanceResult {
            hash,
            from: request.from.clone(),
            calibration: calibration.id,
            distance,
            unit: unit_of(calibration.unit),
            unit_label: calibration.unit_label,
        })
    }
}

/// 元の文書のハッシュ。図面バイナリでは埋め込んだ Source に由来する。
fn checked_hash(hash: Hash, expected: Option<&str>, cx: &mut Cx<'_>) -> Result<String, Failed> {
    cx.check(Kind::Work, 4).map_err(|stop| stopped(cx, stop))?;
    cx.check(Kind::DecodedBytes, 128)
        .map_err(|stop| stopped(cx, stop))?;
    let hash = hash.to_string();
    if expected.is_some_and(|expected| expected != hash) {
        return Err(fail(cx, invalid_value("/expected_hash")));
    }
    Ok(hash)
}

type SheetNames = BTreeMap<SheetId, String>;

/// 公開 ID は全用紙を順に通して作る。内部の SheetId の数字から推定しない。
fn sheet_names(
    drawing: &Drawing2D,
    wanted: Option<&str>,
    cx: &mut Cx<'_>,
) -> Result<(SheetNames, Option<SheetId>), Failed> {
    let mut names = BTreeMap::new();
    let mut namer = id::SheetNamer::default();
    let mut selected = None;
    for sheet in &drawing.sheets {
        cx.check(Kind::Work, 8).map_err(|stop| stopped(cx, stop))?;
        cx.check(Kind::DecodedBytes, SHEET_BYTES)
            .map_err(|stop| stopped(cx, stop))?;
        let name = namer.next(sheet);
        if wanted == Some(name.as_str()) {
            selected = Some(sheet.id);
        }
        names.entry(sheet.id).or_insert(name);
    }
    if wanted.is_some() && selected.is_none() {
        return Err(fail(cx, invalid_value("/from")));
    }
    Ok((names, selected))
}

fn source(reference: &Ref) -> Option<Source> {
    let source = Source {
        record: reference.record(),
        bytes: reference.bytes().map(|bytes| ByteRange {
            start: bytes.start,
            end: bytes.end,
        }),
        handle: match reference.key() {
            Some(Key::Handle(handle)) => Some(format!("{handle:X}")),
            _ => None,
        },
        instance: match reference.key() {
            Some(Key::Instance(number)) => Some(number.to_string()),
            _ => None,
        },
    };
    (source.record.is_some()
        || source.bytes.is_some()
        || source.handle.is_some()
        || source.instance.is_some())
    .then_some(source)
}

fn measurement_failed(
    calibration: &Calibration,
    error: MeasurementError,
    cx: &mut Cx<'_>,
) -> Result<Failed, Failed> {
    let reason = match error {
        MeasurementError::SheetMismatch => "sheet-mismatch",
        MeasurementError::NonFinitePoint => "non-finite-point",
        MeasurementError::OutsideBounds => "outside-bounds",
        MeasurementError::InvalidBounds => "invalid-bounds",
        MeasurementError::NonFiniteMatrix => "non-finite-matrix",
        MeasurementError::SingularMatrix => "singular-matrix",
        MeasurementError::Overflow => "overflow",
    };
    let bytes = (calibration.id.len() as u64)
        .saturating_add(calibration.source.id().map_or(0, |id| id.len() as u64));
    cx.check(Kind::Work, 1 + bytes.div_ceil(16))
        .map_err(|stop| stopped(cx, stop))?;
    cx.check(
        Kind::DecodedBytes,
        ITEM_BYTES.saturating_add(bytes.saturating_mul(2)),
    )
    .map_err(|stop| stopped(cx, stop))?;
    Ok(fail(
        cx,
        Diagnostic::new(codes::DRAWING_MEASUREMENT_FAILED)
            .with("id", calibration.id.as_str())
            .with("reason", reason)
            .at(calibration.source.clone()),
    ))
}
