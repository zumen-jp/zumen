//! 土木の保持値を、出所と高さの基準を保って返す。コピーの前に論理量で予算を数える。
use super::*;
use crate::messages as m;
use zumen_base::{Key, Ref};
use zumen_civil::{CantKind, CrossSectionHeight, SurveyPoint};

fn reserve(cx: &mut Cx<'_>, count: usize) -> Result<(), Failed> {
    let count = u64::try_from(count).unwrap_or(u64::MAX);
    cx.check(Kind::Work, count)
        .map_err(|stop| stopped(cx, stop))?;
    cx.check(Kind::DecodedBytes, count.saturating_mul(512))
        .map_err(|stop| stopped(cx, stop))
}

fn text(value: &str, cx: &mut Cx<'_>) -> Result<String, Failed> {
    cx.check(Kind::DecodedBytes, value.len() as u64)
        .map_err(|stop| stopped(cx, stop))?;
    Ok(value.to_owned())
}

fn optional(value: &Option<String>, cx: &mut Cx<'_>) -> Result<Option<String>, Failed> {
    value.as_deref().map(|value| text(value, cx)).transpose()
}

fn source(value: &Ref) -> m::Source {
    m::Source {
        record: value.record(),
        bytes: value.bytes().map(|range| m::ByteRange {
            start: range.start,
            end: range.end,
        }),
        handle: match value.key() {
            Some(Key::Handle(id)) => Some(format!("{id:X}")),
            _ => None,
        },
        instance: match value.key() {
            Some(Key::Instance(id)) => Some(id.to_string()),
            _ => None,
        },
    }
}

fn point(point: &SurveyPoint, cx: &mut Cx<'_>) -> Result<m::CivilSurveyPoint, Failed> {
    Ok(m::CivilSurveyPoint {
        name: optional(&point.name, cx)?,
        description: optional(&point.description, cx)?,
        northing: point.point.northing,
        easting: point.point.easting,
        height: point.height,
        class: optional(&point.class, cx)?,
        source: source(&point.source),
    })
}

pub(super) fn details(
    model: &CivilModel,
    alignment: Option<&Alignment>,
    cx: &mut Cx<'_>,
) -> Result<m::CivilDetails, Failed> {
    reserve(cx, model.point_sets.len().saturating_add(1))?;
    let mut sets = Vec::with_capacity(model.point_sets.len());
    for set in &model.point_sets {
        reserve(cx, set.points.len())?;
        let mut points = Vec::with_capacity(set.points.len());
        for value in &set.points {
            points.push(point(value, cx)?);
        }
        sets.push(m::CivilPointSet {
            name: optional(&set.name, cx)?,
            description: optional(&set.description, cx)?,
            points,
            source: source(&set.source),
        });
    }
    let height_reference = model
        .height_reference
        .as_ref()
        .map(|value| {
            Ok(m::CivilHeightReference {
                name: optional(&value.name, cx)?,
                tokyo_bay_offset: value.tokyo_bay_offset,
                source: source(&value.source),
            })
        })
        .transpose()?;
    Ok(m::CivilDetails {
        alignment: alignment
            .map(|value| alignment_values(value, cx))
            .transpose()?,
        point_sets: sets,
        height_reference,
    })
}

fn alignment_values(
    alignment: &Alignment,
    cx: &mut Cx<'_>,
) -> Result<m::CivilAlignmentValues, Failed> {
    reserve(
        cx,
        alignment
            .intersection_points
            .len()
            .saturating_add(alignment.ground_profiles.len())
            .saturating_add(alignment.cant.len())
            .saturating_add(alignment.cross_sections.len()),
    )?;
    let mut intersection_points = Vec::with_capacity(alignment.intersection_points.len());
    for value in &alignment.intersection_points {
        intersection_points.push(point(value, cx)?);
    }
    let mut ground_profiles = Vec::with_capacity(alignment.ground_profiles.len());
    for value in &alignment.ground_profiles {
        reserve(cx, value.points.len())?;
        ground_profiles.push(m::CivilGroundProfile {
            name: optional(&value.name, cx)?,
            description: optional(&value.description, cx)?,
            points: value
                .points
                .iter()
                .map(|[distance, height]| m::CivilProfilePoint {
                    distance: *distance,
                    height: *height,
                })
                .collect(),
            source: source(&value.source),
        });
    }
    let mut cant = Vec::with_capacity(alignment.cant.len());
    for value in &alignment.cant {
        let (kind, start, end, gauge) = match value.kind {
            CantKind::Superelevation { start, end } => ("superelevation", start, end, None),
            CantKind::Rail { start, end, gauge } => ("rail", start, end, gauge),
        };
        cant.push(m::CivilCant {
            start_distance: value.start_distance,
            length: value.length,
            kind: kind.to_owned(),
            start: start.to_vec(),
            end: end.to_vec(),
            gauge,
            transition: optional(&value.transition, cx)?,
            source: source(&value.source),
        });
    }
    let mut cross_sections = Vec::with_capacity(alignment.cross_sections.len());
    for section in &alignment.cross_sections {
        reserve(cx, section.lines.len())?;
        let mut lines = Vec::with_capacity(section.lines.len());
        for line in &section.lines {
            reserve(cx, line.points.len())?;
            lines.push(m::CivilCrossSectionLine {
                name: optional(&line.name, cx)?,
                description: optional(&line.description, cx)?,
                side: optional(&line.side, cx)?,
                role: text(
                    match &line.role {
                        SurfaceRole::Existing => "existing",
                        SurfaceRole::Design => "design",
                        SurfaceRole::Other(value) => value,
                    },
                    cx,
                )?,
                height_reference: match line.height_reference {
                    CrossSectionHeight::Elevation => "elevation",
                    CrossSectionHeight::FormationOffset => "formation_offset",
                }
                .to_owned(),
                points: line
                    .points
                    .iter()
                    .map(|[offset, height]| m::CivilCrossSectionPoint {
                        offset: *offset,
                        height: *height,
                    })
                    .collect(),
                source: source(&line.source),
            });
        }
        cross_sections.push(m::CivilCrossSection {
            distance: section.distance,
            name: optional(&section.name, cx)?,
            profile: optional(&section.profile, cx)?,
            lines,
            source: source(&section.source),
        });
    }
    Ok(m::CivilAlignmentValues {
        id: text(&alignment.id, cx)?,
        vertical_name: optional(&alignment.vertical_name, cx)?,
        vertical_name_ambiguous: alignment.vertical_name_ambiguous,
        intersection_points,
        ground_profiles,
        cant,
        cross_sections,
    })
}
