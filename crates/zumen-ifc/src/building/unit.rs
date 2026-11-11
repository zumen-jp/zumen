//! 共有する SI の単位の意味検査。IFC2X3 TC1・IFC4 ADD2 TC1・IFC4.3 ADD2 の
//! IfcSIUnit / IfcNamedUnit.WR1 / IfcCorrectDimensions / IfcDimensionsForSIUnit による。
//! IFC2X3 の電気容量の読み取り判断は docs/formats/ifc.md に記す。

use super::{Lifter, PREFIXES};
use crate::{Record, Value, at_bytes};
use zumen_base::{Diagnostic, Kind, codes};
use zumen_building::Unit;
use zumen_sdk::Cx;

/// SI がなければほかの種類を読める。拒否や停止の後は、別の種類として再採用しない。
pub(super) enum SiUnitRead {
    Absent,
    Rejected,
    Value(Unit),
}

impl Lifter<'_> {
    /// 単位の追加検査は、呼び出したプロパティなどではなく、元の単位で停止する。
    pub(super) fn unit_charge(&self, cx: &mut Cx, kind: Kind, n: u64, record: &Record) -> bool {
        if self.is_halted() {
            return false;
        }
        if self.charge(cx, kind, n) {
            true
        } else {
            self.unit_halt_place.set(Some(record.span()));
            false
        }
    }

    /// 正規の単独・複合の形は最大 2 Part・4 属性。借用値の固定個数の照合には、
    /// 既存の単位を読む Work を使い、正しい入力の予算を変えない。非正規の多 Part は走査前に数える。
    pub(super) fn si_unit(&mut self, record: &Record, cx: &mut Cx) -> SiUnitRead {
        if !self.unit_charge(cx, Kind::Work, 0, record) {
            return SiUnitRead::Rejected;
        }
        if record.id().is_some_and(|id| self.invalid_si.contains(&id)) {
            return SiUnitRead::Rejected;
        }
        let parts = record.parts();
        if parts.len() > 2 && !self.unit_charge(cx, Kind::Work, parts.len() as u64, record) {
            return SiUnitRead::Rejected;
        }
        let is = |part: &zumen_step::Part, name: &[u8]| {
            self.file.bytes(part.name()).eq_ignore_ascii_case(name)
        };
        let Some(si) = parts.iter().find(|p| is(p, b"IFCSIUNIT")) else {
            return SiUnitRead::Absent;
        };
        let values = match (parts, si.args()) {
            ([_], [dimensions, kind, prefix, name]) => Some((dimensions, kind, prefix, name)),
            ([a, b], [prefix, name]) => {
                let named = if is(a, b"IFCNAMEDUNIT") && is(b, b"IFCSIUNIT") {
                    a
                } else if is(b, b"IFCNAMEDUNIT") && is(a, b"IFCSIUNIT") {
                    b
                } else {
                    return self.reject_si(record, "layout", cx);
                };
                match named.args() {
                    [dimensions, kind] => Some((dimensions, kind, prefix, name)),
                    _ => None,
                }
            }
            _ => None,
        };
        let Some((dimensions, kind, prefix, name)) = values else {
            return self.reject_si(record, "layout", cx);
        };
        let reason = if !matches!(dimensions, Value::Derived) {
            Some("dimensions")
        } else if !matches!(prefix, Value::Null)
            && !enumeration(self.file, prefix)
                .is_some_and(|p| PREFIXES.iter().any(|(known, _)| known.as_bytes() == p))
        {
            Some("prefix")
        } else {
            match (
                enumeration(self.file, kind),
                enumeration(self.file, name).and_then(name_dimensions),
            ) {
                // 正規の列挙。UNKNOWN は次元不整合の FALSE と区別し、元値を保持する。
                // 名前・使用文脈が示す種類の検証は行わない。
                (Some(b"USERDEFINED"), Some(_)) => None,
                (kind, Some(actual)) => match kind.and_then(type_dimensions) {
                    Some(expected) if actual == expected => None,
                    Some(_) => Some("unit-type-dimensions"),
                    None => Some("unit-type"),
                },
                (_, None) => Some("name"),
            }
        };
        if let Some(reason) = reason {
            return self.reject_si(record, reason, cx);
        }
        // 検査した後も元の列挙値を、従来と同じ順で保持する。換算や正規化はしない。
        let Some(kind) = self.enumeration(Some(kind)) else {
            return self.reject_si(record, "unit-type", cx);
        };
        let prefix = self.enumeration(Some(prefix));
        let Some(name) = self.enumeration(Some(name)) else {
            return self.reject_si(record, "name", cx);
        };
        SiUnitRead::Value(Unit {
            kind,
            prefix,
            name,
            scale: None,
        })
    }

    fn reject_si(&mut self, record: &Record, reason: &'static str, cx: &mut Cx) -> SiUnitRead {
        // BTreeSet の節・used の追加・診断の固定長の引数の確保前。正しい単位の経路には加えない。
        if !self.unit_charge(cx, Kind::Work, 1, record)
            || !self.unit_charge(cx, Kind::DecodedBytes, 512, record)
        {
            return SiUnitRead::Rejected;
        }
        if let Some(id) = record.id() {
            self.invalid_si.insert(id);
            self.used.insert(id);
        }
        cx.report(
            Diagnostic::new(codes::IFC_UNIT_INVALID)
                .with("reason", reason)
                .at(at_bytes(record.span())),
        );
        self.unit_charge(cx, Kind::Work, 0, record);
        SiUnitRead::Rejected
    }
}

fn enumeration<'f>(file: &'f crate::IfcFile, value: &Value) -> Option<&'f [u8]> {
    match value {
        Value::Enum(raw) => Some(file.bytes(raw.clone())),
        _ => None,
    }
}

/// IfcCorrectDimensions の 7 次元。USERDEFINED の UNKNOWN は呼び出し側で区別する。
fn type_dimensions(kind: &[u8]) -> Option<[i8; 7]> {
    Some(match kind {
        b"LENGTHUNIT" => [1, 0, 0, 0, 0, 0, 0],
        b"AREAUNIT" => [2, 0, 0, 0, 0, 0, 0],
        b"VOLUMEUNIT" => [3, 0, 0, 0, 0, 0, 0],
        b"MASSUNIT" => [0, 1, 0, 0, 0, 0, 0],
        b"TIMEUNIT" => [0, 0, 1, 0, 0, 0, 0],
        b"ELECTRICCURRENTUNIT" => [0, 0, 0, 1, 0, 0, 0],
        b"THERMODYNAMICTEMPERATUREUNIT" => [0, 0, 0, 0, 1, 0, 0],
        b"AMOUNTOFSUBSTANCEUNIT" => [0, 0, 0, 0, 0, 1, 0],
        b"LUMINOUSINTENSITYUNIT" | b"LUMINOUSFLUXUNIT" => [0, 0, 0, 0, 0, 0, 1],
        b"PLANEANGLEUNIT" | b"SOLIDANGLEUNIT" => [0; 7],
        b"FREQUENCYUNIT" | b"RADIOACTIVITYUNIT" => [0, 0, -1, 0, 0, 0, 0],
        b"FORCEUNIT" => [1, 1, -2, 0, 0, 0, 0],
        b"PRESSUREUNIT" => [-1, 1, -2, 0, 0, 0, 0],
        b"ENERGYUNIT" => [2, 1, -2, 0, 0, 0, 0],
        b"POWERUNIT" => [2, 1, -3, 0, 0, 0, 0],
        b"ELECTRICCHARGEUNIT" => [0, 0, 1, 1, 0, 0, 0],
        b"ELECTRICVOLTAGEUNIT" => [2, 1, -3, -1, 0, 0, 0],
        b"ELECTRICCAPACITANCEUNIT" => [-2, -1, 4, 2, 0, 0, 0],
        b"ELECTRICRESISTANCEUNIT" => [2, 1, -3, -2, 0, 0, 0],
        b"ELECTRICCONDUCTANCEUNIT" => [-2, -1, 3, 2, 0, 0, 0],
        b"MAGNETICFLUXUNIT" => [2, 1, -2, -1, 0, 0, 0],
        b"MAGNETICFLUXDENSITYUNIT" => [0, 1, -2, -1, 0, 0, 0],
        b"INDUCTANCEUNIT" => [2, 1, -2, -2, 0, 0, 0],
        b"ILLUMINANCEUNIT" => [-2, 0, 0, 0, 0, 0, 1],
        b"ABSORBEDDOSEUNIT" | b"DOSEEQUIVALENTUNIT" => [2, 0, -2, 0, 0, 0, 0],
        _ => return None,
    })
}

/// IfcSIUnitName と IfcDimensionsForSIUnit。名前と UnitType の一対一の対応は強制しない。
fn name_dimensions(name: &[u8]) -> Option<[i8; 7]> {
    type_dimensions(match name {
        b"METRE" => b"LENGTHUNIT",
        b"SQUARE_METRE" => b"AREAUNIT",
        b"CUBIC_METRE" => b"VOLUMEUNIT",
        b"GRAM" => b"MASSUNIT",
        b"SECOND" => b"TIMEUNIT",
        b"AMPERE" => b"ELECTRICCURRENTUNIT",
        b"KELVIN" | b"DEGREE_CELSIUS" => b"THERMODYNAMICTEMPERATUREUNIT",
        b"MOLE" => b"AMOUNTOFSUBSTANCEUNIT",
        b"CANDELA" | b"LUMEN" => b"LUMINOUSINTENSITYUNIT",
        b"RADIAN" | b"STERADIAN" => b"PLANEANGLEUNIT",
        b"HERTZ" | b"BECQUEREL" => b"FREQUENCYUNIT",
        b"NEWTON" => b"FORCEUNIT",
        b"PASCAL" => b"PRESSUREUNIT",
        b"JOULE" => b"ENERGYUNIT",
        b"WATT" => b"POWERUNIT",
        b"COULOMB" => b"ELECTRICCHARGEUNIT",
        b"VOLT" => b"ELECTRICVOLTAGEUNIT",
        b"FARAD" => b"ELECTRICCAPACITANCEUNIT",
        b"OHM" => b"ELECTRICRESISTANCEUNIT",
        b"SIEMENS" => b"ELECTRICCONDUCTANCEUNIT",
        b"WEBER" => b"MAGNETICFLUXUNIT",
        b"TESLA" => b"MAGNETICFLUXDENSITYUNIT",
        b"HENRY" => b"INDUCTANCEUNIT",
        b"LUX" => b"ILLUMINANCEUNIT",
        b"GRAY" | b"SIEVERT" => b"ABSORBEDDOSEUNIT",
        _ => return None,
    })
}
