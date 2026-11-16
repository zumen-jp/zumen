//! 明記された開閉形式と幅だけで、プロファイルの正規化した曲線を実寸へ移す。
use super::*;
use zumen_building::{
    Coordinates, CurveGraph, CurveItem, CurveKind, CurvePosition, CurveRef, CurveSource, CurveTrim,
    Id, Literal, TrimmingPreference, TypedValue, Unit,
};

pub(super) struct Prepared<'a> {
    element: Id,
    owner: ElementRef,
    type_name: &'a str,
    operation: &'a str,
    curves: Vec<CurveGraph>,
}
impl Prepared<'_> {
    pub fn rule(&self) -> b::AttributeSymbol<'_> {
        b::AttributeSymbol {
            element: self.element,
            owner: self.owner,
            namespace: "ifc.attribute",
            attribute: "OperationType",
            type_name: self.type_name,
            operation: self.operation,
            curves: &self.curves,
        }
    }
}

enum Attribute<'a> {
    Missing,
    Ambiguous,
    Value(&'a TypedValue),
}
fn attribute<'a>(
    building: &'a Building,
    at: ElementRef,
    name: &str,
    cx: &mut Cx<'_>,
) -> Result<Attribute<'a>, Error> {
    let mut value = None;
    if let Some(extension) = building.components.extension.get(at) {
        for field in &extension.values {
            cx.check(Kind::Work, 1)?;
            if field.namespace == "ifc.attribute"
                && field.name == name
                && value.replace(&field.value).is_some()
            {
                return Ok(Attribute::Ambiguous);
            }
        }
    }
    Ok(value.map_or(Attribute::Missing, Attribute::Value))
}

pub(super) fn prepare<'a>(
    profile: &'a DrawingProfile,
    building: &'a Building,
    cx: &mut Cx<'_>,
) -> Result<Vec<Prepared<'a>>, Error> {
    let scale = length_unit(building, cx)?;
    let mut out = Vec::new();
    for index in 0..building.elements().len() {
        cx.check(Kind::Work, 1)?;
        let at = ElementRef(index as u32);
        let Some(class) = building.components.class.get(at) else {
            continue;
        };
        if class.name != "IfcDoor" {
            continue;
        }
        // buildingSMART IfcDoor Attributes: 型が割り当てられたときはその OperationType を使う。
        // 型の値が不明でも occurrence の値で補わない。窓の PartitioningType は開閉形式に使わない。
        let owner = building.components.r#type.get(at).map_or(at, |t| t.target);
        if owner != at
            && !building
                .components
                .class
                .get(owner)
                .is_some_and(|c| matches!(c.name.as_str(), "IfcDoorType" | "IfcDoorStyle"))
        {
            report(building, at, "joinery", "type", cx)?;
            continue;
        }
        let value = match attribute(building, owner, "OperationType", cx)? {
            Attribute::Value(value) => value,
            Attribute::Missing => {
                report(building, at, "joinery", "operation_missing", cx)?;
                continue;
            }
            Attribute::Ambiguous => {
                report(building, at, "joinery", "operation_ambiguous", cx)?;
                continue;
            }
        };
        let (Some(type_name), Literal::Enumeration(operation)) = (&value.type_name, &value.value)
        else {
            report(building, at, "joinery", "operation_type", cx)?;
            continue;
        };
        profile.profile.plan_work(cx)?;
        let Some(symbol) = profile
            .settings
            .doors
            .iter()
            .find(|s| s.operation == *operation && s.types.contains(type_name))
        else {
            report(building, at, "joinery", "operation_unmapped", cx)?;
            continue;
        };
        let width = match attribute(building, at, "OverallWidth", cx)? {
            Attribute::Value(TypedValue {
                type_name: Some(type_name),
                value: Literal::Real(width),
            }) if type_name == "IfcPositiveLengthMeasure" => {
                Some(width * scale).filter(|n| n.is_finite() && *n > 0.0)
            }
            _ => None,
        };
        let Some(width) = width else {
            report(building, at, "joinery", "width", cx)?;
            continue;
        };
        let mut curves = Vec::new();
        reserve(&mut curves, symbol.lines.len() + symbol.arcs.len(), 256, cx)?;
        for line in &symbol.lines {
            cx.check(Kind::Work, 1)?;
            cx.check(Kind::DecodedBytes, 512)?;
            let points = line.map(|p| Coordinates::Two([p[0] * width, p[1] * width]));
            if points
                .iter()
                .any(|v| matches!(v, Coordinates::Two(p) if p.iter().any(|n| !n.is_finite())))
            {
                return Err(Error::Input("joinery_symbol_size"));
            }
            curves.push(graph(vec![item(CurveKind::Polyline(points.into()))], 0));
        }
        for arc in &symbol.arcs {
            cx.check(Kind::Work, 1)?;
            cx.check(Kind::DecodedBytes, 1024)?;
            let center = arc.center.map(|v| v * width);
            let radius = arc.radius * width;
            if !radius.is_finite() || center.iter().any(|v| !v.is_finite()) {
                return Err(Error::Input("joinery_symbol_size"));
            }
            curves.push(graph(
                vec![
                    item(CurveKind::Circle {
                        position: CurvePosition {
                            location: Coordinates::Two(center),
                            axis: None,
                            reference_direction: None,
                        },
                        radius,
                    }),
                    item(CurveKind::Trimmed {
                        basis: CurveRef(0),
                        trim1: vec![CurveTrim::Parameter(
                            arc.start * (core::f64::consts::PI / 180.0),
                        )],
                        trim2: vec![CurveTrim::Parameter(
                            arc.end * (core::f64::consts::PI / 180.0),
                        )],
                        sense_agreement: true,
                        preference: TrimmingPreference::Parameter,
                    }),
                ],
                1,
            ));
        }
        reserve(&mut out, 1, 128, cx)?;
        out.push(Prepared {
            element: building.id(at).ok_or(Error::Input("joinery_element"))?,
            owner,
            type_name,
            operation,
            curves,
        });
    }
    Ok(out)
}
fn item(kind: CurveKind) -> CurveItem {
    CurveItem {
        kind,
        source: CurveSource::default(),
    }
}
fn graph(items: Vec<CurveItem>, root: u32) -> CurveGraph {
    CurveGraph {
        root: CurveRef(root),
        items,
        length_unit: Some(Unit {
            kind: "LENGTHUNIT".into(),
            name: "METRE".into(),
            prefix: None,
            scale: None,
        }),
        angle_unit: Some(Unit {
            kind: "PLANEANGLEUNIT".into(),
            name: "RADIAN".into(),
            prefix: None,
            scale: None,
        }),
    }
}
