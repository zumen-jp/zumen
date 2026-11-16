//! 形を持つ母体、または母体を分解した子部品へ開口を適用する。

use super::*;
use crate::ParentKind;
use std::collections::BTreeSet;

fn targets(
    host: ElementRef,
    children: &[Vec<ElementRef>],
    elements: &[ElementMesh],
    cx: &mut Cx,
) -> Result<Vec<ElementRef>, Failure> {
    let mut todo = Vec::new();
    reserve(&mut todo, 1, 4, cx)?;
    todo.push(host);
    let mut seen = BTreeSet::new();
    let mut targets = Vec::new();
    while let Some(at) = todo.pop() {
        cx.check(Kind::Work, 1)?;
        cx.check(Kind::DecodedBytes, 64)?;
        if !seen.insert(at) {
            return Err(Failure::Invalid("decomposition_cycle"));
        }
        let element = elements
            .get(at.0 as usize)
            .ok_or(Failure::Invalid("host_reference"))?;
        if !element.parts.is_empty() {
            reserve(&mut targets, 1, 4, cx)?;
            targets.push(at);
        } else if let Some(children) = children.get(at.0 as usize) {
            reserve(&mut todo, children.len(), 4, cx)?;
            todo.extend(children.iter().rev().copied());
        }
    }
    if targets.is_empty() {
        return Err(Failure::Invalid("host_missing"));
    }
    Ok(targets)
}

pub(super) fn apply(
    building: &Building,
    elements: &mut [ElementMesh],
    include: impl Fn(ElementRef) -> bool,
    cx: &mut Cx,
) -> Result<(), Failure> {
    if building.components.opening.is_empty() {
        return Ok(());
    }
    let mut children = Vec::new();
    reserve(&mut children, elements.len(), 24, cx)?;
    children.resize_with(elements.len(), Vec::new);
    for (child, parent) in building.components.parent.iter() {
        cx.check(Kind::Work, 1)?;
        let Some(parent) = &parent.of else { continue };
        if !matches!(parent.kind, ParentKind::Aggregates | ParentKind::Nests) {
            continue;
        }
        if let Some(list) = children.get_mut(parent.target.0 as usize) {
            reserve(list, 1, 4, cx)?;
            list.push(child);
        }
    }
    for (opening, relation) in building.components.opening.iter() {
        if !include(opening) {
            cx.check(Kind::Work, 1)?;
            continue;
        }
        let Opening::Voids(host) = relation else {
            continue;
        };
        cx.check(Kind::Work, 1)?;
        let Some(source) = elements.get(opening.0 as usize) else {
            continue;
        };
        let mut openings = Vec::new();
        reserve(&mut openings, source.parts.len(), 256, cx)?;
        openings.extend_from_slice(&source.parts);
        let complete = source.complete;
        let targets = match targets(*host, &children, elements, cx) {
            Ok(targets) => targets,
            Err(error) => {
                failed(error, "opening", building.place(*host), false, cx)?;
                if let Some(host) = elements.get_mut(host.0 as usize) {
                    host.complete = false;
                }
                continue;
            }
        };
        for at in targets {
            let Some(target) = elements.get_mut(at.0 as usize) else {
                continue;
            };
            if !complete || openings.is_empty() {
                failed(
                    Failure::Invalid("opening_missing"),
                    "opening",
                    building.place(at),
                    true,
                    cx,
                )?;
                target.complete = false;
            }
            for part in &mut target.parts {
                for opening in &openings {
                    let result = (|| {
                        let to_local = inverse(part.frame)?;
                        let mesh = transformed(&part.mesh, part.placement.then(&to_local), cx)?;
                        let tool =
                            transformed(&opening.mesh, opening.placement.then(&to_local), cx)?;
                        ManifoldKernel.boolean(
                            BooleanOp::Difference,
                            &mesh,
                            &tool,
                            part.tolerance,
                            cx,
                        )
                    })();
                    match result {
                        Ok(out) => {
                            out.changes
                                .report(Some(&building.place(at)), part.tolerance, cx)?;
                            cx.check(Kind::DecodedBytes, 128)?;
                            part.mesh = Arc::new(out.mesh);
                            part.placement = part.frame;
                        }
                        Err(error) => {
                            failed(error, "opening", building.place(at), true, cx)?;
                            target.complete = false;
                        }
                    }
                }
            }
        }
    }
    Ok(())
}
