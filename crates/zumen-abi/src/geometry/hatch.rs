//! 非連動のハッチングの追加と、値の取得。

use super::{Point, Spot};
use crate::messages as api;
use zumen_drawing::{
    Boundary, Edge, Fill, Hatch, HatchLoop, Islands, Pattern, PatternLine, PatternOrigin, Vertex,
};

/// API の応答へ境界・模様・種点を複製する前に、入れ子の列にも作業量の上限を当てる。
pub(crate) fn work<P>(hatch: &Hatch<P>, cx: &mut zumen_base::Cx) -> Result<(), zumen_base::Stop> {
    use zumen_base::Kind;
    cx.check(
        Kind::Work,
        (hatch.loops.len() as u64).saturating_add(hatch.seeds.len() as u64),
    )?;
    for path in &hatch.loops {
        let count = match &path.boundary {
            Boundary::Polyline { vertices, .. } => vertices.len(),
            Boundary::Edges(edges) => edges.len(),
        };
        cx.check(Kind::Work, count as u64)?;
    }
    if let Fill::Pattern(pattern) = &hatch.fill {
        cx.check(
            Kind::Work,
            (pattern.name.len() as u64).saturating_add(pattern.lines.len() as u64),
        )?;
        for line in &pattern.lines {
            cx.check(Kind::Work, line.dashes.len() as u64)?;
        }
    }
    Ok(())
}

pub(super) fn geometry_of<P: Spot>(hatch: &Hatch<P>) -> Option<api::HatchGeometry> {
    if hatch.associative || hatch.loops.is_empty() {
        return None;
    }
    let loops = hatch
        .loops
        .iter()
        .map(|path| {
            if path.derived || path.text_box || !path.sources.is_empty() {
                return None;
            }
            let boundary = match &path.boundary {
                Boundary::Polyline { vertices, closed } if *closed && vertices.len() >= 2 => {
                    api::HatchBoundary::Polyline(api::PolylineGeometry {
                        points: vertices
                            .iter()
                            .map(|v| Point::xy(v.point).to_vec())
                            .collect(),
                        bulges: vertices
                            .iter()
                            .any(|v| v.bulge != 0.0)
                            .then(|| vertices.iter().map(|v| v.bulge).collect()),
                        closed: true,
                    })
                }
                Boundary::Edges(edges) => {
                    let [
                        Edge::Arc {
                            center,
                            radius,
                            start: 0.0,
                            end: 360.0,
                            counterclockwise: true,
                        },
                    ] = edges.as_slice()
                    else {
                        return None;
                    };
                    api::HatchBoundary::Circle(api::CircleGeometry {
                        center: Point::xy(*center).to_vec(),
                        radius: center.length(*radius)?,
                    })
                }
                _ => return None,
            };
            Some(api::HatchLoop {
                boundary,
                external: path.external,
                outermost: path.outermost,
            })
        })
        .collect::<Option<Vec<_>>>()?;
    let (fill, pattern) = match &hatch.fill {
        Fill::Solid => (api::HatchFill::Solid, None),
        // 既存の読取側は用紙上の模様を座標変換しないので、API の mm としては返さない。
        Fill::Pattern(pattern) if !P::PAPER => (
            api::HatchFill::Pattern,
            Some(api::HatchPattern {
                name: pattern.name.clone(),
                origin: match pattern.origin {
                    PatternOrigin::UserDefined => api::HatchPatternOrigin::UserDefined,
                    PatternOrigin::Predefined => api::HatchPatternOrigin::Predefined,
                    PatternOrigin::Custom => api::HatchPatternOrigin::Custom,
                },
                angle: pattern.angle,
                scale: pattern.scale,
                double: pattern.double,
                lines: pattern
                    .lines
                    .iter()
                    .map(|line| api::HatchPatternLine {
                        angle: line.angle,
                        base: line.base.to_vec(),
                        offset: line.offset.to_vec(),
                        dashes: line.dashes.clone(),
                    })
                    .collect(),
            }),
        ),
        _ => return None,
    };
    Some(api::HatchGeometry {
        loops,
        fill,
        pattern,
        islands: match hatch.islands {
            Islands::Nested => api::HatchIslands::Nested,
            Islands::Outer => api::HatchIslands::Outer,
            Islands::Ignore => api::HatchIslands::Ignore,
        },
        seeds: (!hatch.seeds.is_empty()).then(|| {
            hatch
                .seeds
                .iter()
                .map(|point| Point::xy(*point).to_vec())
                .collect()
        }),
    })
}

pub(super) fn shape_of<P: Spot>(given: &api::HatchGeometry, origin: P) -> Result<Hatch<P>, String> {
    let invalid = |field: &str| format!("geometry/{field}");
    let pair = |values: &[f64], field: &str| match values {
        [x, y] if x.is_finite() && y.is_finite() => Ok([*x, *y]),
        _ => Err(invalid(field)),
    };
    let finite = |value: f64, field: &str| {
        if value.is_finite() {
            Ok(value)
        } else {
            Err(invalid(field))
        }
    };
    if given.loops.is_empty() || given.loops.len() > i32::MAX as usize {
        return Err(invalid("loops"));
    }
    let mut loops = Vec::with_capacity(given.loops.len());
    for (index, path) in given.loops.iter().enumerate() {
        let at = format!("loops/{index}/boundary");
        let boundary = match &path.boundary {
            api::HatchBoundary::Polyline(line) => {
                if !line.closed {
                    return Err(invalid(&format!("{at}/closed")));
                }
                if line.points.len() < 2 || line.points.len() > i32::MAX as usize {
                    return Err(invalid(&format!("{at}/points")));
                }
                if line
                    .bulges
                    .as_ref()
                    .is_some_and(|bulges| bulges.len() != line.points.len())
                {
                    return Err(invalid(&format!("{at}/bulges")));
                }
                let vertices = line
                    .points
                    .iter()
                    .enumerate()
                    .map(|(index, point)| {
                        Ok(Vertex {
                            point: origin.at(pair(point, &format!("{at}/points/{index}"))?),
                            bulge: finite(
                                line.bulges
                                    .as_ref()
                                    .and_then(|bulges| bulges.get(index))
                                    .copied()
                                    .unwrap_or(0.0),
                                &format!("{at}/bulges/{index}"),
                            )?,
                        })
                    })
                    .collect::<Result<_, String>>()?;
                Boundary::Polyline {
                    vertices,
                    closed: true,
                }
            }
            api::HatchBoundary::Circle(circle) => {
                if !circle.radius.is_finite() || circle.radius <= 0.0 {
                    return Err(invalid(&format!("{at}/radius")));
                }
                Boundary::Edges(vec![Edge::Arc {
                    center: origin.at(pair(&circle.center, &format!("{at}/center"))?),
                    radius: origin.length_of(circle.radius),
                    start: 0.0,
                    end: 360.0,
                    counterclockwise: true,
                }])
            }
        };
        loops.push(HatchLoop {
            boundary,
            external: path.external,
            outermost: path.outermost,
            derived: false,
            text_box: false,
            sources: Vec::new(),
        });
    }
    let fill = match (given.fill, &given.pattern) {
        (api::HatchFill::Solid, None) => Fill::Solid,
        (api::HatchFill::Pattern, Some(pattern)) => {
            if pattern.name.is_empty() || pattern.name.chars().any(char::is_control) {
                return Err(invalid("pattern/name"));
            }
            if !pattern.scale.is_finite() || pattern.scale <= 0.0 {
                return Err(invalid("pattern/scale"));
            }
            if pattern.lines.is_empty() || pattern.lines.len() > i16::MAX as usize {
                return Err(invalid("pattern/lines"));
            }
            let mut lines = Vec::with_capacity(pattern.lines.len());
            for (index, line) in pattern.lines.iter().enumerate() {
                let at = format!("pattern/lines/{index}");
                let offset = pair(&line.offset, &format!("{at}/offset"))?;
                if offset == [0.0, 0.0] {
                    return Err(invalid(&format!("{at}/offset")));
                }
                if line.dashes.len() > i16::MAX as usize {
                    return Err(invalid(&format!("{at}/dashes")));
                }
                let dashes = line
                    .dashes
                    .iter()
                    .enumerate()
                    .map(|(index, &value)| finite(value, &format!("{at}/dashes/{index}")))
                    .collect::<Result<_, _>>()?;
                lines.push(PatternLine {
                    angle: finite(line.angle, &format!("{at}/angle"))?,
                    base: pair(&line.base, &format!("{at}/base"))?,
                    offset,
                    dashes,
                });
            }
            Fill::Pattern(Pattern {
                name: pattern.name.clone(),
                origin: match pattern.origin {
                    api::HatchPatternOrigin::UserDefined => PatternOrigin::UserDefined,
                    api::HatchPatternOrigin::Predefined => PatternOrigin::Predefined,
                    api::HatchPatternOrigin::Custom => PatternOrigin::Custom,
                },
                angle: finite(pattern.angle, "pattern/angle")?,
                scale: pattern.scale,
                double: pattern.double,
                lines,
            })
        }
        _ => return Err(invalid("pattern")),
    };
    let seeds = given
        .seeds
        .iter()
        .flatten()
        .enumerate()
        .map(|(index, point)| pair(point, &format!("seeds/{index}")).map(|xy| origin.at(xy)))
        .collect::<Result<_, _>>()?;
    Ok(Hatch {
        loops,
        fill,
        islands: match given.islands {
            api::HatchIslands::Nested => Islands::Nested,
            api::HatchIslands::Outer => Islands::Outer,
            api::HatchIslands::Ignore => Islands::Ignore,
        },
        associative: false,
        seeds,
    })
}
