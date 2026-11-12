//! 用紙の変位を、明示的に選んだ領域の校正単位へ写す。
//!
//! 描画用の空間やビューは作らず、既存の Document 拡張に保存する。
//! `drawing.calibration` の schema 1 は、平行移動や実座標の原点を定義しない。

use std::collections::BTreeMap;
use std::fmt;

use zumen_base::{Cx, Diagnostic, Kind, Ref, Stop, codes};

use crate::{
    Basis, Draft, Drawing2D, Extension, Field, FieldValue, PaperPoint, Position, SheetId, Target,
    Unit,
};

const NAMESPACE: &str = "drawing.calibration";
const DRAFT_BYTES: u64 = 4096;
const ENTRY_BYTES: u64 = 2048;
const INDEX_BYTES: u64 = 512;

/// 用紙上の閉じた矩形の、距離の校正。重なる領域は `id` で明示的に選ぶ。
#[derive(Clone, Debug, PartialEq)]
pub struct Calibration {
    /// 文書内で重複しない、空でない ID。拡張の `name` と同じ。
    pub id: String,
    /// 元の名前。ID や単位ラベルに置き換えない。
    pub name: Option<String>,
    /// 校正を使える用紙。
    pub sheet: SheetId,
    /// 用紙の mm の `[min_x, min_y, max_x, max_y]`。境界を含み、幅・高さは正。
    pub bounds: [f64; 4],
    /// 用紙の変位から校正単位の変位への、行優先の 2×2 行列。
    pub matrix: [f64; 4],
    /// 校正後の長さの単位。ラベルだけから既知の単位を推定しない。
    pub unit: Unit,
    /// 元の単位ラベル。未知の単位でも原文を保つ。
    pub unit_label: String,
    /// 元のレコード。
    pub source: Ref,
}

/// 校正を使えなかった理由。図面や校正の値は変更しない。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MeasurementError {
    /// 始点か終点が、校正とは異なる用紙にある。
    SheetMismatch,
    /// 始点か終点に有限でない座標がある。
    NonFinitePoint,
    /// 始点か終点が、校正の閉領域の外にある。
    OutsideBounds,
    /// 領域が有限でないか、幅か高さが正でない。
    InvalidBounds,
    /// 行列に有限でない成分がある。
    NonFiniteMatrix,
    /// 行列が特異で、変位の一部を失う。
    SingularMatrix,
    /// 差分・変換・距離の計算が有限な値に収まらない。
    Overflow,
}

impl fmt::Display for MeasurementError {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        out.write_str(match self {
            Self::SheetMismatch => "校正と異なる用紙の点です",
            Self::NonFinitePoint => "点の座標が有限でありません",
            Self::OutsideBounds => "点が校正の領域の外にあります",
            Self::InvalidBounds => "校正の領域が不正です",
            Self::NonFiniteMatrix => "校正の行列が有限でありません",
            Self::SingularMatrix => "校正の行列が特異です",
            Self::Overflow => "校正の計算が有限な値に収まりません",
        })
    }
}

impl std::error::Error for MeasurementError {}

impl Calibration {
    /// 所有している文字列を移して、schema 1 の Document 拡張の下書きへ符号化する。
    /// 値の検証・不正な拡張の除外は [`Drawing2D::calibrations`] で行う。
    pub fn into_draft(self, cx: &mut Cx<'_>) -> Result<Draft, Stop> {
        cx.check(Kind::Work, 8)?;
        cx.check(Kind::DecodedBytes, DRAFT_BYTES)?;
        let [min_x, min_y, max_x, max_y] = self.bounds;
        let [a, b, c, d] = self.matrix;
        let mut fields = Vec::with_capacity(7 + usize::from(self.name.is_some()));
        for (key, value) in [
            ("sheet", FieldValue::Integer(i64::from(self.sheet.0))),
            ("bounds_min", FieldValue::Point([min_x, min_y, 0.0])),
            ("bounds_max", FieldValue::Point([max_x, max_y, 0.0])),
            ("matrix_x", FieldValue::Point([a, b, 0.0])),
            ("matrix_y", FieldValue::Point([c, d, 0.0])),
            ("unit", FieldValue::Text(unit_name(self.unit).to_owned())),
            ("unit_label", FieldValue::Text(self.unit_label)),
        ] {
            fields.push(Field {
                key: key.into(),
                code: None,
                value,
            });
        }
        if let Some(name) = self.name {
            fields.push(Field {
                key: "name".into(),
                code: None,
                value: FieldValue::Text(name),
            });
        }
        Ok(Draft {
            namespace: NAMESPACE.into(),
            name: self.id,
            version: None,
            schema: 1,
            at: Position::Document,
            fields,
            source: self.source,
        })
    }

    /// 同じ用紙・閉領域内の 2 点を、校正単位で測る。原点や自動的な領域選択は使わない。
    pub fn measure(&self, start: PaperPoint, end: PaperPoint) -> Result<f64, MeasurementError> {
        validate(self.bounds, self.matrix)?;
        if start.sheet != self.sheet || end.sheet != self.sheet {
            return Err(MeasurementError::SheetMismatch);
        }
        if ![start.x, start.y, end.x, end.y]
            .iter()
            .all(|value| value.is_finite())
        {
            return Err(MeasurementError::NonFinitePoint);
        }
        let [min_x, min_y, max_x, max_y] = self.bounds;
        if [start, end]
            .iter()
            .any(|point| point.x < min_x || point.x > max_x || point.y < min_y || point.y > max_y)
        {
            return Err(MeasurementError::OutsideBounds);
        }
        let [a, b, c, d] = self.matrix;
        let (dx, dy) = (end.x - start.x, end.y - start.y);
        let (x, y) = (a * dx + b * dy, c * dx + d * dy);
        let distance = libm::hypot(x, y);
        if [dx, dy, x, y, distance]
            .iter()
            .all(|value| value.is_finite())
        {
            Ok(distance)
        } else {
            Err(MeasurementError::Overflow)
        }
    }
}

impl Drawing2D {
    /// schema 1 の校正を、拡張の順に返す。不正な値は理由と出所を診断し、返さない。
    /// 重複した校正 ID と、重複した用紙 ID を指す校正は、先勝ちにせず除外する。
    pub fn calibrations(&self, cx: &mut Cx<'_>) -> Result<Vec<Calibration>, Stop> {
        cx.check(Kind::Work, 0)?;
        let mut ids = BTreeMap::new();
        for extension in &self.extensions {
            cx.check(Kind::Work, 1)?;
            if extension.namespace != NAMESPACE {
                continue;
            }
            cx.check(Kind::Work, 1 + (extension.name.len() as u64).div_ceil(16))?;
            if let Some(duplicate) = ids.get_mut(extension.name.as_str()) {
                *duplicate = true;
            } else {
                cx.check(Kind::DecodedBytes, INDEX_BYTES)?;
                ids.insert(extension.name.as_str(), false);
            }
        }
        if ids.is_empty() {
            return Ok(Vec::new());
        }
        let mut sheets = BTreeMap::new();
        for sheet in &self.sheets {
            cx.check(Kind::Work, 1)?;
            if let Some(duplicate) = sheets.get_mut(&sheet.id) {
                *duplicate = true;
            } else {
                cx.check(Kind::DecodedBytes, INDEX_BYTES)?;
                sheets.insert(sheet.id, false);
            }
        }
        let mut out = Vec::new();
        for extension in &self.extensions {
            cx.check(Kind::Work, 1)?;
            if extension.namespace != NAMESPACE {
                continue;
            }
            cx.check(Kind::Work, 1 + (extension.name.len() as u64).div_ceil(16))?;
            let header = if extension.schema != 1 {
                Some("schema")
            } else if extension.target != Target::Document {
                Some("target")
            } else if extension.basis != Basis::Document {
                Some("basis")
            } else if extension.name.is_empty() {
                Some("id")
            } else if ids.get(extension.name.as_str()) == Some(&true) {
                Some("duplicate-id")
            } else {
                None
            };
            if let Some(reason) = header {
                invalid(extension, reason, cx)?;
                continue;
            }
            let parsed = match read_fields(extension, cx) {
                Ok(parsed) => parsed,
                Err(ReadError::Invalid(reason)) => {
                    invalid(extension, reason, cx)?;
                    continue;
                }
                Err(ReadError::Stopped(stop)) => return Err(stop),
            };
            if sheets.get(&parsed.sheet) != Some(&false) {
                invalid(extension, "sheet", cx)?;
                continue;
            }
            if let Err(error) = validate(parsed.bounds, parsed.matrix) {
                let reason = if error == MeasurementError::InvalidBounds {
                    "bounds"
                } else {
                    "matrix"
                };
                invalid(extension, reason, cx)?;
                continue;
            }
            let strings = (extension.name.len() as u64)
                .saturating_add(parsed.name.map_or(0, |name| name.len() as u64))
                .saturating_add(parsed.unit_label.len() as u64)
                .saturating_add(extension.source.id().map_or(0, |id| id.len() as u64));
            cx.check(Kind::Work, 1 + strings.div_ceil(16))?;
            cx.check(
                Kind::DecodedBytes,
                ENTRY_BYTES.saturating_add(strings.saturating_mul(2)),
            )?;
            out.push(Calibration {
                id: extension.name.clone(),
                name: parsed.name.map(str::to_owned),
                sheet: parsed.sheet,
                bounds: parsed.bounds,
                matrix: parsed.matrix,
                unit: parsed.unit,
                unit_label: parsed.unit_label.to_owned(),
                source: extension.source.clone(),
            });
        }
        cx.check(Kind::Work, 0)?;
        Ok(out)
    }
}

struct Parsed<'a> {
    name: Option<&'a str>,
    sheet: SheetId,
    bounds: [f64; 4],
    matrix: [f64; 4],
    unit: Unit,
    unit_label: &'a str,
}

enum ReadError {
    Invalid(&'static str),
    Stopped(Stop),
}

impl From<Stop> for ReadError {
    fn from(stop: Stop) -> Self {
        Self::Stopped(stop)
    }
}

fn read_fields<'a>(extension: &'a Extension, cx: &mut Cx<'_>) -> Result<Parsed<'a>, ReadError> {
    let (mut sheet, mut min, mut max, mut x, mut y, mut unit, mut label, mut name) =
        (None, None, None, None, None, None, None, None);
    for field in &extension.fields {
        cx.check(Kind::Work, 1)?;
        if field.code.is_some() {
            return Err(ReadError::Invalid("fields"));
        }
        match (field.key.as_ref(), &field.value) {
            ("sheet", FieldValue::Integer(value)) if sheet.is_none() => {
                sheet = Some(SheetId(
                    u32::try_from(*value).map_err(|_| ReadError::Invalid("sheet"))?,
                ));
            }
            ("bounds_min", FieldValue::Point([a, b, z])) if min.is_none() && *z == 0.0 => {
                min = Some([*a, *b])
            }
            ("bounds_max", FieldValue::Point([a, b, z])) if max.is_none() && *z == 0.0 => {
                max = Some([*a, *b])
            }
            ("matrix_x", FieldValue::Point([a, b, z])) if x.is_none() && *z == 0.0 => {
                x = Some([*a, *b])
            }
            ("matrix_y", FieldValue::Point([a, b, z])) if y.is_none() && *z == 0.0 => {
                y = Some([*a, *b])
            }
            ("unit", FieldValue::Text(value)) if unit.is_none() => {
                unit = Some(read_unit(value).ok_or(ReadError::Invalid("fields"))?)
            }
            ("unit_label", FieldValue::Text(value)) if label.is_none() => {
                label = Some(value.as_str())
            }
            ("name", FieldValue::Text(value)) if name.is_none() => name = Some(value.as_str()),
            _ => return Err(ReadError::Invalid("fields")),
        }
    }
    let missing = || ReadError::Invalid("fields");
    let ([min_x, min_y], [max_x, max_y]) = (min.ok_or_else(missing)?, max.ok_or_else(missing)?);
    let ([a, b], [c, d]) = (x.ok_or_else(missing)?, y.ok_or_else(missing)?);
    Ok(Parsed {
        name,
        sheet: sheet.ok_or_else(missing)?,
        bounds: [min_x, min_y, max_x, max_y],
        matrix: [a, b, c, d],
        unit: unit.ok_or_else(missing)?,
        unit_label: label.ok_or_else(missing)?,
    })
}

fn validate(bounds: [f64; 4], matrix: [f64; 4]) -> Result<(), MeasurementError> {
    let [min_x, min_y, max_x, max_y] = bounds;
    if !bounds.iter().all(|value| value.is_finite()) || min_x >= max_x || min_y >= max_y {
        return Err(MeasurementError::InvalidBounds);
    }
    if !matrix.iter().all(|value| value.is_finite()) {
        return Err(MeasurementError::NonFiniteMatrix);
    }
    let [a, b, c, d] = matrix;
    if product(a, d) == product(b, c) {
        return Err(MeasurementError::SingularMatrix);
    }
    Ok(())
}

/// 有限な binary64 の積を整数の仮数と指数で比べる。行列式の overflow・underflow・丸め消去を避ける。
fn product(left: f64, right: f64) -> (bool, u128, i32) {
    let parts = |value: f64| {
        let bits = value.to_bits();
        let exponent = ((bits >> 52) & 0x7ff) as i32;
        let fraction = bits & ((1_u64 << 52) - 1);
        if exponent == 0 {
            (fraction, -1074)
        } else {
            (fraction | (1_u64 << 52), exponent - 1075)
        }
    };
    let (a, ax) = parts(left);
    let (b, bx) = parts(right);
    let value = u128::from(a) * u128::from(b);
    if value == 0 {
        return (false, 0, 0);
    }
    let zeros = value.trailing_zeros();
    (
        (left.to_bits() ^ right.to_bits()) >> 63 != 0,
        value >> zeros,
        ax + bx + zeros as i32,
    )
}

fn invalid(extension: &Extension, reason: &'static str, cx: &mut Cx<'_>) -> Result<(), Stop> {
    let strings = (extension.name.len() as u64)
        .saturating_add(extension.source.id().map_or(0, |id| id.len() as u64));
    cx.check(Kind::Work, 1 + strings.div_ceil(16))?;
    cx.check(
        Kind::DecodedBytes,
        ENTRY_BYTES.saturating_add(strings.saturating_mul(2)),
    )?;
    cx.report(
        Diagnostic::new(codes::DRAWING_CALIBRATION_INVALID)
            .with("id", extension.name.as_str())
            .with("reason", reason)
            .at(extension.source.clone()),
    );
    cx.check(Kind::Work, 0)
}

fn unit_name(unit: Unit) -> &'static str {
    match unit {
        Unit::Millimeter => "millimeter",
        Unit::Centimeter => "centimeter",
        Unit::Meter => "meter",
        Unit::Inch => "inch",
        Unit::Foot => "foot",
        Unit::Unknown => "unknown",
    }
}

fn read_unit(value: &str) -> Option<Unit> {
    match value {
        "millimeter" => Some(Unit::Millimeter),
        "centimeter" => Some(Unit::Centimeter),
        "meter" => Some(Unit::Meter),
        "inch" => Some(Unit::Inch),
        "foot" => Some(Unit::Foot),
        "unknown" => Some(Unit::Unknown),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Element, Hash, Inherit, LayerId, PaperLength, Shape, Sheet, SheetItem, Style, id};
    use zumen_base::{Budget, CancelToken, Diagnostics, Limit, Limits, Value};

    fn point(x: f64, y: f64) -> PaperPoint {
        PaperPoint {
            sheet: SheetId(0),
            x,
            y,
        }
    }

    fn calibration() -> Calibration {
        Calibration {
            id: "vp/0".to_owned(),
            name: Some("平面図".to_owned()),
            sheet: SheetId(0),
            bounds: [0.0, 0.0, 100.0, 100.0],
            matrix: [1.0, 0.0, 0.0, 1.0],
            unit: Unit::Unknown,
            unit_label: "元の単位".to_owned(),
            source: Ref::at_record(7).with_id("measurement"),
        }
    }

    fn extension(value: Calibration) -> Extension {
        let mut budget = Budget::default();
        let mut diag = Diagnostics::new();
        let cancel = CancelToken::new();
        let draft = value
            .into_draft(&mut Cx::new(&mut budget, &mut diag, &cancel))
            .unwrap();
        assert_eq!(draft.at, Position::Document);
        Extension {
            namespace: draft.namespace,
            name: draft.name,
            version: draft.version,
            schema: draft.schema,
            target: Target::Document,
            basis: Basis::Document,
            fields: draft.fields,
            source: draft.source,
        }
    }

    fn drawing(extensions: Vec<Extension>) -> Drawing2D {
        Drawing2D {
            sheets: vec![Sheet {
                id: SheetId(0),
                name: Some("用紙".to_owned()),
                size: Some([PaperLength(100.0); 2]),
                elements: vec![Element {
                    source: Ref::at_record(2),
                    layer: LayerId(0),
                    style: Style {
                        color: Inherit::Layer,
                        line_type: Inherit::Layer,
                        line_type_scale: 1.0,
                        line_weight: None,
                        transparency: Inherit::Layer,
                    },
                    shape: Shape::Line {
                        start: point(3.0, 4.0),
                        end: point(6.0, 8.0),
                    },
                }],
                views: Vec::new(),
                order: vec![SheetItem::Element(0)],
                source: Ref::at_record(1),
            }],
            extensions,
            ..Drawing2D::default()
        }
    }

    fn loaded(
        drawing: &Drawing2D,
        limits: Limits,
        cancel: &CancelToken,
    ) -> (Result<Vec<Calibration>, Stop>, Budget, Vec<Diagnostic>) {
        let mut budget = Budget::new(limits);
        let mut diag = Diagnostics::new();
        let result = drawing.calibrations(&mut Cx::new(&mut budget, &mut diag, cancel));
        (result, budget, diag.into_vec())
    }

    #[test]
    fn encodes_owned_strings_and_round_trips_all_units_and_optional_names() {
        for unit in [
            Unit::Millimeter,
            Unit::Centimeter,
            Unit::Meter,
            Unit::Inch,
            Unit::Foot,
            Unit::Unknown,
        ] {
            for name in [None, Some(String::new()), Some("測距領域".to_owned())] {
                let expected = Calibration {
                    unit,
                    name,
                    ..calibration()
                };
                let value = expected.clone();
                let id_pointer = value.id.as_ptr();
                let label_pointer = value.unit_label.as_ptr();
                let name_pointer = value.name.as_ref().map(|name| name.as_ptr());
                let encoded = extension(value);
                assert_eq!(encoded.name.as_ptr(), id_pointer);
                assert!(
                    matches!(&encoded.fields[6].value, FieldValue::Text(text) if text.as_ptr() == label_pointer)
                );
                if let Some(pointer) = name_pointer {
                    assert!(
                        matches!(&encoded.fields[7].value, FieldValue::Text(text) if text.as_ptr() == pointer)
                    );
                }
                assert_eq!(encoded.namespace, NAMESPACE);
                assert_eq!(encoded.schema, 1);
                let (result, _, diagnostics) = loaded(
                    &drawing(vec![encoded]),
                    Limits::default(),
                    &CancelToken::new(),
                );
                assert_eq!(result.unwrap(), [expected]);
                assert!(diagnostics.is_empty());
            }
        }
    }

    #[test]
    fn measures_anisotropic_sheared_rotated_and_mirrored_displacements() {
        for (matrix, end, expected) in [
            ([1.0, 0.0, 0.0, 1.0], [3.0, 4.0], 5.0),
            ([2.0, 0.0, 0.0, 3.0], [3.0, 4.0], 13.416407864998739),
            ([0.0, -2.0, 3.0, 0.0], [4.0, 3.0], 13.416407864998739),
            ([1.0, 1.0, 0.0, 1.0], [1.0, 2.0], 3.605551275463989),
            ([-1.0, 0.0, 0.0, 1.0], [3.0, 4.0], 5.0),
        ] {
            let calibration = Calibration {
                matrix,
                ..calibration()
            };
            let distance = calibration
                .measure(point(0.0, 0.0), point(end[0], end[1]))
                .unwrap();
            assert!(
                (distance - expected).abs() < 1e-12,
                "{matrix:?}: {distance}"
            );
            assert_eq!(
                calibration.measure(point(42.0, 42.0), point(42.0, 42.0)),
                Ok(0.0)
            );
        }
        // 閉じた領域の四辺を含め、未知の単位でも数値とラベルを変えない。
        let value = calibration();
        assert!(value.measure(point(0.0, 0.0), point(100.0, 100.0)).is_ok());
        assert_eq!(value.unit, Unit::Unknown);
        assert_eq!(value.unit_label, "元の単位");
    }

    #[test]
    fn distinguishes_invalid_points_regions_matrices_and_overflow() {
        let value = calibration();
        let other = PaperPoint {
            sheet: SheetId(1),
            ..point(1.0, 1.0)
        };
        assert_eq!(
            value.measure(other, point(1.0, 1.0)),
            Err(MeasurementError::SheetMismatch)
        );
        assert_eq!(
            value.measure(point(1.0, 1.0), other),
            Err(MeasurementError::SheetMismatch)
        );
        assert_eq!(
            value.measure(point(f64::NAN, 1.0), point(1.0, 1.0)),
            Err(MeasurementError::NonFinitePoint)
        );
        assert_eq!(
            value.measure(point(1.0, 1.0), point(1.0, f64::INFINITY)),
            Err(MeasurementError::NonFinitePoint)
        );
        assert_eq!(
            value.measure(point(-1.0, 0.0), point(1.0, 1.0)),
            Err(MeasurementError::OutsideBounds)
        );
        assert_eq!(
            value.measure(point(0.0, 0.0), point(101.0, 1.0)),
            Err(MeasurementError::OutsideBounds)
        );
        for bounds in [
            [0.0, 0.0, 0.0, 10.0],
            [0.0, 10.0, 10.0, 0.0],
            [0.0, 0.0, f64::INFINITY, 10.0],
        ] {
            assert_eq!(
                Calibration {
                    bounds,
                    ..calibration()
                }
                .measure(point(0.0, 0.0), point(1.0, 1.0)),
                Err(MeasurementError::InvalidBounds)
            );
        }
        assert_eq!(
            Calibration {
                matrix: [f64::NAN, 0.0, 0.0, 1.0],
                ..calibration()
            }
            .measure(point(0.0, 0.0), point(1.0, 1.0)),
            Err(MeasurementError::NonFiniteMatrix)
        );
        for matrix in [[0.0; 4], [1.0, 2.0, 2.0, 4.0], [f64::MAX; 4]] {
            assert_eq!(
                Calibration {
                    matrix,
                    ..calibration()
                }
                .measure(point(0.0, 0.0), point(1.0, 1.0)),
                Err(MeasurementError::SingularMatrix)
            );
        }
        assert_eq!(
            Calibration {
                matrix: [f64::MAX, 0.0, 0.0, 1.0],
                ..calibration()
            }
            .measure(point(0.0, 0.0), point(2.0, 1.0)),
            Err(MeasurementError::Overflow)
        );
        let wide = Calibration {
            bounds: [-f64::MAX, 0.0, f64::MAX, 10.0],
            ..calibration()
        };
        assert_eq!(
            wide.measure(point(-f64::MAX, 0.0), point(f64::MAX, 1.0)),
            Err(MeasurementError::Overflow)
        );
    }

    #[test]
    fn does_not_mistake_extreme_or_near_singular_matrices_for_singular_ones() {
        let epsilon = f64::EPSILON;
        for matrix in [
            [1e308, 0.0, 0.0, 1e-308],
            [1e308, 1e-308, 1e308, 2e-308],
            [1.0, 1.0 + epsilon, 1.0 - epsilon, 1.0],
            [f64::from_bits(1), 0.0, 0.0, f64::from_bits(1)],
        ] {
            assert_eq!(
                validate([0.0, 0.0, 100.0, 100.0], matrix),
                Ok(()),
                "{matrix:?}"
            );
            let loaded = loaded(
                &drawing(vec![extension(Calibration {
                    matrix,
                    ..calibration()
                })]),
                Limits::default(),
                &CancelToken::new(),
            );
            assert_eq!(loaded.0.unwrap().len(), 1);
            assert!(loaded.2.is_empty());
        }
        let value = Calibration {
            matrix: [1e308, 1e-308, 1e308, 2e-308],
            ..calibration()
        };
        assert!(
            (value.measure(point(0.0, 0.0), point(1e-308, 0.0)).unwrap()
                - std::f64::consts::SQRT_2)
                .abs()
                < 1e-15
        );
    }

    #[test]
    fn filters_bad_headers_field_types_and_missing_sheet_references() {
        for (case, reason) in [
            (0, "schema"),
            (1, "target"),
            (2, "basis"),
            (3, "id"),
            (4, "fields"),
            (5, "fields"),
            (6, "fields"),
            (7, "fields"),
            (8, "fields"),
            (9, "bounds"),
            (10, "matrix"),
            (11, "sheet"),
            (12, "sheet"),
            (13, "fields"),
            (14, "fields"),
        ] {
            let mut extension = extension(calibration());
            match case {
                0 => extension.schema = 2,
                1 => extension.target = Target::Layer(LayerId(0)),
                2 => extension.basis = Basis::Hash(Hash([0; 16])),
                3 => extension.name.clear(),
                4 => {
                    extension.fields.remove(6);
                }
                5 => extension.fields.push(Field {
                    key: "unknown".into(),
                    code: None,
                    value: FieldValue::Integer(1),
                }),
                6 => extension.fields.push(extension.fields[0].clone()),
                7 => extension.fields[0].code = Some(1),
                8 => extension.fields[1].value = FieldValue::Point([0.0, 0.0, 1.0]),
                9 => extension.fields[2].value = FieldValue::Point([0.0, 0.0, 0.0]),
                10 => extension.fields[4].value = FieldValue::Point([1.0, 0.0, 0.0]),
                11 => extension.fields[0].value = FieldValue::Integer(1),
                12 => extension.fields[0].value = FieldValue::Integer(-1),
                13 => extension.fields[5].value = FieldValue::Text("yard".to_owned()),
                _ => extension.fields[0].value = FieldValue::Real(0.0),
            }
            let (result, _, diagnostics) = loaded(
                &drawing(vec![extension]),
                Limits::default(),
                &CancelToken::new(),
            );
            assert!(result.unwrap().is_empty(), "{case}");
            assert_eq!(diagnostics.len(), 1, "{case}");
            assert_eq!(diagnostics[0].code(), codes::DRAWING_CALIBRATION_INVALID);
            assert_eq!(
                diagnostics[0].param("reason"),
                Some(&Value::Text(reason.to_owned()))
            );
            assert_eq!(diagnostics[0].places()[0].record(), Some(7));
        }
        let mut foreign = extension(calibration());
        foreign.namespace = "other.calibration".into();
        foreign.schema = 999;
        let loaded = loaded(
            &drawing(vec![foreign]),
            Limits::default(),
            &CancelToken::new(),
        );
        assert!(loaded.0.unwrap().is_empty() && loaded.2.is_empty());
    }

    #[test]
    fn rejects_all_duplicate_ids_and_ambiguous_sheet_ids() {
        let first = extension(calibration());
        let second = extension(Calibration {
            source: Ref::at_record(8),
            ..calibration()
        });
        let (result, _, diagnostics) = loaded(
            &drawing(vec![first, second]),
            Limits::default(),
            &CancelToken::new(),
        );
        assert!(result.unwrap().is_empty());
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].count(), 2);
        assert_eq!(
            diagnostics[0].param("reason"),
            Some(&Value::Text("duplicate-id".to_owned()))
        );
        assert_eq!(
            diagnostics[0]
                .places()
                .iter()
                .map(Ref::record)
                .collect::<Vec<_>>(),
            [Some(7), Some(8)]
        );
        let mut drawing = drawing(vec![extension(calibration())]);
        drawing.sheets.push(drawing.sheets[0].clone());
        let loaded = loaded(&drawing, Limits::default(), &CancelToken::new());
        assert!(loaded.0.unwrap().is_empty());
        assert_eq!(
            loaded.2[0].param("reason"),
            Some(&Value::Text("sheet".to_owned()))
        );
    }

    #[test]
    fn requires_explicit_selection_for_overlapping_regions_without_changing_geometry_or_ids() {
        let mut drawing = drawing(Vec::new());
        let before = drawing.clone();
        let element = &before.sheets[0].elements[0];
        let old_id = id::Namer::new("sheet/0").next(0, element);
        let old_hash = Hash::element(element);
        drawing.extensions = vec![
            extension(calibration()),
            extension(Calibration {
                id: "vp/1".to_owned(),
                matrix: [2.0, 0.0, 0.0, 2.0],
                ..calibration()
            }),
        ];
        let stored = drawing.clone();
        let values = loaded(&drawing, Limits::default(), &CancelToken::new())
            .0
            .unwrap();
        assert_eq!(
            values
                .iter()
                .map(|value| value.id.as_str())
                .collect::<Vec<_>>(),
            ["vp/0", "vp/1"]
        );
        assert_eq!(values[0].measure(point(0.0, 0.0), point(3.0, 4.0)), Ok(5.0));
        assert_eq!(
            values[1].measure(point(0.0, 0.0), point(3.0, 4.0)),
            Ok(10.0)
        );
        assert_eq!(drawing, stored);
        assert_eq!(drawing.sheets, before.sheets);
        assert_eq!(Hash::element(&drawing.sheets[0].elements[0]), old_hash);
        assert_eq!(
            id::Namer::new("sheet/0").next(0, &drawing.sheets[0].elements[0]),
            old_id
        );
    }

    #[test]
    fn reserves_encoding_and_extraction_before_allocation_and_retries_after_stop() {
        let value = Calibration {
            id: "i".repeat(4097),
            name: Some("n".repeat(8193)),
            unit_label: "u".repeat(16385),
            source: Ref::at_record(7).with_id("s".repeat(8193)),
            ..calibration()
        };
        for kind in [Kind::Work, Kind::DecodedBytes] {
            let mut budget = Budget::new(Limits::default().with(kind, Limit::Finite(0)));
            let mut diag = Diagnostics::new();
            let cancel = CancelToken::new();
            assert!(
                matches!(value.clone().into_draft(&mut Cx::new(&mut budget, &mut diag, &cancel)), Err(Stop::Exceeded(exceeded)) if exceeded.kind == kind)
            );
        }
        let mut invalid = extension(Calibration {
            id: "bad".repeat(4097),
            ..calibration()
        });
        invalid.schema = 2;
        let drawing = drawing(vec![extension(value), invalid]);
        let expected = loaded(&drawing, Limits::default(), &CancelToken::new());
        assert!(expected.0.is_ok());
        for kind in [Kind::Work, Kind::DecodedBytes] {
            let used = expected.1.used(kind);
            for limit in [used - 1, used] {
                let actual = loaded(
                    &drawing,
                    Limits::default().with(kind, Limit::Finite(limit)),
                    &CancelToken::new(),
                );
                if limit == used {
                    assert_eq!(actual.0, expected.0);
                    assert_eq!(actual.2, expected.2);
                } else {
                    assert!(
                        matches!(actual.0, Err(Stop::Exceeded(exceeded)) if exceeded.kind == kind)
                    );
                }
            }
        }
        let cancel = CancelToken::new();
        cancel.cancel();
        assert_eq!(
            loaded(&drawing, Limits::default(), &cancel).0,
            Err(Stop::Cancelled)
        );
        assert_eq!(
            loaded(&Drawing2D::default(), Limits::default(), &cancel).0,
            Err(Stop::Cancelled)
        );
        let retried = loaded(&drawing, Limits::default(), &CancelToken::new());
        assert_eq!(retried.0, expected.0);
        assert_eq!(retried.2, expected.2);
    }

    #[cfg(feature = "binary")]
    #[test]
    fn existing_binary_extensions_preserve_calibrations_without_a_schema_change() {
        let drawing = drawing(vec![extension(calibration())]);
        let source = crate::binary::Source {
            format: "test".to_owned(),
            family: "drawing".to_owned(),
            hash: Hash([0; 16]),
        };
        let mut budget = Budget::default();
        let mut diag = Diagnostics::new();
        let cancel = CancelToken::new();
        let mut cx = Cx::new(&mut budget, &mut diag, &cancel);
        let bytes = crate::binary::write(&source, &drawing, &mut cx).unwrap();
        let (read_source, read_drawing) = crate::binary::read(&bytes, &mut cx).unwrap();
        assert_eq!(read_source, source);
        assert_eq!(read_drawing, drawing);
        assert_eq!(read_drawing.calibrations(&mut cx).unwrap(), [calibration()]);
    }
}
