//! 版・配置・表現・属性を、規則の順にチェックする。

use super::{BuildingProfile, Error, alignment, reserve, strings};
use crate::profile::{GetKey, text};
use crate::{Out, Report, RuleSummary};
use zumen_base::{Code, Cx, Diagnostic, Kind, Ref, Severity, codes};
use zumen_building::{Building, ElementRef, Literal};
use zumen_json::Value;

pub(super) fn finding(
    out: &mut Out<'_>,
    code: Code,
    model: usize,
    reason: &str,
    place: Option<Ref>,
    unsupported: bool,
    cx: &mut Cx<'_>,
) -> Result<(), Error> {
    cx.check(Kind::Work, 1)?;
    cx.check(Kind::DecodedBytes, 256 + reason.len() as u64)?;
    let mut diagnostic = Diagnostic::new(code)
        .with("model", model as u64)
        .with("reason", reason);
    if let Some(place) = place {
        diagnostic = diagnostic.at(place);
    }
    out.push(diagnostic, unsupported, cx)?;
    Ok(())
}

fn classes<'a>(
    profile: &'a BuildingProfile,
    rule: &'a Value,
    cx: &mut Cx<'_>,
) -> Result<Option<Vec<&'a str>>, Error> {
    let Some(names) = strings(rule.get_key("classes")).filter(|v| !v.is_empty()) else {
        return Ok(None);
    };
    let mut classes = Vec::new();
    for name in names {
        cx.check(Kind::Work, profile.resolved.vocabularies.len() as u64 + 1)?;
        let Some(vocabulary) = text(name).and_then(|name| profile.resolved.vocabularies.get(name))
        else {
            return Ok(None);
        };
        let Some(values) = strings(vocabulary.get_key("values")).filter(|v| !v.is_empty()) else {
            return Ok(None);
        };
        reserve(&mut classes, values.len(), 16, cx)?;
        for value in values {
            cx.check(Kind::Work, 1)?;
            let Some(value) = text(value).filter(|v| !v.is_empty()) else {
                return Ok(None);
            };
            classes.push(value);
        }
    }
    Ok(Some(classes))
}

fn schema(
    models: &[&Building],
    rule: &Value,
    out: &mut Out<'_>,
    cx: &mut Cx<'_>,
) -> Result<(), Error> {
    let Some(allowed) = strings(rule.get_key("schemas"))
        .filter(|v| !v.is_empty() && v.iter().all(|s| text(s) != Some("")))
    else {
        return finding(
            out,
            codes::PROFILE_IFC_SCHEMA_UNSUPPORTED,
            0,
            "schemas",
            None,
            true,
            cx,
        );
    };
    if models.is_empty() {
        return finding(
            out,
            codes::PROFILE_IFC_SCHEMA_UNSUPPORTED,
            0,
            "models",
            None,
            true,
            cx,
        );
    }
    for (index, model) in models.iter().enumerate() {
        cx.check(Kind::Work, 1)?;
        let schemas = &model.document.header.schemas;
        cx.check(Kind::Work, schemas.len() as u64)?;
        if schemas.is_empty() || schemas.iter().any(String::is_empty) {
            finding(
                out,
                codes::PROFILE_IFC_SCHEMA_UNSUPPORTED,
                index + 1,
                "schema-missing",
                None,
                true,
                cx,
            )?;
            continue;
        }
        let mut valid = true;
        for schema in schemas {
            for candidate in allowed {
                cx.check(
                    Kind::Work,
                    schema.len() as u64 + text(candidate).map_or(0, |v| v.len()) as u64 + 1,
                )?;
            }
            valid &= allowed.iter().any(|v| text(v) == Some(schema.as_str()));
        }
        if !valid {
            finding(
                out,
                codes::PROFILE_IFC_SCHEMA_INVALID,
                index + 1,
                "schema",
                None,
                false,
                cx,
            )?;
        }
    }
    Ok(())
}

#[derive(Clone, Copy)]
enum Present {
    Yes,
    No,
    Unknown,
}

fn literal(value: &Literal, building: &Building, cx: &mut Cx<'_>) -> Result<Present, Error> {
    cx.enter()?;
    let result = (|| {
        cx.check(Kind::Work, 1)?;
        Ok(match value {
            Literal::Logical(None) => Present::Unknown,
            Literal::Real(value) if !value.is_finite() => Present::Unknown,
            Literal::Text(value) | Literal::Enumeration(value) => {
                cx.check(Kind::Work, value.len() as u64)?;
                if value.trim().is_empty() {
                    Present::No
                } else {
                    Present::Yes
                }
            }
            Literal::Element(at) if building.element(*at).is_none() => Present::Unknown,
            Literal::List(values) => {
                let mut result = Present::No;
                for value in values {
                    match literal(value, building, cx)? {
                        Present::Yes => result = Present::Yes,
                        Present::Unknown if !matches!(result, Present::Yes) => {
                            result = Present::Unknown
                        }
                        _ => {}
                    }
                }
                result
            }
            _ => Present::Yes,
        })
    })();
    cx.leave();
    result
}

fn property(
    building: &Building,
    at: ElementRef,
    set_name: &str,
    name: &str,
    cx: &mut Cx<'_>,
) -> Result<Present, Error> {
    let mut found = None;
    let inherited = building.components.r#type.get(at).map(|kind| kind.target);
    for at in [Some(at), inherited].into_iter().flatten() {
        cx.check(Kind::Work, 1)?;
        if building.element(at).is_none() {
            return Ok(Present::Unknown);
        }
        if let Some(properties) = building.components.properties.get(at) {
            for set in &properties.sets {
                cx.check(Kind::Work, (set_name.len() + set.name.len() + 1) as u64)?;
                if set.name != set_name {
                    continue;
                }
                for property in &set.properties {
                    cx.check(Kind::Work, (name.len() + property.name.len() + 1) as u64)?;
                    if property.name != name {
                        continue;
                    }
                    if found.is_some() {
                        return Ok(Present::Unknown);
                    }
                    found = Some(match &property.value {
                        Some(value) => literal(&value.value, building, cx)?,
                        None => Present::No,
                    });
                }
            }
        }
        // 明示した欠値を型のプロパティで補わない。
        if let Some(found) = found {
            return Ok(found);
        }
    }
    Ok(Present::No)
}

fn elements(
    profile: &BuildingProfile,
    models: &[&Building],
    rule: &Value,
    properties: bool,
    out: &mut Out<'_>,
    cx: &mut Cx<'_>,
) -> Result<(), Error> {
    let (invalid, unsupported) = if properties {
        (
            codes::PROFILE_PROPERTY_PRESENT_INVALID,
            codes::PROFILE_PROPERTY_PRESENT_UNSUPPORTED,
        )
    } else {
        (
            codes::PROFILE_REPRESENTATION_PRESENT_INVALID,
            codes::PROFILE_REPRESENTATION_PRESENT_UNSUPPORTED,
        )
    };
    let property_names = if properties {
        match (
            rule.get_key("property-set").and_then(text),
            rule.get_key("property").and_then(text),
        ) {
            (Some(set), Some(name)) if !set.is_empty() && !name.is_empty() => Some((set, name)),
            _ => return finding(out, unsupported, 0, "property-names", None, true, cx),
        }
    } else {
        None
    };
    let Some(classes) = classes(profile, rule, cx)? else {
        return finding(out, unsupported, 0, "class-mapping", None, true, cx);
    };
    if models.is_empty() {
        return finding(out, unsupported, 0, "models", None, true, cx);
    }
    for (index, model) in models.iter().enumerate() {
        for at in (0..model.elements().len()).map(|i| ElementRef(i as u32)) {
            cx.check(Kind::Work, 1)?;
            cx.check(Kind::Elements, 1)?;
            cx.check(Kind::DecodedBytes, 128)?;
            let Some(class) = model.components.class.get(at) else {
                finding(
                    out,
                    unsupported,
                    index + 1,
                    "class-missing",
                    Some(model.place(at)),
                    true,
                    cx,
                )?;
                continue;
            };
            for candidate in &classes {
                cx.check(
                    Kind::Work,
                    (candidate.len() + class.name.len() + class.original.len() + 1) as u64,
                )?;
            }
            if !classes
                .iter()
                .any(|v| *v == class.name || *v == class.original)
            {
                continue;
            }
            let present = if let Some((set, name)) = property_names {
                property(model, at, set, name, cx)?
            } else if let Some(representation) = model.components.representation.get(at) {
                cx.check(Kind::Work, representation.representations.len() as u64)?;
                if representation
                    .representations
                    .iter()
                    .any(|r| !r.items.is_empty())
                {
                    Present::Yes
                } else {
                    Present::No
                }
            } else {
                Present::No
            };
            match present {
                Present::Yes => {}
                Present::No => finding(
                    out,
                    invalid,
                    index + 1,
                    "missing",
                    Some(model.place(at)),
                    false,
                    cx,
                )?,
                Present::Unknown => finding(
                    out,
                    unsupported,
                    index + 1,
                    "value-unresolved",
                    Some(model.place(at)),
                    true,
                    cx,
                )?,
            }
        }
    }
    Ok(())
}

pub(super) fn check(
    profile: &BuildingProfile,
    models: &[&Building],
    cx: &mut Cx<'_>,
) -> Result<Report, Error> {
    profile.plan_work(cx)?;
    let mut report = Report {
        chain: profile.owned_chain(cx)?,
        rules: Vec::new(),
        findings: Vec::new(),
    };
    reserve(&mut report.rules, profile.resolved.rules.len(), 64, cx)?;
    for (id, rule) in &profile.resolved.rules {
        profile.plan_work(cx)?;
        cx.check(Kind::Work, 1)?;
        let kind = rule.get_key("check").and_then(text).unwrap_or_default();
        let level = match rule.get_key("level").and_then(text) {
            Some("info") => Some(Severity::Info),
            Some("warning") => Some(Severity::Warning),
            Some("error") => Some(Severity::Error),
            _ => None,
        };
        let mut out = Out {
            rule: id,
            level: level.unwrap_or(Severity::Info),
            findings: Vec::new(),
            invalid: 0,
            unsupported: 0,
        };
        match kind {
            "ifc-schema" if level.is_some() => schema(models, rule, &mut out, cx)?,
            "models-aligned" if level.is_some() => alignment::check(models, rule, &mut out, cx)?,
            "representation-present" | "property-present" if level.is_some() => elements(
                profile,
                models,
                rule,
                kind == "property-present",
                &mut out,
                cx,
            )?,
            _ => {
                cx.check(Kind::DecodedBytes, 128 + kind.len() as u64)?;
                out.unsupported(
                    Diagnostic::new(codes::PROFILE_RULE_UNSUPPORTED).with("check", kind),
                    cx,
                )?;
            }
        }
        cx.check(Kind::DecodedBytes, (id.len() + kind.len()) as u64)?;
        report.rules.push(RuleSummary {
            id: id.clone(),
            check: kind.into(),
            findings: out.invalid,
            unsupported: out.unsupported,
        });
        reserve(&mut report.findings, out.findings.len(), 256, cx)?;
        report.findings.extend(out.findings);
    }
    cx.check(Kind::Work, 0)?;
    Ok(report)
}
