//! 計測対象と、開口・部品の分解でつながる形だけを評価する。

use super::{Options, reserve};
use crate::{
    Building, ElementRef, Opening, ParentKind,
    evaluation::{self, ElementMesh},
};
use zumen_base::{Cx, Kind};
use zumen_geom3d::Failure;

fn edge(
    edges: &mut [Vec<ElementRef>],
    a: ElementRef,
    b: ElementRef,
    cx: &mut Cx<'_>,
) -> Result<(), Failure> {
    for (from, to) in [(a, b), (b, a)] {
        let list = edges
            .get_mut(from.0 as usize)
            .ok_or(Failure::Invalid("measure_related_element"))?;
        reserve(list, 1, 4, cx)?;
        list.push(to);
    }
    Ok(())
}

pub(super) fn evaluate(
    building: &Building,
    selected: &[ElementRef],
    openings: bool,
    options: Options<'_>,
    cx: &mut Cx<'_>,
) -> Result<Vec<ElementMesh>, Failure> {
    let count = building.elements().len();
    let mut mask = Vec::new();
    reserve(&mut mask, count, 1, cx)?;
    cx.check(Kind::Work, count as u64)?;
    mask.resize(count, false);
    let mut pending = Vec::new();
    reserve(&mut pending, selected.len(), 4, cx)?;
    for &at in selected {
        cx.check(Kind::Work, 1)?;
        let seen = mask
            .get_mut(at.0 as usize)
            .ok_or(Failure::Invalid("measure_element"))?;
        if !*seen {
            *seen = true;
            pending.push(at);
        }
    }
    if openings && !building.components.opening.is_empty() {
        let mut edges = Vec::new();
        reserve(&mut edges, count, 24, cx)?;
        cx.check(Kind::Work, count as u64)?;
        edges.resize_with(count, Vec::new);
        for (child, parent) in building.components.parent.iter() {
            cx.check(Kind::Work, 1)?;
            if let Some(parent) = &parent.of
                && matches!(parent.kind, ParentKind::Aggregates | ParentKind::Nests)
                // 空間構造の集約は部品の分解ではない。室から階・建物へ広げない。
                && building.components.spatial.get(parent.target).is_none()
            {
                edge(&mut edges, child, parent.target, cx)?;
            }
        }
        for (opening, relation) in building.components.opening.iter() {
            cx.check(Kind::Work, 1)?;
            if let Opening::Voids(host) = relation {
                edge(&mut edges, opening, *host, cx)?;
            }
        }
        while let Some(at) = pending.pop() {
            let neighbors = edges
                .get(at.0 as usize)
                .ok_or(Failure::Invalid("measure_related_element"))?;
            for &neighbor in neighbors {
                cx.check(Kind::Work, 1)?;
                let seen = mask
                    .get_mut(neighbor.0 as usize)
                    .ok_or(Failure::Invalid("measure_related_element"))?;
                if !*seen {
                    *seen = true;
                    reserve(&mut pending, 1, 4, cx)?;
                    pending.push(neighbor);
                }
            }
        }
    }
    let include = |at: ElementRef| mask.get(at.0 as usize).copied().unwrap_or(false);
    evaluation::evaluate_filtered(
        building,
        evaluation::Options {
            chord: options.tolerance.chord,
            openings,
        },
        include,
        include,
        cx,
    )
}
