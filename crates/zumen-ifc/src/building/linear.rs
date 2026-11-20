//! IFC4.3 ADD2 の `IfcLinearPlacement`（8.7.3.13）・`IfcAxis2PlacementLinear`（8.9.3.4）・
//! `IfcPointByDistanceExpression`（8.9.3.48）を、線形の曲線または `CartesianPosition` から評価する（#376）。

#[cfg(feature = "civil")]
use zumen_base::Kind;
#[cfg(feature = "civil")]
use zumen_building::Transform;
use zumen_sdk::Cx;

use super::{Fold, Lifter, Step, Version, arg, dot, normalize, pad, reference};
#[cfg(feature = "civil")]
use super::{cross, prefix_scale, stopped};
use crate::{Record, Value};

#[cfg(feature = "civil")]
pub(super) struct LinearPlacements {
    length_scale: f64,
    alignments: Vec<crate::civil::LocalAlignment>,
}

impl<'f> Lifter<'f> {
    pub(super) fn linear_placement(
        &mut self,
        id: u64,
        record: &'f Record,
        cx: &mut Cx,
    ) -> Result<Step, Fold> {
        self.used.insert(id);
        if self.version != Version::Ifc4x3 {
            return Ok(Step::Unsupported("IfcLinearPlacement"));
        }
        if !matches!(arg(record, 1), Some(Value::Null | Value::Ref(_)) | None) {
            return Err(Fold::Invalid);
        }
        let cartesian = match arg(record, 3) {
            Some(Value::Null) | None => None,
            Some(Value::Ref(cp)) => Some(*cp),
            _ => return Err(Fold::Invalid),
        };
        let axes_id = reference(arg(record, 2)).ok_or(Fold::Invalid)?;
        if self.file.instance(axes_id).is_none() {
            return match cartesian {
                Some(cp) => self.axes(cp).map(Step::Local).ok_or(Fold::Invalid),
                None => Ok(Step::Unsupported("IfcLinearPlacement")),
            };
        }
        if !self.is(axes_id, b"IFCAXIS2PLACEMENTLINEAR") {
            return Err(Fold::Invalid);
        }
        let axes_rec = self.take(axes_id).ok_or(Fold::Invalid)?;
        let point_id = reference(arg(axes_rec, 1)).ok_or(Fold::Invalid)?;
        if !self.is(point_id, b"IFCPOINTBYDISTANCEEXPRESSION") {
            return Err(Fold::Invalid);
        }
        let point_rec = self.take(point_id).ok_or(Fold::Invalid)?;
        let distance = self
            .real(arg(point_rec, 1))
            .filter(|v| v.is_finite())
            .ok_or(Fold::Invalid)?;
        let lat = self.optional_real(point_rec, 2)?;
        let vert = self.optional_real(point_rec, 3)?;
        let lon = self.optional_real(point_rec, 4)?;
        let axis_dir = self.optional_direction(axes_rec, 2)?;
        let ref_dir = self.optional_direction(axes_rec, 3)?;
        if let (Some(a), Some(r)) = (axis_dir, ref_dir) {
            let z = normalize(a).ok_or(Fold::Invalid)?;
            let along = dot(r, z);
            normalize([
                r[0] - along * z[0],
                r[1] - along * z[1],
                r[2] - along * z[2],
            ])
            .ok_or(Fold::Invalid)?;
        }
        let curve_id = reference(arg(point_rec, 5)).ok_or(Fold::Invalid)?;

        #[cfg(feature = "civil")]
        if let Some(step) = self.eval_on_alignment(
            record,
            curve_id,
            [distance, lat, vert, lon],
            axis_dir,
            ref_dir,
            cartesian,
            cx,
        )? {
            return Ok(step);
        }
        let _ = (distance, lat, vert, lon, curve_id, cx);

        match cartesian {
            Some(cp) => self.axes(cp).map(Step::Local).ok_or(Fold::Invalid),
            None => Ok(Step::Unsupported("IfcLinearPlacement")),
        }
    }

    #[cfg(feature = "civil")]
    #[allow(clippy::too_many_arguments)]
    fn eval_on_alignment(
        &mut self,
        record: &Record,
        curve_id: u64,
        [distance, lat, vert, lon]: [f64; 4],
        axis_dir: Option<[f64; 3]>,
        ref_dir: Option<[f64; 3]>,
        cartesian: Option<u64>,
        cx: &mut Cx,
    ) -> Result<Option<Step>, Fold> {
        if self.linear.is_none() {
            let length_scale = self.document_scale("LENGTHUNIT");
            let angle_scale = self.document_scale("PLANEANGLEUNIT");
            let alignments =
                crate::civil::local_alignments(self.file, length_scale, angle_scale, cx)
                    .map_err(Fold::Stopped)?;
            self.linear = Some(LinearPlacements {
                length_scale,
                alignments,
            });
        }
        let found = {
            let Some(linear) = self.linear.as_ref() else {
                return Ok(None);
            };
            let length_scale = linear.length_scale;
            let matched = linear.alignments.iter().find_map(|item| {
                if item.gradient_curves.contains(&curve_id) {
                    Some((item, true))
                } else if item.composite_curves.contains(&curve_id) {
                    Some((item, false))
                } else {
                    None
                }
            });
            let Some((item, is_gradient)) = matched else {
                return Ok(None);
            };
            cx.check(Kind::Work, zumen_civil::place_work(&item.alignment))
                .map_err(|stop| Fold::Stopped(stopped(cx, stop, Some(record.span()))))?;
            let placed = zumen_civil::place_within(&item.alignment, distance * length_scale, cx)
                .map_err(|stop| Fold::Stopped(stopped(cx, stop, Some(record.span()))))?;
            let Ok(placed) = placed else {
                return Err(Fold::Invalid);
            };
            let (z0_m, g) = if is_gradient {
                let (Some(h), Some(grad)) = (placed.height, placed.gradient) else {
                    return Err(Fold::Invalid);
                };
                (h, grad)
            } else {
                (0.0, 0.0)
            };
            (
                length_scale,
                item.placement,
                placed.point,
                placed.direction,
                z0_m,
                g,
            )
        };
        let (length_scale, align_placement, point_m, direction, z0_m, g) = found;
        let tx_h = libm::sin(direction);
        let ty_h = libm::cos(direction);
        let inv_hypot = 1.0 / libm::hypot(1.0, g);
        let t = [tx_h * inv_hypot, ty_h * inv_hypot, g * inv_hypot];
        let l = [-ty_h, tx_h, 0.0];
        let n = [-g * tx_h * inv_hypot, -g * ty_h * inv_hypot, inv_hypot];

        let base = [
            point_m.easting / length_scale,
            point_m.northing / length_scale,
            z0_m / length_scale,
        ];
        let location = [
            base[0] + lat * l[0] + vert * n[0] + lon * t[0],
            base[1] + lat * l[1] + vert * n[1] + lon * t[1],
            base[2] + vert * n[2] + lon * t[2],
        ];
        if !location.iter().all(|v| v.is_finite()) {
            return Err(Fold::Invalid);
        }

        let reference_direction = match ref_dir {
            Some(r) => normalize(r).ok_or(Fold::Invalid)?,
            None => t,
        };
        let z = match axis_dir {
            Some(a) => normalize(a).ok_or(Fold::Invalid)?,
            None => {
                let [rx, ry, rz] = reference_direction;
                normalize([-rz * rx, -rz * ry, 1.0 - rz * rz]).ok_or(Fold::Invalid)?
            }
        };
        let along = dot(reference_direction, z);
        let [rx, ry, rz] = reference_direction;
        let [zx, zy, zz] = z;
        let x =
            normalize([rx - along * zx, ry - along * zy, rz - along * zz]).ok_or(Fold::Invalid)?;
        let [xx, xy, xz] = x;
        let [yx, yy, yz] = cross(z, x);
        let [tx, ty, tz] = location;
        let t_local = Transform([[xx, yx, zx, tx], [xy, yy, zy, ty], [xz, yz, zz, tz]]);

        if let Some(cp) = cartesian {
            let _ = self.axes(cp);
        }

        if reference(arg(record, 1)).is_none()
            && let Some(align_pl) = align_placement
        {
            let t_align = self.alignment_transform(align_pl, cx)?;
            return Ok(Some(Step::Local(t_align.then(&t_local))));
        }
        Ok(Some(Step::Local(t_local)))
    }

    #[cfg(feature = "civil")]
    fn document_scale(&self, kind: &str) -> f64 {
        self.building
            .document
            .units
            .iter()
            .find(|u| u.kind == kind)
            .map(|u| u.scale.unwrap_or_else(|| prefix_scale(u)))
            .filter(|s| s.is_finite() && *s > 0.0)
            .unwrap_or(1.0)
    }

    #[cfg(feature = "civil")]
    fn alignment_transform(&mut self, start: u64, cx: &mut Cx) -> Result<Transform, Fold> {
        let mut depth = 0usize;
        let result = (|| {
            let mut chain: Vec<(u64, Transform)> = Vec::new();
            let mut at = Some(start);
            while let Some(id) = at {
                if chain.iter().any(|&(seen, _)| seen == id) {
                    return Err(Fold::Invalid);
                }
                let record = self.file.instance(id).ok_or(Fold::Invalid)?;
                if !self.is(id, b"IFCLOCALPLACEMENT") {
                    return Err(Fold::Invalid);
                }
                cx.enter()
                    .map_err(|stop| Fold::Stopped(stopped(cx, stop, Some(record.span()))))?;
                depth += 1;
                self.used.insert(id);
                let axes = reference(arg(record, 2)).ok_or(Fold::Invalid)?;
                let transform = self.axes(axes).ok_or(Fold::Invalid)?;
                chain.push((id, transform));
                at = reference(arg(record, 1));
            }
            let identity = Transform([
                [1.0, 0.0, 0.0, 0.0],
                [0.0, 1.0, 0.0, 0.0],
                [0.0, 0.0, 1.0, 0.0],
            ]);
            Ok(chain
                .into_iter()
                .rev()
                .fold(identity, |parent, (_, child)| parent.then(&child)))
        })();
        for _ in 0..depth {
            cx.leave();
        }
        result
    }

    fn optional_real(&self, record: &Record, at: usize) -> Result<f64, Fold> {
        match arg(record, at) {
            Some(Value::Null) | None => Ok(0.0),
            value => self
                .real(value)
                .filter(|v| v.is_finite())
                .ok_or(Fold::Invalid),
        }
    }

    fn optional_direction(&mut self, record: &Record, at: usize) -> Result<Option<[f64; 3]>, Fold> {
        match arg(record, at) {
            Some(Value::Null) | None => Ok(None),
            Some(Value::Ref(id)) => pad(&self.direction(*id).ok_or(Fold::Invalid)?)
                .map(Some)
                .ok_or(Fold::Invalid),
            _ => Err(Fold::Invalid),
        }
    }
}
