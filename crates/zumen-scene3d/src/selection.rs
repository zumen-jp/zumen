//! 要素の番号と所属する階。表示用の番号を永続 ID にしない。

use crate::{Error, Options, building::reserve};
use zumen_base::{Cx, Diagnostic, Kind, codes};
use zumen_building::{Building, ElementRef, Opening, SpatialKind};

pub(crate) struct Selection {
    pub elements: Vec<ElementRef>,
    pub storeys: Vec<Option<ElementRef>>,
}

pub(crate) fn is_void(building: &Building, at: ElementRef) -> bool {
    matches!(building.components.opening.get(at), Some(Opening::Voids(_)))
}

fn storey(building: &Building, at: ElementRef, cx: &mut Cx) -> Result<Option<ElementRef>, Error> {
    let mut current = at;
    let mut entered = 0;
    let result = (|| {
        for _ in 0..=building.elements().len() {
            cx.check(Kind::Work, 1)?;
            cx.enter()?;
            entered += 1;
            if building.element(current).is_none() {
                break;
            }
            if building
                .components
                .spatial
                .get(current)
                .is_some_and(|s| s.kind == SpatialKind::Storey)
            {
                return Ok(Some(current));
            }
            let Some(parent) = building
                .components
                .parent
                .get(current)
                .and_then(|p| p.of.as_ref())
            else {
                return Ok(None);
            };
            current = parent.target;
        }
        cx.report(Diagnostic::new(codes::SCENE3D_STOREY_INVALID).at(building.place(at)));
        Ok(None)
    })();
    for _ in 0..entered {
        cx.leave();
    }
    result
}

pub(crate) fn select(
    building: &Building,
    options: Options<'_>,
    cx: &mut Cx,
) -> Result<Selection, Error> {
    let mut selected = Selection {
        elements: Vec::new(),
        storeys: Vec::new(),
    };
    reserve(&mut selected.storeys, building.elements().len(), 8, cx)?;
    for (index, element) in building.elements().iter().enumerate() {
        cx.check(Kind::Work, 1)?;
        let at = ElementRef(u32::try_from(index).map_err(|_| Error::Invalid("size"))?);
        let floor = storey(building, at, cx)?;
        selected.storeys.push(floor);
        let class = building
            .components
            .class
            .get(at)
            .map(|c| c.name.as_str())
            .unwrap_or("");
        let cost = options.ids.map_or(0, |v| v.len()) as u64
            + options.storeys.map_or(0, |v| v.len()) as u64
            + options.classes.map_or(0, |v| v.len()) as u64 * (class.len() as u64 + 1);
        cx.check(Kind::Work, cost)?;
        if options.ids.is_some_and(|ids| !ids.contains(&element.id()))
            || options.storeys.is_some_and(|ids| {
                !floor
                    .and_then(|s| building.id(s))
                    .is_some_and(|s| ids.contains(&s))
            })
            || options
                .classes
                .is_some_and(|names| !names.iter().any(|name| name == class))
        {
            continue;
        }
        reserve(&mut selected.elements, 1, 4, cx)?;
        selected.elements.push(at);
    }
    Ok(selected)
}
