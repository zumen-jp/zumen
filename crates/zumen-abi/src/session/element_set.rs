//! 実寸の空間の公開 ID で要素の集合を選ぶ op の、要素の解決・確保・出所付きの診断。

use super::*;
use crate::{measurement, messages::MeasureElement};
use zumen_base::Code;
use zumen_drawing::{ModelPoint, ModelSpace};

/// op ごとの失敗と近似の診断コード。
#[derive(Clone, Copy)]
pub(super) struct Family {
    pub failed: Code,
    pub approximated: Code,
}

pub(super) struct Target<'a> {
    pub id: &'a str,
    pub value: &'a Element<ModelPoint>,
}

pub(super) fn reserve<T>(
    family: Family,
    output: &mut Vec<T>,
    count: usize,
    id: &str,
    cx: &mut Cx<'_>,
) -> Result<(), Failed> {
    cx.check(Kind::DecodedBytes, (count as u64).saturating_mul(64))
        .map_err(|stop| stopped(cx, stop))?;
    output
        .try_reserve(count)
        .map_err(|_| report_failed(family, id, None, "allocation", cx))
}

/// 指定順の要素を 1 回の走査で探す。最初に見つからなかった ID で失敗する。
pub(super) fn resolve<'a>(
    family: Family,
    wanted: &[&'a MeasureElement],
    space: &'a ModelSpace,
    place: &str,
    head: &str,
    cx: &mut Cx<'_>,
) -> Result<Vec<Target<'a>>, Failed> {
    let mut order = Vec::new();
    reserve(family, &mut order, wanted.len(), head, cx)?;
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
    reserve(family, &mut found, wanted.len(), head, cx)?;
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
    reserve(family, &mut targets, wanted.len(), head, cx)?;
    for (item, value) in wanted.iter().zip(found) {
        let Some(value) = value else {
            return Err(report_failed(family, &item.id, None, "id-not-found", cx));
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

pub(super) fn report_failed(
    family: Family,
    id: &str,
    source: Option<&Ref>,
    reason: &str,
    cx: &mut Cx<'_>,
) -> Failed {
    if let Err(failed) = diagnostic_budget(id, source, cx) {
        return failed;
    }
    let mut diagnostic = Diagnostic::new(family.failed)
        .with("id", id)
        .with("reason", reason);
    if let Some(source) = source {
        diagnostic = diagnostic.at(source.clone());
    }
    fail(cx, diagnostic)
}

pub(super) fn failed(
    family: Family,
    target: &Target<'_>,
    error: measurement::Error,
    cx: &mut Cx<'_>,
) -> Failed {
    match error {
        measurement::Error::Stopped(stop) => {
            fail(cx, stop.to_diagnostic().at(target.value.source.clone()))
        }
        measurement::Error::Geometry(reason) => {
            report_failed(family, target.id, Some(&target.value.source), reason, cx)
        }
    }
}

pub(super) fn approximated(
    family: Family,
    target: &Target<'_>,
    tolerance: f64,
    cx: &mut Cx<'_>,
) -> Result<(), Failed> {
    diagnostic_budget(target.id, Some(&target.value.source), cx)?;
    cx.report(
        Diagnostic::new(family.approximated)
            .with("id", target.id)
            .with("tolerance", tolerance)
            .at(target.value.source.clone()),
    );
    Ok(())
}
