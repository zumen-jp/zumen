//! 直線の通り芯から鉛直面を作る。曲がった通り芯を勝手に直線へ置き換えない。

use super::{Role, Side, View, budget::work, drawing_unit, joinery};
use crate::{Acquired, Building, Coordinates, CurveKind, ElementRef, GeometryGridAxis};
use zumen_base::{Cx, Diagnostic, Kind, codes};
use zumen_crs::ProjectPoint;
use zumen_geom3d::{Failure, add, normalize, scale};

pub(super) fn draw(
    building: &Building,
    settings: joinery::Settings,
    out: &mut joinery::Prepared,
    cx: &mut Cx<'_>,
) -> Result<(), Failure> {
    for index in 0..building.elements().len() {
        work(cx, 1)?;
        let owner = ElementRef(index as u32);
        let Some(crate::Grid::Grid { axes }) = building.components.grid.get(owner) else {
            continue;
        };
        // 通り芯の表現は 3D の実体として重ねない。各軸の曲線から描く。
        cx.check(Kind::DecodedBytes, 64)?;
        out.skipped.insert(owner);
        for &(_, at) in axes {
            work(cx, 1)?;
            let previous_sources = out.sources.len();
            let previous_strokes = out.strokes.len();
            let result = (|| {
                let value = axis(building, owner, at, cx)?;
                let placement = joinery::world(building, owner, settings.units, cx)?;
                let mut source = super::source(building, at, None, cx)?;
                source.shape_instance = value
                    .curve
                    .item(value.curve.root)
                    .and_then(|c| c.source.instance);
                joinery::draw_curve(
                    &value.curve,
                    placement,
                    source,
                    Role::Grid,
                    settings,
                    out,
                    cx,
                )
            })();
            if let Err(error) = result {
                if matches!(error, Failure::Stopped(_)) {
                    return Err(error);
                }
                out.sources.truncate(previous_sources);
                out.strokes.truncate(previous_strokes);
                out.complete = false;
                cx.report(
                    Diagnostic::new(codes::BUILDING_DRAWING_DROPPED)
                        .with("reason", super::reason(&error))
                        .at(building.place(at)),
                );
                work(cx, 0)?;
            }
        }
    }
    Ok(())
}

/// 通り芯と高さで指定する断面または立面。高さと深さは文書単位。
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GridView {
    /// 通り芯を持つ建物の要素。
    pub grid: ElementRef,
    /// 面を決める軸の要素。
    pub axis: ElementRef,
    /// 図面の原点とするプロジェクトの Z。
    pub elevation: f64,
    /// 軸の向きに対する見る側。
    pub side: Side,
    /// 面の向こうへ見る深さ。
    pub depth: f64,
    /// 断面なら真、立面なら偽。
    pub section: bool,
}

pub(super) fn axis<'a>(
    building: &'a Building,
    owner: ElementRef,
    axis: ElementRef,
    cx: &mut Cx<'_>,
) -> Result<&'a GeometryGridAxis, Failure> {
    let Some(crate::Grid::Grid { axes }) = building.components.grid.get(owner) else {
        return Err(Failure::Invalid("drawing_grid_owner"));
    };
    work(cx, axes.len() as u64)?;
    let mut matches = axes.iter().enumerate().filter(|(_, (_, at))| *at == axis);
    let (index, (set, _)) = matches
        .next()
        .ok_or(Failure::Invalid("drawing_grid_axis"))?;
    if matches.next().is_some() {
        return Err(Failure::Invalid("drawing_grid_axis"));
    }
    let Some(Acquired::Value(grid)) = building
        .geometry(owner)
        .and_then(|g| g.data().grid.as_ref())
    else {
        return Err(Failure::Unsupported("drawing_grid"));
    };
    let value = grid
        .axes
        .get(index)
        .ok_or(Failure::Invalid("drawing_grid_axis"))?;
    if value.definition.set != *set {
        return Err(Failure::Invalid("drawing_grid_axis"));
    }
    let original = building.element(axis).and_then(|e| e.source().instance);
    if original.is_some() && value.source.instance.is_some() && original != value.source.instance {
        return Err(Failure::Invalid("drawing_grid_axis"));
    }
    Ok(value)
}

impl GridView {
    /// 直線、または 2 点の折れ線の通り芯から面を作る。
    /// 曲がった軸・同じ 2 点・鉛直方向を持つ軸には面を補わず失敗を返す。
    pub fn view(self, building: &Building, cx: &mut Cx<'_>) -> Result<View, Failure> {
        let (_, units) = drawing_unit(building, cx)?;
        let axis = axis(building, self.grid, self.axis, cx)?;
        let graph = &axis.curve;
        let factor = crate::evaluation::length_scale(
            graph
                .length_unit
                .as_ref()
                .ok_or(Failure::Invalid("unit_missing"))?,
        )?;
        let item = graph
            .item(graph.root)
            .ok_or(Failure::Invalid("curve_reference"))?;
        let xyz = |value| match value {
            Coordinates::Two([x, y]) => [x, y, 0.0],
            Coordinates::Three(p) => p,
        };
        let pair = match &item.kind {
            CurveKind::Polyline(points) => {
                let [a, b] = points.as_slice() else {
                    return Err(Failure::Unsupported("grid_plane_curve"));
                };
                [xyz(*a), xyz(*b)]
            }
            CurveKind::Line {
                point,
                direction,
                magnitude,
            } => {
                if !magnitude.is_finite() || *magnitude <= 0.0 {
                    return Err(Failure::Invalid("grid_plane_curve"));
                }
                let direction =
                    normalize(xyz(*direction)).ok_or(Failure::Invalid("grid_plane_curve"))?;
                [xyz(*point), add(xyz(*point), scale(direction, *magnitude))]
            }
            _ => return Err(Failure::Unsupported("grid_plane_curve")),
        };
        let world = joinery::world(building, self.grid, units, cx)?;
        let [mut a, mut b] = pair.map(|p| scale(world.point(scale(p, factor)), 1.0 / units));
        if !a.into_iter().chain(b).all(f64::is_finite) || a[2] != b[2] {
            return Err(Failure::Invalid("grid_plane_curve"));
        }
        if !axis.definition.same_sense {
            std::mem::swap(&mut a, &mut b);
        }
        let point = |p: [f64; 3]| ProjectPoint {
            x: p[0],
            y: p[1],
            z: self.elevation,
        };
        let view = View::Vertical {
            start: point(a),
            end: point(b),
            side: self.side,
            depth: self.depth,
            section: self.section,
        };
        view.frame()?;
        Ok(view)
    }
}
