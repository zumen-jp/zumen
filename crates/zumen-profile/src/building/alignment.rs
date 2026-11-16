//! 単位と建物の工学座標系を比べる。地図への不明な対応や曖昧な基準点を補わない。

use super::{Error, check::finding, reserve};
use crate::Out;
use crate::profile::GetKey;
use zumen_base::{Cx, Kind, codes};
use zumen_building::{
    Building, ContextPlacement, ElementRef, GeometryDefinition, Local, SpatialKind,
};
use zumen_geom3d::Transform;
use zumen_json::Value;

struct Frame {
    unit: f64,
    placement: Transform,
}
enum Read {
    Frame(Frame),
    Unknown(&'static str),
}

// world_placement と同じく外側の配置から掛け、浮動小数の演算の順もそろえる。
// 循環は要素数を超える歩数で止める。配列の確保と深さは事前に数える。
fn placement(
    building: &Building,
    mut at: ElementRef,
    cx: &mut Cx<'_>,
) -> Result<Option<Transform>, Error> {
    let mut entered = 0;
    let result = (|| {
        let mut chain = Vec::new();
        for _ in 0..=building.elements().len() {
            cx.check(Kind::Work, 1)?;
            cx.enter()?;
            entered += 1;
            if building.element(at).is_none() {
                return Ok(None);
            }
            let Some(value) = building.components.placement.get(at) else {
                return Ok(None);
            };
            let Local::Transform(local) = &value.local else {
                return Ok(None);
            };
            reserve(&mut chain, 1, 96, cx)?;
            chain.push(*local);
            match value.relative_to {
                Some(parent) => at = parent,
                None => {
                    cx.check(Kind::Work, chain.len() as u64)?;
                    let world = chain
                        .iter()
                        .rev()
                        .fold(zumen_building::Transform::IDENTITY, |outer, local| {
                            outer.then(local)
                        });
                    return Ok(Some(Transform(world.0)));
                }
            }
        }
        Ok(None)
    })();
    for _ in 0..entered {
        cx.leave();
    }
    result
}

fn equal(a: Transform, b: Transform, length: f64, axis: f64) -> bool {
    a.0.into_iter().zip(b.0).all(|(a, b)| {
        a.into_iter()
            .zip(b)
            .enumerate()
            .all(|(index, (a, b))| (a - b).abs() <= if index == 3 { length } else { axis })
    })
}

fn frame(building: &Building, length: f64, axis: f64, cx: &mut Cx<'_>) -> Result<Read, Error> {
    let mut unit = None;
    for value in &building.document.units {
        cx.check(
            Kind::Work,
            (value.kind.len() + value.name.len() + value.prefix.as_ref().map_or(0, String::len) + 1)
                as u64,
        )?;
        if value.kind != "LENGTHUNIT" {
            continue;
        }
        let Ok(factor) = zumen_building::evaluation::length_scale(value) else {
            return Ok(Read::Unknown("length-unit"));
        };
        if unit.is_some_and(|previous| previous != factor) {
            return Ok(Read::Unknown("length-unit-conflict"));
        }
        unit = Some(factor);
    }
    let Some(unit) = unit else {
        return Ok(Read::Unknown("length-unit-missing"));
    };
    let mut anchor = None;
    for (at, spatial) in building.components.spatial.iter() {
        cx.check(Kind::Work, 1)?;
        if spatial.kind != SpatialKind::Building {
            continue;
        }
        if anchor.replace(at).is_some() {
            return Ok(Read::Unknown("building-ambiguous"));
        }
    }
    let Some(anchor) = anchor else {
        return Ok(Read::Unknown("building-missing"));
    };
    let Some(mut placement) = placement(building, anchor, cx)? else {
        return Ok(Read::Unknown("placement"));
    };
    for row in &mut placement.0 {
        row[3] *= unit;
    }
    if !placement.is_finite()
        || !placement.determinant().is_finite()
        || placement.determinant() == 0.0
    {
        return Ok(Read::Unknown("placement"));
    }
    let mut context = None;
    for at in (0..building.elements().len()).map(|n| zumen_building::ElementRef(n as u32)) {
        cx.check(Kind::Work, 1)?;
        let Some(geometry) = building.geometry(at) else {
            continue;
        };
        for node in &geometry.data().nodes {
            cx.check(Kind::Work, 1)?;
            let GeometryDefinition::Context {
                definition,
                length_unit,
            } = &node.definition
            else {
                continue;
            };
            if definition.geometric.dimension != 3 {
                continue;
            }
            let ContextPlacement::Space(value) = &definition.geometric.world_coordinate_system
            else {
                return Ok(Read::Unknown("context-dimension"));
            };
            let Some(context_unit) = length_unit.as_ref() else {
                return Ok(Read::Unknown("context-unit"));
            };
            cx.check(
                Kind::Work,
                (context_unit.kind.len()
                    + context_unit.name.len()
                    + context_unit.prefix.as_ref().map_or(0, String::len)
                    + 1) as u64,
            )?;
            let Ok(factor) = zumen_building::evaluation::length_scale(context_unit) else {
                return Ok(Read::Unknown("context-unit"));
            };
            let Some(frame) = Transform::from_axes(
                value.location.map(|v| v * factor),
                value.axis,
                value.reference_direction,
            ) else {
                return Ok(Read::Unknown("context-frame"));
            };
            if context.is_some_and(|previous| !equal(previous, frame, length, axis)) {
                return Ok(Read::Unknown("context-ambiguous"));
            }
            context = Some(frame);
        }
    }
    let Some(context) = context else {
        return Ok(Read::Unknown("context-missing"));
    };
    let placement = placement.then(&context);
    if !placement.is_finite() {
        return Ok(Read::Unknown("placement"));
    }
    Ok(Read::Frame(Frame { unit, placement }))
}

pub(super) fn check(
    models: &[&Building],
    rule: &Value,
    out: &mut Out<'_>,
    cx: &mut Cx<'_>,
) -> Result<(), Error> {
    let positive = |key| {
        rule.get_key(key)
            .and_then(crate::number)
            .filter(|n| n.is_finite() && *n > 0.0)
    };
    let (Some(length), Some(axis)) = (positive("length-tolerance"), positive("axis-tolerance"))
    else {
        return finding(
            out,
            codes::PROFILE_MODELS_ALIGNED_UNSUPPORTED,
            0,
            "tolerances",
            None,
            true,
            cx,
        );
    };
    if models.is_empty() {
        return finding(
            out,
            codes::PROFILE_MODELS_ALIGNED_UNSUPPORTED,
            0,
            "models",
            None,
            true,
            cx,
        );
    }
    if models.len() == 1 {
        return Ok(());
    }
    let mut reference = None;
    for (index, model) in models.iter().enumerate() {
        cx.check(Kind::Work, 1)?;
        cx.check(
            Kind::Work,
            model
                .document
                .map
                .as_ref()
                .and_then(|map| map.crs_name.as_ref())
                .map_or(0, String::len) as u64,
        )?;
        let current = match frame(model, length, axis, cx)? {
            Read::Frame(frame) => frame,
            Read::Unknown(reason) => {
                finding(
                    out,
                    codes::PROFILE_MODELS_ALIGNED_UNSUPPORTED,
                    index + 1,
                    reason,
                    None,
                    true,
                    cx,
                )?;
                continue;
            }
        };
        if let Some((previous, map)) = reference.as_ref() {
            let previous: &Frame = previous;
            let reason = if previous.unit != current.unit {
                Some("length-unit")
            } else if !equal(previous.placement, current.placement, length, axis) {
                Some("origin-or-axis")
            } else {
                None
            };
            if let Some(reason) = reason {
                finding(
                    out,
                    codes::PROFILE_MODELS_ALIGNED_INVALID,
                    index + 1,
                    reason,
                    None,
                    false,
                    cx,
                )?;
            }
            if *map != model.document.map.as_ref() {
                finding(
                    out,
                    codes::PROFILE_MODELS_ALIGNED_UNSUPPORTED,
                    index + 1,
                    "map-frame",
                    None,
                    true,
                    cx,
                )?;
            }
        } else {
            reference = Some((current, model.document.map.as_ref()));
        }
    }
    Ok(())
}
