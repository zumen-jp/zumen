//! 換算と派生単位。buildingSMART の IfcConversionBasedUnit / IfcMeasureWithUnit /
//! IfcDerivedUnit / IfcDerivedUnitElement の自然文の属性の節による。
//! 倍率だけで表せる単位を解き、値そのものは換算しない。

use super::*;

/// 名前と単位の種類。IfcDerivedMeasureValue の各量の自然文による。
pub(super) const DERIVED_MEASURES: &[(&str, Option<&str>)] = &[
    ("IfcLinearVelocityMeasure", Some("LINEARVELOCITYUNIT")),
    ("IfcAngularVelocityMeasure", Some("ANGULARVELOCITYUNIT")),
    ("IfcAccelerationMeasure", Some("ACCELERATIONUNIT")),
    ("IfcMassDensityMeasure", Some("MASSDENSITYUNIT")),
    ("IfcMassFlowRateMeasure", Some("MASSFLOWRATEUNIT")),
    (
        "IfcVolumetricFlowRateMeasure",
        Some("VOLUMETRICFLOWRATEUNIT"),
    ),
    (
        "IfcThermalTransmittanceMeasure",
        Some("THERMALTRANSMITTANCEUNIT"),
    ),
    (
        "IfcThermalConductivityMeasure",
        Some("THERMALCONDUCTANCEUNIT"),
    ),
    ("IfcThermalResistanceMeasure", Some("THERMALRESISTANCEUNIT")),
    (
        "IfcSpecificHeatCapacityMeasure",
        Some("SPECIFICHEATCAPACITYUNIT"),
    ),
    ("IfcDynamicViscosityMeasure", Some("DYNAMICVISCOSITYUNIT")),
    (
        "IfcKinematicViscosityMeasure",
        Some("KINEMATICVISCOSITYUNIT"),
    ),
    ("IfcLinearForceMeasure", Some("LINEARFORCEUNIT")),
    ("IfcPlanarForceMeasure", Some("PLANARFORCEUNIT")),
    ("IfcTorqueMeasure", Some("TORQUEUNIT")),
    (
        "IfcRotationalFrequencyMeasure",
        Some("ROTATIONALFREQUENCYUNIT"),
    ),
];

impl Lifter<'_> {
    pub(super) fn direct_conversion(
        &mut self,
        record: &Record,
        factor: u64,
        base: &Record,
        unit: &Unit,
        cx: &mut Cx,
    ) -> Option<Unit> {
        let kind = self.enumeration(arg(record, 2))?;
        let name = self.text(arg(record, 3), record, cx)?;
        let value = self
            .file
            .instance(factor)
            .and_then(|f| self.real(arg(f, 1)))
            .filter(|v| v.is_finite() && *v > 0.);
        let scale = value
            .zip(si_scale(unit))
            .map(|(v, s)| v * s)
            .filter(|v| v.is_finite() && *v > 0.);
        let Some(scale) = scale else {
            self.reject_composed_unit(record, "factor", cx);
            return None;
        };
        self.used.insert(factor);
        if let Some(id) = base.id() {
            self.used.insert(id);
        }
        Some(Unit {
            kind,
            prefix: None,
            name,
            scale: Some(scale),
        })
    }
    /// 所有権と呼び出しを再帰させない。共有する単位は cache し、経路内の再訪だけを循環とする。
    pub(super) fn unit_graph(&mut self, root: u64, cx: &mut Cx) -> Option<Unit> {
        let record = self.file.instance(root)?;
        if !self.unit_charge(cx, Kind::DecodedBytes, 64, record) {
            return None;
        }
        let mut stack = vec![(root, false)];
        let mut active = BTreeSet::new();
        while let Some((id, exit)) = stack.pop() {
            let Some(record) = self.file.instance(id) else {
                continue;
            };
            if !self.unit_charge(cx, Kind::Work, 1, record) {
                break;
            }
            if exit {
                let value = self.finish_unit(record, cx);
                if let Some(ref value) = value {
                    if !self.unit_charge(cx, Kind::DecodedBytes, unit_size(value), record) {
                        break;
                    }
                    self.used.insert(id);
                }
                self.units.insert(id, value);
                active.remove(&id);
                cx.leave();
                continue;
            }
            if self.units.contains_key(&id) {
                continue;
            }
            if active.contains(&id) {
                self.reject_composed_unit(record, "cycle", cx);
                self.units.insert(id, None);
                continue;
            }
            if !self.unit_charge(cx, Kind::DecodedBytes, 512, record) {
                break;
            }
            match self.si_unit(record, cx) {
                unit::SiUnitRead::Value(value) => {
                    self.used.insert(id);
                    self.units.insert(id, Some(value));
                    continue;
                }
                unit::SiUnitRead::Rejected => {
                    self.units.insert(id, None);
                    continue;
                }
                unit::SiUnitRead::Absent => {}
            }
            if !self.unit_charge(cx, Kind::Depth, 1, record) {
                break;
            }
            active.insert(id);
            let dependencies = self.unit_dependencies(record, cx);
            stack.push((id, true));
            for dependency in dependencies.into_iter().rev() {
                stack.push((dependency, false));
            }
            if self.is_halted() {
                break;
            }
        }
        for _ in active {
            cx.leave();
        }
        if self.is_halted() {
            return None;
        }
        let value = self.units.get(&root)?.as_ref()?;
        self.copied(cx, unit_size(value)).then(|| value.clone())
    }

    fn unit_dependencies(&mut self, record: &Record, cx: &mut Cx) -> Vec<u64> {
        let mut result = Vec::new();
        let kind = self.file.kind(record);
        if kind.eq_ignore_ascii_case(b"IFCCONVERSIONBASEDUNIT") {
            let dependency = reference(arg(record, 4))
                .filter(|&id| self.is(id, b"IFCMEASUREWITHUNIT"))
                .and_then(|id| self.file.instance(id))
                .and_then(|factor| reference(arg(factor, 2)))
                .filter(|&id| self.file.instance(id).is_some());
            if let Some(id) = dependency {
                result.push(id);
            }
        } else if kind.eq_ignore_ascii_case(b"IFCDERIVEDUNIT")
            && let Some(Value::List(values)) = arg(record, 1)
        {
            if !self.unit_charge(
                cx,
                Kind::DecodedBytes,
                (values.len() as u64).saturating_mul(64),
                record,
            ) {
                return result;
            }
            for value in values {
                if !self.unit_charge(cx, Kind::Work, 1, record) {
                    break;
                }
                let dependency = reference(Some(value))
                    .filter(|&id| self.is(id, b"IFCDERIVEDUNITELEMENT"))
                    .and_then(|id| self.file.instance(id))
                    .and_then(|element| reference(arg(element, 1)))
                    .filter(|&id| self.file.instance(id).is_some());
                if let Some(id) = dependency {
                    result.push(id);
                }
            }
        }
        result
    }

    fn finish_unit(&mut self, record: &Record, cx: &mut Cx) -> Option<Unit> {
        if record.id().is_some_and(|id| self.units.contains_key(&id)) {
            return None;
        }
        let kind = self.file.kind(record);
        if let Some(Value::Enum(raw)) = arg(record, 2)
            && !self.unit_charge(cx, Kind::DecodedBytes, raw.len() as u64, record)
        {
            return None;
        }
        let value = if kind.eq_ignore_ascii_case(b"IFCCONVERSIONBASEDUNIT") {
            self.conversion_unit(record, cx)
        } else if kind.eq_ignore_ascii_case(b"IFCDERIVEDUNIT") {
            self.derived_unit(record, cx)
        } else {
            // オフセットと文脈依存の単位は倍率だけの Unit に読み替えず、未取得の診断を残す。
            return None;
        };
        if value.is_none() && !self.is_halted() {
            let dependencies = self.unit_dependencies(record, cx);
            if !dependencies
                .iter()
                .any(|id| self.units.get(id).is_some_and(Option::is_none))
            {
                self.reject_composed_unit(record, "definition", cx);
            }
        }
        value
    }

    fn conversion_unit(&mut self, record: &Record, cx: &mut Cx) -> Option<Unit> {
        let factor = reference(arg(record, 4)).filter(|&id| self.is(id, b"IFCMEASUREWITHUNIT"))?;
        let factor_record = self.file.instance(factor)?;
        let value = self
            .real(arg(factor_record, 1))
            .filter(|v| v.is_finite() && *v > 0.)?;
        let base = reference(arg(factor_record, 2))?;
        let base = self.units.get(&base)?.as_ref()?;
        let scale = (value * si_scale(base)?)
            .is_finite()
            .then_some(value * si_scale(base)?)?;
        if scale <= 0. {
            return None;
        }
        let kind = self.enumeration(arg(record, 2))?;
        let name = self.text(arg(record, 3), record, cx)?;
        self.used.insert(factor);
        Some(Unit {
            kind,
            prefix: None,
            name,
            scale: Some(scale),
        })
    }

    fn derived_unit(&mut self, record: &Record, cx: &mut Cx) -> Option<Unit> {
        let Some(Value::List(values)) = arg(record, 1) else {
            return None;
        };
        if values.is_empty() {
            return None;
        }
        let kind = self.enumeration(arg(record, 2))?;
        let user = self.text(arg(record, 3), record, cx);
        if kind == "USERDEFINED" && user.is_none() {
            return None;
        }
        let mut name = String::new();
        let mut scale = 1.;
        let mut seen = BTreeSet::new();
        for value in values {
            if !self.unit_charge(cx, Kind::Work, 1, record) {
                return None;
            }
            let id = reference(Some(value)).filter(|&id| self.is(id, b"IFCDERIVEDUNITELEMENT"))?;
            if !seen.insert(id) {
                return None;
            }
            let element = self.file.instance(id)?;
            let dependency = reference(arg(element, 1))?;
            // DerivedUnitElement.Unit は NamedUnit。組立済みの派生単位を再び入れない。
            if self.is(dependency, b"IFCDERIVEDUNIT") {
                return None;
            }
            let Some(Value::Number(raw)) = arg(element, 2) else {
                return None;
            };
            if !self.unit_charge(cx, Kind::Work, 64 + raw.len() as u64, element) {
                return None;
            }
            let exponent = core::str::from_utf8(self.file.bytes(raw.clone()))
                .ok()?
                .parse::<i64>()
                .ok()?;
            let base = self.units.get(&dependency)?.as_ref()?;
            if !self.unit_charge(
                cx,
                Kind::DecodedBytes,
                unit_size(base).saturating_mul(2) + 64,
                element,
            ) {
                return None;
            }
            let factor = integer_power(si_scale(base)?, exponent);
            scale *= factor;
            if !scale.is_finite() || scale <= 0. {
                return None;
            }
            if !name.is_empty() {
                name.push_str(" * ");
            }
            if let Some(prefix) = &base.prefix {
                name.push_str(prefix);
                name.push(' ');
            }
            name.push_str(&base.name);
            name.push('^');
            name.push_str(&exponent.to_string());
            self.used.insert(id);
        }
        // IFC4.3 の Name を優先し、それ以前は式を名前として保持する。
        if self.version == Version::Ifc4x3
            && let Some(written) = self.text(arg(record, 4), record, cx)
        {
            name = written;
        }
        if kind == "USERDEFINED" {
            name = user?;
        }
        Some(Unit {
            kind,
            prefix: None,
            name,
            scale: Some(scale),
        })
    }

    fn reject_composed_unit(&mut self, record: &Record, reason: &'static str, cx: &mut Cx) {
        if !self.unit_charge(cx, Kind::DecodedBytes, 512, record) {
            return;
        }
        if let Some(id) = record.id() {
            self.used.insert(id);
        }
        cx.report(
            Diagnostic::new(codes::IFC_UNIT_INVALID)
                .with("reason", reason)
                .at(at_bytes(record.span())),
        );
        self.unit_charge(cx, Kind::Work, 0, record);
    }
}

/// SI 基本単位系の倍率。質量は kg。温度の原点の移動は倍率に見せない。
fn si_scale(unit: &Unit) -> Option<f64> {
    if let Some(scale) = unit.scale {
        return Some(scale);
    }
    if unit.name == "DEGREE_CELSIUS" {
        return None;
    }
    Some(prefix_scale(unit) * if unit.name == "GRAM" { 0.001 } else { 1. })
}

/// 整数の指数は最大 64 回の二乗で解く。std の環境依存の数学関数を使わない。
fn integer_power(mut base: f64, exponent: i64) -> f64 {
    let mut n = exponent.unsigned_abs();
    if exponent < 0 {
        base = 1. / base;
    }
    let mut result = 1.;
    while n != 0 {
        if n & 1 != 0 {
            result *= base;
        }
        n >>= 1;
        if n != 0 {
            base *= base;
        }
    }
    result
}
