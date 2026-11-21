//! 建物の形を切り、元の要素と形の項目への対応を持つ図面へ投影する。
//!
//! 長さはプロジェクトの文書単位、Z が上。表示と切断の既定値は呼び出し側の
//! プロファイルが渡す。元の建物・形・ID は変更しない。

mod budget;
mod bvh;
mod grid;
mod joinery;
mod output;
mod plane;
mod project;
mod visibility;

pub use grid::GridView;
pub use joinery::{AttributeSymbol, JoinerySymbol};
pub use output::{Line, Output, Paper, Role, Source, Statistics};
pub use plane::{Frame, Side, View};

use zumen_base::{Cx, Diagnostic, Kind, codes};
use zumen_geom2d::topology::Grid;
use zumen_geom3d::Failure;

use crate::{Building, ElementRef, GeometryRef, Opening, evaluation};
use budget::{push, reserve, work};
use output::Stroke;
use visibility::Visibility;

/// 切り出しの設定。長さと弦の誤差は文書の単位、用紙だけ mm。
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Options<'a> {
    /// 切る面と見る深さ。
    pub view: View,
    /// 整数格子と可視性の判定に使う、正の有限な許容差。
    pub tolerance: f64,
    /// 形の曲線を近似する弦の誤差。
    pub chord: f64,
    /// 縮尺と紙上の位置。
    pub paper: Paper,
    /// 建具の開き方に対応する、プロファイルの記号。既定の記号は補わない。
    pub symbols: &'a [JoinerySymbol<'a>],
}

fn source(
    building: &Building,
    at: ElementRef,
    item: Option<GeometryRef>,
    cx: &mut Cx<'_>,
) -> Result<Source, Failure> {
    let element = building
        .element(at)
        .ok_or(Failure::Invalid("drawing_element"))?;
    let node = item.and_then(|item| {
        building
            .geometry(at)
            .and_then(|g| g.data().nodes.get(item.0 as usize))
    });
    let n = element
        .source()
        .global_id
        .as_ref()
        .map_or(0, |v| v.len() as u64);
    if n > i32::MAX as u64 {
        return Err(Failure::Invalid("drawing_size"));
    }
    cx.check(Kind::DecodedBytes, n)?;
    work(cx, n)?;
    Ok(Source {
        element: at,
        id: element.id(),
        global_id: element.source().global_id.clone(),
        instance: element.source().instance,
        item,
        shape_instance: node.and_then(|n| n.source.instance),
    })
}

fn reason(error: &Failure) -> &str {
    match error {
        Failure::Invalid(value) | Failure::Unsupported(value) => value,
        Failure::Failed(value) => value,
        Failure::Stopped(_) => "stopped",
    }
}

fn report_snaps(
    source: &Source,
    spacing: f64,
    snapped: u64,
    cx: &mut Cx<'_>,
) -> Result<(), Failure> {
    for _ in 0..snapped {
        work(cx, 1)?;
        cx.report(
            Diagnostic::new(codes::BUILDING_GRID_APPROXIMATED)
                .with("spacing", spacing)
                .at(source.place()),
        );
    }
    work(cx, 0)
}

fn report_loss(
    source: &Source,
    reason: &'static str,
    count: u64,
    cx: &mut Cx<'_>,
) -> Result<(), Failure> {
    for _ in 0..count {
        work(cx, 1)?;
        cx.report(
            Diagnostic::new(codes::BUILDING_LINE_DROPPED)
                .with("reason", reason)
                .at(source.place()),
        );
    }
    work(cx, 0)
}

fn drawing_unit(
    building: &Building,
    cx: &mut Cx<'_>,
) -> Result<(zumen_drawing::Unit, f64), Failure> {
    work(cx, building.document.units.len() as u64)?;
    let mut units = building
        .document
        .units
        .iter()
        .filter(|u| u.kind == "LENGTHUNIT");
    let first = units.next().ok_or(Failure::Invalid("drawing_unit"))?;
    let scale = evaluation::length_scale(first)?;
    for unit in units {
        if evaluation::length_scale(unit)? != scale {
            return Err(Failure::Invalid("drawing_unit"));
        }
    }
    use zumen_drawing::Unit::*;
    let unit = [Millimeter, Centimeter, Meter, Inch, Foot]
        .into_iter()
        .find(|u| u.millimeters().is_some_and(|mm| mm / 1000.0 == scale))
        .ok_or(Failure::Unsupported("drawing_unit"))?;
    Ok((unit, scale))
}

/// 形状を評価し、平断面・断面・立面を元要素との対応を持つ図面へ切り出す。
///
/// 開口は母体から引き、開口そのものを重ねて描かない。未対応の形は出所付きで診断し、
/// 予算停止と中断では部分結果を返さない。元の建物の値と ID は変更しない。
pub fn cut(building: &Building, options: Options<'_>, cx: &mut Cx<'_>) -> Result<Output, Failure> {
    cut_with_attribute_symbols(building, options, &[], cx)
}

/// 型付きの拡張属性に対応する記号も渡して切り出す。既存のプロパティによる記号と競合すれば診断する。
pub fn cut_with_attribute_symbols(
    building: &Building,
    options: Options<'_>,
    symbols: &[AttributeSymbol<'_>],
    cx: &mut Cx<'_>,
) -> Result<Output, Failure> {
    let frame = options.view.frame()?;
    let grid = Grid::new([0.0, 0.0], options.tolerance)
        .map_err(|_| Failure::Invalid("drawing_tolerance"))?;
    let (unit, units) = drawing_unit(building, cx)?;
    output::validate_paper(options.paper, units)?;
    if !options.chord.is_finite() || options.chord <= 0.0 || !(options.chord * units).is_finite() {
        return Err(Failure::Invalid("drawing_chord"));
    }
    let mut prepared = joinery::prepare(building, options, symbols, frame, grid, units, cx)?;
    grid::draw(
        building,
        joinery::Settings {
            frame,
            grid,
            units,
            chord: options.chord * units,
        },
        &mut prepared,
        cx,
    )?;
    let evaluated = evaluation::evaluate_selected(
        building,
        evaluation::Options {
            chord: options.chord * units,
            openings: true,
        },
        |at| !prepared.skipped.contains(&at),
        cx,
    )?;
    let mut sources = prepared.sources;
    let mut edges = Vec::new();
    let mut surfaces = Vec::new();
    let mut complete = prepared.complete;
    let mut statistics = Statistics {
        elements: evaluated.len() as u64,
        collapsed: prepared.collapsed,
        ..Statistics::default()
    };
    for element in evaluated {
        work(cx, 1)?;
        if matches!(
            building.components.opening.get(element.element),
            Some(Opening::Voids(_))
        ) {
            continue;
        }
        complete &= element.complete;
        statistics.evaluation_failures += u64::from(!element.complete);
        // 室・空間区域は占める範囲であり、室内の階段や建具を隠す物体ではない。
        let opaque = !building
            .components
            .spatial
            .get(element.element)
            .is_some_and(|s| {
                matches!(
                    s.kind,
                    crate::SpatialKind::Space | crate::SpatialKind::SpatialZone
                )
            });
        let mut fallback_reported = false;
        for part in element.parts {
            let source = source(building, element.element, Some(part.item), cx)?;
            let at = sources.len();
            match project::part(&part, frame, units, grid, at, cx) {
                Ok(projected) => {
                    let losses = projected.collapsed_edges + projected.collapsed_contours;
                    complete &= losses == 0;
                    statistics.collapsed += losses;
                    if !prepared.replaced.contains(&element.element)
                        && !fallback_reported
                        && !projected.edges.is_empty()
                        && matches!(options.view, View::Plan { .. })
                        && joinery::is_joinery(building, element.element)
                    {
                        cx.report(
                            Diagnostic::new(codes::BUILDING_JOINERY_DRAWN)
                                .with("reason", "plan_and_symbol_unavailable")
                                .at(source.place()),
                        );
                        work(cx, 0)?;
                        fallback_reported = true;
                    }
                    report_snaps(&source, grid.spacing(), projected.snapped, cx)?;
                    report_loss(&source, "projected_edge", projected.collapsed_edges, cx)?;
                    report_loss(&source, "cut_contour", projected.collapsed_contours, cx)?;
                    reserve(&mut edges, projected.edges.len(), 64, cx)?;
                    reserve(&mut surfaces, projected.surfaces.len(), 80, cx)?;
                    statistics.parts += 1;
                    statistics.triangles += part.mesh.triangles.len() as u64;
                    if !prepared.replaced.contains(&element.element) {
                        edges.extend(projected.edges);
                    }
                    if opaque {
                        surfaces.extend(projected.surfaces);
                    }
                }
                Err(Failure::Stopped(stop)) => return Err(Failure::Stopped(stop)),
                Err(error) => {
                    statistics.projection_failures += 1;
                    complete = false;
                    cx.report(
                        Diagnostic::new(codes::BUILDING_DRAWING_DROPPED)
                            .with("reason", reason(&error))
                            .at(source.place()),
                    );
                    work(cx, 0)?;
                }
            }
            push(&mut sources, source, 112, cx)?;
        }
    }
    statistics.surfaces = surfaces.len() as u64;
    statistics.edges = edges.len() as u64;
    let visibility = Visibility::new(surfaces, options.tolerance, cx)?;
    let mut strokes = prepared.strokes;
    for edge in edges {
        work(cx, 1)?;
        let positions = [
            grid.position(edge.vertices[0].point),
            grid.position(edge.vertices[1].point),
        ];
        let [Ok(a), Ok(b)] = positions else {
            return Err(Failure::Invalid("drawing_grid"));
        };
        if edge.cut {
            push(
                &mut strokes,
                Stroke {
                    ends: [a, b],
                    source: edge.source,
                    role: Role::Cut,
                },
                48,
                cx,
            )?;
            continue;
        }
        let split = visibility.split(edge.vertices, cx)?;
        let mut snapped = split.rounded;
        let mut collapsed = 0;
        for span in split.spans {
            let raw = span
                .range
                .map(|t| [a[0] * (1.0 - t) + b[0] * t, a[1] * (1.0 - t) + b[1] * t]);
            let [Ok(first), Ok(last)] = raw.map(|p| grid.snap(p)) else {
                return Err(Failure::Invalid("drawing_grid"));
            };
            snapped += u64::from(first.offset != [0.0, 0.0]) + u64::from(last.offset != [0.0, 0.0]);
            if first.point == last.point {
                collapsed += 1;
                continue;
            }
            let [Ok(first), Ok(last)] = [grid.position(first.point), grid.position(last.point)]
            else {
                return Err(Failure::Invalid("drawing_grid"));
            };
            cx.check(Kind::Segments, 1)?;
            push(
                &mut strokes,
                Stroke {
                    ends: [first, last],
                    source: edge.source,
                    role: if span.hidden {
                        Role::Hidden
                    } else {
                        Role::Visible
                    },
                },
                48,
                cx,
            )?;
        }
        let source = sources
            .get(edge.source)
            .ok_or(Failure::Invalid("drawing_source"))?;
        report_snaps(source, grid.spacing(), snapped, cx)?;
        report_loss(source, "visibility_span", collapsed, cx)?;
        complete &= collapsed == 0;
        statistics.collapsed += collapsed;
    }
    canonical(&mut strokes, &sources, cx)?;
    let mut out = output::write(strokes, sources, frame, grid, unit, options.paper, cx)?;
    statistics.lines = out.lines.len() as u64;
    out.statistics = statistics;
    out.complete = complete;
    work(cx, 0)?;
    Ok(out)
}

fn canonical(
    strokes: &mut Vec<Stroke>,
    sources: &[Source],
    cx: &mut Cx<'_>,
) -> Result<(), Failure> {
    let point = |a: &[f64; 2], b: &[f64; 2]| a[0].total_cmp(&b[0]).then(a[1].total_cmp(&b[1]));
    let n = strokes.len() as u64;
    work(
        cx,
        n.saturating_mul(65 - n.leading_zeros() as u64)
            .saturating_mul(8),
    )?;
    for stroke in strokes.iter_mut() {
        if point(&stroke.ends[0], &stroke.ends[1]).is_gt() {
            stroke.ends.swap(0, 1);
        }
    }
    let ends =
        |a: &Stroke, b: &Stroke| point(&a.ends[0], &b.ends[0]).then(point(&a.ends[1], &b.ends[1]));
    strokes.sort_unstable_by(|a, b| {
        a.source
            .cmp(&b.source)
            .then(ends(a, b))
            .then(a.role.cmp(&b.role))
    });
    strokes.dedup_by(|a, b| a.source == b.source && a.ends == b.ends);
    for stroke in strokes.iter() {
        if sources.get(stroke.source).is_none() {
            return Err(Failure::Invalid("drawing_source"));
        }
    }
    let key = |source: usize| sources.get(source).map(|s| (s.id, s.shape_instance));
    strokes.sort_unstable_by(|a, b| {
        key(a.source)
            .map(|s| s.0)
            .cmp(&key(b.source).map(|s| s.0))
            .then(a.role.cmp(&b.role))
            .then(ends(a, b))
            .then(key(a.source).cmp(&key(b.source)))
            .then(a.source.cmp(&b.source))
    });
    Ok(())
}
