//! 実寸の空間と公開 ID で要素の集合を選び、閉領域・丸め・使わなかった部分と近似・失敗の理由を返す。

use super::*;
use crate::{
    measurement,
    messages::{
        ClosedRegion, MeasureElement, Point2, RegionDiscard, RegionDiscardKind, RegionFill,
        RegionOperation, RegionRounding, RegionsRequest, RegionsResult,
    },
    region::{self, Mode, Note},
};
use zumen_drawing::{ModelPoint, ModelSpace};
use zumen_geom2d::topology as t;

struct Target<'a> {
    id: &'a str,
    value: &'a Element<ModelPoint>,
}

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
        let targets = resolve(&wanted, space, &request.from, head, cx)?;
        let mut paths = Vec::new();
        reserve(&mut paths, targets.len(), head, cx)?;
        for target in &targets {
            paths.push(
                measurement::path(&target.value.shape, space.id, tolerance, cx)
                    .map_err(|error| failed(target, error, cx))?,
            );
        }
        let Some(first) = targets.first() else {
            return Err(fail(cx, invalid_value("/subject")));
        };
        let grid = region::grid(&paths, tolerance).map_err(|error| failed(first, error, cx))?;
        let mut lines = Vec::new();
        reserve(&mut lines, targets.len(), head, cx)?;
        let mut approximate = Vec::new();
        reserve(&mut approximate, targets.len(), head, cx)?;
        for (target, path) in targets.iter().zip(&paths) {
            if matches!(mode, Mode::Boolean(..)) && !path.closed {
                return Err(report_failed(
                    target.id,
                    Some(&target.value.source),
                    "not-closed",
                    cx,
                ));
            }
            let (line, approximated) =
                region::polyline(path, grid, cx).map_err(|error| failed(target, error, cx))?;
            lines.push(line);
            approximate.push(approximated);
        }
        let (subject, clip_lines) = lines.split_at(request.subject.len());
        let found = region::regions(subject, clip_lines, mode, grid, cx)
            .map_err(|error| failed(first, error, cx))?;
        for (target, approximated) in targets.iter().zip(&approximate) {
            if *approximated {
                approximated_diagnostic(target, tolerance, cx)?;
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

fn reserve<T>(output: &mut Vec<T>, count: usize, id: &str, cx: &mut Cx<'_>) -> Result<(), Failed> {
    cx.check(Kind::DecodedBytes, (count as u64).saturating_mul(64))
        .map_err(|stop| stopped(cx, stop))?;
    output
        .try_reserve(count)
        .map_err(|_| report_failed(id, None, "allocation", cx))
}

/// 指定順の要素を 1 回の走査で探す。最初に見つからなかった ID で失敗する。
fn resolve<'a>(
    wanted: &[&'a MeasureElement],
    space: &'a ModelSpace,
    place: &str,
    head: &str,
    cx: &mut Cx<'_>,
) -> Result<Vec<Target<'a>>, Failed> {
    let mut order = Vec::new();
    reserve(&mut order, wanted.len(), head, cx)?;
    order.extend(
        wanted
            .iter()
            .enumerate()
            .map(|(index, item)| (item.id.as_str(), index)),
    );
    cx.check(Kind::Work, (order.len() as u64).saturating_mul(16))
        .map_err(|stop| stopped(cx, stop))?;
    order.sort_unstable();
    let mut found: Vec<Option<&'a Element<ModelPoint>>> = Vec::new();
    reserve(&mut found, wanted.len(), head, cx)?;
    found.resize(wanted.len(), None);
    let mut left = wanted.len();
    let mut namer = id::Namer::new(place);
    for (index, value) in space.elements.iter().enumerate() {
        if left == 0 {
            break;
        }
        let bytes = value.source.id().map_or(0, |id| id.len() as u64) + place.len() as u64;
        cx.check(Kind::Work, 8 + bytes.div_ceil(16))
            .map_err(|stop| stopped(cx, stop))?;
        cx.check(Kind::DecodedBytes, 1024 + bytes.saturating_mul(4))
            .map_err(|stop| stopped(cx, stop))?;
        let name = namer.next(index, value);
        let start = order.partition_point(|(id, _)| *id < name.as_str());
        for &(id, position) in order.iter().skip(start) {
            if id != name {
                break;
            }
            if let Some(slot) = found.get_mut(position)
                && slot.is_none()
            {
                *slot = Some(value);
                left -= 1;
            }
        }
    }
    let mut targets = Vec::new();
    reserve(&mut targets, wanted.len(), head, cx)?;
    for (item, value) in wanted.iter().zip(found) {
        let Some(value) = value else {
            return Err(report_failed(&item.id, None, "id-not-found", cx));
        };
        targets.push(Target {
            id: &item.id,
            value,
        });
    }
    Ok(targets)
}

fn diagnostic_budget(id: &str, source: Option<&Ref>, cx: &mut Cx<'_>) -> Result<(), Failed> {
    let bytes = id.len() as u64 + source.and_then(Ref::id).map_or(0, |id| id.len() as u64);
    cx.check(Kind::Work, 4 + bytes.div_ceil(16))
        .map_err(|stop| stopped(cx, stop))?;
    cx.check(Kind::DecodedBytes, 2048 + bytes.saturating_mul(2))
        .map_err(|stop| stopped(cx, stop))
}

fn report_failed(id: &str, source: Option<&Ref>, reason: &str, cx: &mut Cx<'_>) -> Failed {
    if let Err(failed) = diagnostic_budget(id, source, cx) {
        return failed;
    }
    let mut diagnostic = Diagnostic::new(codes::DRAWING_ELEMENT_REGION_FAILED)
        .with("id", id)
        .with("reason", reason);
    if let Some(source) = source {
        diagnostic = diagnostic.at(source.clone());
    }
    fail(cx, diagnostic)
}

fn failed(target: &Target<'_>, error: measurement::Error, cx: &mut Cx<'_>) -> Failed {
    match error {
        measurement::Error::Stopped(stop) => {
            fail(cx, stop.to_diagnostic().at(target.value.source.clone()))
        }
        measurement::Error::Geometry(reason) => {
            report_failed(target.id, Some(&target.value.source), reason, cx)
        }
    }
}

fn approximated_diagnostic(
    target: &Target<'_>,
    tolerance: f64,
    cx: &mut Cx<'_>,
) -> Result<(), Failed> {
    diagnostic_budget(target.id, Some(&target.value.source), cx)?;
    cx.report(
        Diagnostic::new(codes::DRAWING_ELEMENT_REGION_APPROXIMATED)
            .with("id", target.id)
            .with("tolerance", tolerance)
            .at(target.value.source.clone()),
    );
    Ok(())
}
