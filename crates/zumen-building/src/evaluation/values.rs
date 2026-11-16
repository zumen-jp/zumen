//! 元の単位と配置を、評価用のメートルの座標へ写す。

use crate::{AxisPlacement3, CartesianMapping3, Coordinates, MappingScale3, ShapeMapping3, Unit};
use zumen_base::{Cx, Kind};
use zumen_geom3d::{Failure, Transform, Xyz, cross, dot, normalize, scale, sub};

pub(super) fn reserve<T>(
    out: &mut Vec<T>,
    additional: usize,
    slot: u64,
    cx: &mut Cx,
) -> Result<(), Failure> {
    let n = out
        .len()
        .checked_add(additional)
        .ok_or(Failure::Invalid("size"))?;
    let limit = (i32::MAX as u64 / slot.max(1)) as usize;
    if n > limit {
        return Err(Failure::Invalid("item_count"));
    }
    if n > out.capacity() {
        let cap = n.max(out.capacity().saturating_mul(2)).clamp(4, limit);
        cx.check(Kind::DecodedBytes, (cap - out.capacity()) as u64 * slot)?;
        out.try_reserve_exact(cap - out.len())
            .map_err(|_| Failure::Failed("allocation".into()))?;
    }
    Ok(())
}

pub(super) fn coordinates(value: Coordinates) -> Xyz {
    match value {
        Coordinates::Two([x, y]) => [x, y, 0.0],
        Coordinates::Three(p) => p,
    }
}

pub(super) fn unit(unit: Option<&Unit>, kind: &str) -> Result<f64, Failure> {
    let unit = unit.ok_or(Failure::Invalid("unit_missing"))?;
    if unit.kind != kind {
        return Err(Failure::Invalid("unit_kind"));
    }
    let value = if let Some(scale) = unit.scale {
        scale
    } else {
        let expected = match kind {
            "LENGTHUNIT" => "METRE",
            "PLANEANGLEUNIT" => "RADIAN",
            _ => return Err(Failure::Unsupported("unit")),
        };
        if unit.name != expected {
            return Err(Failure::Unsupported("unit"));
        }
        match unit.prefix.as_deref() {
            None => 1.0,
            Some("EXA") => 1e18,
            Some("PETA") => 1e15,
            Some("TERA") => 1e12,
            Some("GIGA") => 1e9,
            Some("MEGA") => 1e6,
            Some("KILO") => 1e3,
            Some("HECTO") => 1e2,
            Some("DECA") => 1e1,
            Some("DECI") => 1e-1,
            Some("CENTI") => 1e-2,
            Some("MILLI") => 1e-3,
            Some("MICRO") => 1e-6,
            Some("NANO") => 1e-9,
            Some("PICO") => 1e-12,
            Some("FEMTO") => 1e-15,
            Some("ATTO") => 1e-18,
            _ => return Err(Failure::Unsupported("unit_prefix")),
        }
    };
    if !(value.is_finite() && value > 0.0) {
        return Err(Failure::Invalid("unit_scale"));
    }
    Ok(value)
}

pub(crate) fn placement(value: Option<&AxisPlacement3>, factor: f64) -> Result<Transform, Failure> {
    let Some(value) = value else {
        return Ok(Transform::IDENTITY);
    };
    Transform::from_axes(
        scale(value.location, factor),
        value.axis,
        value.reference_direction,
    )
    .ok_or(Failure::Invalid("axis"))
}

pub(super) fn inverse(m: Transform) -> Result<Transform, Failure> {
    let (x, y, z, o) = (m.column(0), m.column(1), m.column(2), m.column(3));
    let determinant = dot(x, cross(y, z));
    if !m.is_finite() || !determinant.is_finite() || determinant == 0.0 {
        return Err(Failure::Invalid("inverse"));
    }
    let row = |v: Xyz| {
        let v = scale(v, 1.0 / determinant);
        [v[0], v[1], v[2], -dot(v, o)]
    };
    let result = Transform([row(cross(y, z)), row(cross(z, x)), row(cross(x, y))]);
    if !result.is_finite() {
        return Err(Failure::Invalid("inverse"));
    }
    Ok(result)
}

fn operator(value: &CartesianMapping3, factor: f64) -> Result<Transform, Failure> {
    // 取得時に保持した方向比を正規化し、第二軸の指定で鏡映の向きを選ぶ。
    let frame = Transform::from_axes(scale(value.local_origin, factor), value.axis3, value.axis1)
        .ok_or(Failure::Invalid("mapping_axis"))?;
    let x = frame.column(0);
    let mut y = frame.column(1);
    let z = frame.column(2);
    if let Some(axis) = value.axis2 {
        let axis = normalize(sub(
            sub(axis, scale(z, dot(axis, z))),
            scale(x, dot(axis, x)),
        ))
        .ok_or(Failure::Invalid("mapping_axis"))?;
        if dot(axis, y) < 0.0 {
            y = scale(y, -1.0);
        }
    }
    let [sx, sy, sz] = match value.scale {
        MappingScale3::Uniform(s) => [s.unwrap_or(1.0); 3],
        MappingScale3::NonUniform { x, y, z } => {
            let x = x.unwrap_or(1.0);
            [x, y.unwrap_or(x), z.unwrap_or(x)]
        }
    };
    if [sx, sy, sz].iter().any(|s| !s.is_finite() || *s <= 0.0) {
        return Err(Failure::Invalid("mapping_scale"));
    }
    let (x, y, z, o) = (scale(x, sx), scale(y, sy), scale(z, sz), frame.column(3));
    Ok(Transform([
        [x[0], y[0], z[0], o[0]],
        [x[1], y[1], z[1], o[1]],
        [x[2], y[2], z[2], o[2]],
    ]))
}

pub(crate) fn mapping(value: &ShapeMapping3, factor: f64) -> Result<Transform, Failure> {
    Ok(inverse(placement(Some(&value.origin), factor)?)?.then(&operator(&value.target, factor)?))
}
