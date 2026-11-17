//! 実寸の空間と公開 ID で要素の集合を選び、閉領域・丸め・使わなかった部分と近似・失敗の理由を返す。

use super::*;
use crate::{
    measurement,
    messages::{
        ClosedRegion, Point2, RegionDiscard, RegionDiscardKind, RegionFill, RegionOperation,
        RegionRounding, RegionsRequest, RegionsResult,
    },
    region::{self, Mode, Note},
};
use element_set::{Family, approximated, failed, report_failed, reserve, resolve};
use zumen_geom2d::topology as t;

const FAMILY: Family = Family {
    failed: codes::DRAWING_ELEMENT_REGION_FAILED,
    approximated: codes::DRAWING_ELEMENT_REGION_APPROXIMATED,
};

impl Session {
    pub(super) fn regions(
        &self,
        request: &RegionsRequest,
        cx: &mut Cx<'_>,
    ) -> Result<RegionsResult, Failed> {
        let tolerance = request.tolerance.unwrap_or(0.001);
        if !tolerance.is_finite() || tolerance <= 0.0 {
            return Err(fail(cx, invalid_value("/tolerance")));
        }
        let mode = match request.operation {
            RegionOperation::Detect => {
                if request.clip.is_some() {
                    return Err(fail(cx, invalid_value("/clip")));
                }
                if request.fill.is_some() {
                    return Err(fail(cx, invalid_value("/fill")));
                }
                Mode::Detect
            }
            operation => Mode::Boolean(
                match operation {
                    RegionOperation::Intersection => t::RegionOperation::Intersection,
                    RegionOperation::Difference => t::RegionOperation::Difference,
                    RegionOperation::Xor => t::RegionOperation::Xor,
                    _ => t::RegionOperation::Union,
                },
                match request.fill {
                    Some(RegionFill::NonZero) => t::RegionFill::NonZero,
                    _ => t::RegionFill::EvenOdd,
                },
            ),
        };
        let clip = request.clip.as_deref().unwrap_or_default();
        let Some(head) = request.subject.first() else {
            return Err(fail(cx, invalid_value("/subject")));
        };
        let head = head.id.as_str();
        for (list, name) in [(&request.subject[..], "subject"), (clip, "clip")] {
            if list.len() > t::MAX_REGION_EDGES {
                return Err(fail(cx, invalid_value(format!("/{name}"))));
            }
            cx.check(
                Kind::Work,
                (list.len() as u64).saturating_mul(list.len() as u64),
            )
            .map_err(|stop| stopped(cx, stop))?;
            for (index, item) in list.iter().enumerate() {
                if list.iter().take(index).any(|other| other.id == item.id) {
                    return Err(fail(cx, invalid_value(format!("/{name}/{index}"))));
                }
            }
        }
        let (_, hash) = self.drawing_file(&request.doc, cx)?;
        cx.check(Kind::Work, 4).map_err(|stop| stopped(cx, stop))?;
        cx.check(Kind::DecodedBytes, 256)
            .map_err(|stop| stopped(cx, stop))?;
        let hash = hash.to_string();
        if request
            .expected_hash
            .as_ref()
            .is_some_and(|expected| expected != &hash)
        {
            return Err(fail(cx, invalid_value("/expected_hash")));
        }
        let drawing = self.lift(&request.doc, cx)?;
        let mut selected = None;
        for space in &drawing.spaces {
            cx.check(Kind::Work, 4).map_err(|stop| stopped(cx, stop))?;
            cx.check(Kind::DecodedBytes, 64)
                .map_err(|stop| stopped(cx, stop))?;
            if id::space(space.id) == request.from {
                selected = Some(space);
                break;
            }
        }
        let space = selected.ok_or_else(|| fail(cx, invalid_value("/from")))?;
        let wanted = request.subject.iter().chain(clip).collect::<Vec<_>>();
        let targets = resolve(FAMILY, &wanted, space, &request.from, head, cx)?;
        let mut paths = Vec::new();
        reserve(FAMILY, &mut paths, targets.len(), head, cx)?;
        for target in &targets {
            paths.push(
                measurement::path(&target.value.shape, space.id, tolerance, cx)
                    .map_err(|error| failed(FAMILY, target, error, cx))?,
            );
        }
        let Some(first) = targets.first() else {
            return Err(fail(cx, invalid_value("/subject")));
        };
        let grid =
            region::grid(&paths, tolerance).map_err(|error| failed(FAMILY, first, error, cx))?;
        let mut lines = Vec::new();
        reserve(FAMILY, &mut lines, targets.len(), head, cx)?;
        let mut approximate = Vec::new();
        reserve(FAMILY, &mut approximate, targets.len(), head, cx)?;
        for (target, path) in targets.iter().zip(&paths) {
            if matches!(mode, Mode::Boolean(..)) && !path.closed {
                return Err(report_failed(
                    FAMILY,
                    target.id,
                    Some(&target.value.source),
                    "not-closed",
                    cx,
                ));
            }
            let (line, approximated) = region::polyline(path, grid, cx)
                .map_err(|error| failed(FAMILY, target, error, cx))?;
            lines.push(line);
            approximate.push(approximated);
        }
        let (subject, clip_lines) = lines.split_at(request.subject.len());
        let found = region::regions(subject, clip_lines, mode, grid, cx)
            .map_err(|error| failed(FAMILY, first, error, cx))?;
        for (target, &flag) in targets.iter().zip(&approximate) {
            if flag {
                approximated(FAMILY, target, tolerance, cx)?;
            }
        }
        let points = found
            .regions
            .iter()
            .map(|r| r.outer.len() + r.holes.iter().map(Vec::len).sum::<usize>())
            .sum::<usize>();
        let count = (points + found.regions.len() + found.notes.len()) as u64;
        cx.check(
            Kind::DecodedBytes,
            1024 + request.from.len() as u64 + count.saturating_mul(128),
        )
        .map_err(|stop| stopped(cx, stop))?;
        cx.check(Kind::Work, count)
            .map_err(|stop| stopped(cx, stop))?;
        let point = |[x, y]: [f64; 2]| Point2 { x, y };
        let points = |list: Vec<[f64; 2]>| list.into_iter().map(point).collect::<Vec<_>>();
        let name = |element: usize| {
            targets
                .get(element)
                .map_or_else(String::new, |target| target.id.to_owned())
        };
        let mut rounded = Vec::new();
        let mut discarded = Vec::new();
        for note in found.notes {
            match note {
                Note::Rounded { element, position } => rounded.push(RegionRounding {
                    id: name(element),
                    position: point(position),
                }),
                Note::Degenerate { element } => discarded.push(RegionDiscard {
                    id: name(element),
                    kind: RegionDiscardKind::Degenerate,
                    start: None,
                    end: None,
                }),
                Note::Open {
                    element,
                    start,
                    end,
                } => discarded.push(RegionDiscard {
                    id: name(element),
                    kind: RegionDiscardKind::Open,
                    start: Some(point(start)),
                    end: Some(point(end)),
                }),
            }
        }
        Ok(RegionsResult {
            hash,
            from: request.from.clone(),
            unit: unit_of(space.unit),
            tolerance,
            approximate: approximate.iter().any(|&value| value),
            regions: found
                .regions
                .into_iter()
                .map(|value| ClosedRegion {
                    outer: points(value.outer),
                    holes: value.holes.into_iter().map(points).collect(),
                    area: value.area,
                })
                .collect(),
            rounded,
            discarded,
        })
    }
}
