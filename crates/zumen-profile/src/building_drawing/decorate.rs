//! 切断線の閉じた輪へ模様を置き、直線の通り芯の両端へ記号を置く。
use super::*;
use std::collections::BTreeMap;
use zumen_geom2d::topology::{self as t, Point, Segment};

// 同じ形の項目も、配置が異なれば別の閉じた輪。source は評価した part ごとに分かれる。
type Groups = BTreeMap<ElementRef, BTreeMap<usize, Vec<Segment>>>;

fn charge(cx: &mut Cx<'_>, value: t::Charge) -> Result<(), zumen_base::Stop> {
    match value {
        t::Charge::Work(n) => cx.check(Kind::Work, n),
        t::Charge::Bytes(n) => cx.check(Kind::DecodedBytes, n),
        t::Charge::Points(n) => cx.check(Kind::Segments, n),
        _ => cx.check(Kind::Work, 0),
    }
}
fn topology(error: t::Failure<zumen_base::Stop>) -> Error {
    match error {
        t::Failure::Stopped(stop) => Error::Stopped(stop),
        t::Failure::Geometry(_) => Error::Input("hatch_boundary"),
    }
}

fn groups(output: &b::Output, role: b::Role, cx: &mut Cx<'_>) -> Result<Groups, Error> {
    let mut groups = BTreeMap::<_, BTreeMap<_, Vec<_>>>::new();
    let model = output
        .drawing
        .spaces
        .first()
        .ok_or(Error::Input("drawing_space"))?;
    for line in &output.lines {
        cx.check(Kind::Work, 64)?;
        if line.role != role {
            continue;
        }
        let source = output
            .sources
            .get(line.source)
            .ok_or(Error::Input("drawing_source"))?;
        let Some(d::Element {
            shape: d::Shape::Line { start, end },
            ..
        }) = model.elements.get(line.index)
        else {
            return Err(Error::Input("drawing_shape"));
        };
        let a = output
            .grid
            .snap([start.x, start.y])
            .map_err(|_| Error::Input("drawing_grid"))?
            .point;
        let b = output
            .grid
            .snap([end.x, end.y])
            .map_err(|_| Error::Input("drawing_grid"))?
            .point;
        if !groups.contains_key(&source.element) {
            cx.check(Kind::DecodedBytes, 128)?;
        }
        let element = groups.entry(source.element).or_default();
        if !element.contains_key(&line.source) {
            cx.check(Kind::DecodedBytes, 128)?;
        }
        let segments = element.entry(line.source).or_default();
        reserve(segments, 1, 32, cx)?;
        segments.push(if a < b {
            Segment { start: a, end: b }
        } else {
            Segment { start: b, end: a }
        });
    }
    Ok(groups)
}

// 輪が重なる場合の面積や穴を推測しない。各点の次数が 2 の輪だけをつなぐ。
fn contours(mut lines: Vec<Segment>, cx: &mut Cx<'_>) -> Result<Vec<Vec<Point>>, Error> {
    let n = lines.len();
    if n > t::MAX_REGION_EDGES {
        return Err(Error::Input("hatch_boundary_limit"));
    }
    cx.check(Kind::Work, 8 * (n as u64 + 1) * (n as u64 + 1))?;
    lines.sort_unstable_by_key(|s| (s.start, s.end));
    lines.dedup();
    for line in &lines {
        if line.start == line.end
            || [line.start, line.end].iter().any(|p| {
                lines
                    .iter()
                    .filter(|l| l.start == *p || l.end == *p)
                    .count()
                    != 2
            })
        {
            return Err(Error::Input("hatch_boundary_open"));
        }
    }
    cx.check(Kind::DecodedBytes, lines.len() as u64)?;
    let mut used = vec![false; lines.len()];
    let mut paths = Vec::new();
    for (i, first) in lines.iter().enumerate() {
        if *used.get(i).ok_or(Error::Input("hatch_boundary"))? {
            continue;
        }
        *used.get_mut(i).ok_or(Error::Input("hatch_boundary"))? = true;
        let mut path = Vec::new();
        reserve(&mut path, 2, 16, cx)?;
        path.push(first.start);
        let mut end = first.end;
        while end != first.start {
            reserve(&mut path, 1, 16, cx)?;
            path.push(end);
            let Some((j, next)) = lines
                .iter()
                .enumerate()
                .find(|(j, s)| used.get(*j) == Some(&false) && (s.start == end || s.end == end))
            else {
                return Err(Error::Input("hatch_boundary_open"));
            };
            *used.get_mut(j).ok_or(Error::Input("hatch_boundary"))? = true;
            end = if next.start == end {
                next.end
            } else {
                next.start
            };
        }
        reserve(&mut paths, 1, 24, cx)?;
        paths.push(path);
    }
    Ok(paths)
}

fn regions(
    paths: &[Vec<Point>],
    fill: t::RegionFill,
    cx: &mut Cx<'_>,
) -> Result<Vec<t::Region>, Error> {
    let mut refs = Vec::new();
    reserve(&mut refs, paths.len(), 16, cx)?;
    for path in paths {
        refs.push(path.as_slice());
    }
    let result = t::boolean_regions(&refs, &[], t::RegionOperation::Union, fill, |v| {
        charge(cx, v)
    })
    .map_err(topology)?;
    if !result.adjustments.is_empty() || !result.discarded.is_empty() {
        return Err(Error::Input("hatch_boundary_changed"));
    }
    Ok(result.regions)
}

fn material<'a>(
    profile: &'a DrawingProfile,
    building: &Building,
    at: ElementRef,
    cx: &mut Cx<'_>,
) -> Result<Option<&'a settings::Hatch>, Error> {
    let material = building.components.material.get(at).or_else(|| {
        building
            .components
            .r#type
            .get(at)
            .and_then(|t| building.components.material.get(t.target))
    });
    let Some(material) = material else {
        return Ok(None);
    };
    let mut selected = None;
    for part in &material.parts {
        profile.profile.plan_work(cx)?;
        let name = part
            .material
            .and_then(|r| building.components.identity.get(r))
            .and_then(|v| v.name.as_deref())
            .or(part.name.as_deref());
        let Some(hatch) = profile
            .settings
            .hatches
            .iter()
            .position(|h| name.is_some_and(|n| h.materials.iter().any(|m| m == n)))
        else {
            return Ok(None);
        };
        if selected.is_some_and(|prior| prior != hatch) {
            return Ok(None);
        }
        selected = Some(hatch);
    }
    Ok(selected.and_then(|i| profile.settings.hatches.get(i)))
}

pub(super) fn hatches(
    profile: &DrawingProfile,
    building: &Building,
    output: &mut b::Output,
    decorations: &mut Vec<Decoration>,
    cx: &mut Cx<'_>,
) -> Result<(), Error> {
    let scale = paper_scale(output)?;
    for (at, parts) in groups(output, b::Role::Cut, cx)? {
        let Some(hatch) = material(profile, building, at, cx)? else {
            report(building, at, "hatch", "material_unmapped", cx)?;
            continue;
        };
        let evaluated = (|| {
            let mut paths = Vec::new();
            for (_, lines) in parts {
                for region in regions(&contours(lines, cx)?, t::RegionFill::EvenOdd, cx)? {
                    reserve(&mut paths, 1 + region.holes.len(), 24, cx)?;
                    paths.push(region.outer);
                    paths.extend(region.holes);
                }
            }
            regions(&paths, t::RegionFill::NonZero, cx)
        })();
        let regions = match evaluated {
            Ok(value) => value,
            Err(Error::Stopped(stop)) => return Err(Error::Stopped(stop)),
            Err(_) => {
                report(building, at, "hatch", "boundary", cx)?;
                output.complete = false;
                continue;
            }
        };
        for region in regions {
            let mut loops = Vec::new();
            reserve(&mut loops, 1 + region.holes.len(), 80, cx)?;
            for (i, path) in std::iter::once(region.outer)
                .chain(region.holes)
                .enumerate()
            {
                let mut vertices = Vec::new();
                reserve(&mut vertices, path.len(), 32, cx)?;
                for p in path {
                    cx.check(Kind::Work, 1)?;
                    let [x, y] = output
                        .grid
                        .position(p)
                        .map_err(|_| Error::Input("drawing_grid"))?;
                    vertices.push(d::Vertex {
                        point: point([x, y]),
                        bulge: 0.0,
                    });
                }
                loops.push(d::HatchLoop {
                    boundary: d::Boundary::Polyline {
                        vertices,
                        closed: true,
                    },
                    external: i == 0,
                    outermost: i == 0,
                    derived: true,
                    text_box: false,
                    sources: vec![],
                });
            }
            let mut lines = Vec::new();
            reserve(&mut lines, hatch.angles.len(), 64, cx)?;
            let pitch = hatch.pitch / scale;
            if !pitch.is_finite() {
                return Err(Error::Input("hatch_size"));
            }
            for angle in &hatch.angles {
                cx.check(Kind::Work, 1)?;
                let radians = *angle * (core::f64::consts::PI / 180.0);
                lines.push(d::PatternLine {
                    angle: *angle,
                    base: [0.0; 2],
                    offset: [-libm::sin(radians) * pitch, libm::cos(radians) * pitch],
                    dashes: vec![],
                });
            }
            cx.check(Kind::DecodedBytes, hatch.name.len() as u64)?;
            let shape = d::Shape::Hatch(d::Hatch {
                loops,
                islands: d::Islands::Nested,
                fill: d::Fill::Pattern(d::Pattern {
                    name: hatch.name.clone(),
                    origin: d::PatternOrigin::UserDefined,
                    angle: 0.0,
                    scale: 1.0,
                    double: false,
                    lines,
                }),
                associative: false,
                seeds: vec![],
            });
            append(
                profile,
                building,
                output,
                decorations,
                Added {
                    at,
                    kind: "hatch",
                    shape,
                },
                cx,
            )?;
        }
    }
    Ok(())
}

pub(super) fn grids(
    profile: &DrawingProfile,
    building: &Building,
    output: &mut b::Output,
    decorations: &mut Vec<Decoration>,
    cx: &mut Cx<'_>,
) -> Result<(), Error> {
    let scale = paper_scale(output)?;
    let [radius, text_height, offset] = profile.settings.grid;
    for (at, parts) in groups(output, b::Role::Grid, cx)? {
        let tag = match building.components.grid.get(at) {
            Some(zumen_building::Grid::Axis { tag, .. }) => tag.as_deref(),
            _ => None,
        };
        let Some(tag) = tag.filter(|s| !s.is_empty()) else {
            report(building, at, "grid", "tag", cx)?;
            continue;
        };
        let mut ends = None;
        let mut straight = true;
        let mut all = Vec::new();
        for (_, lines) in parts {
            reserve(&mut all, lines.len(), 32, cx)?;
            all.extend(lines);
        }
        // 最遠端は辞書順の端。全点が同じ直線にあり、線分の間に隙間がない場合だけ使う。
        cx.check(Kind::Work, (all.len() as u64 + 1) * 64)?;
        all.sort_unstable_by_key(|s| (s.start, s.end));
        for line in &all {
            if line.start == line.end {
                straight = false;
                break;
            }
            if let Some((a, b)) = ends {
                if line.start > b || !collinear(a, b, line.start) || !collinear(a, b, line.end) {
                    straight = false;
                    break;
                }
                ends = Some((a, b.max(line.end)));
            } else {
                ends = Some((line.start, line.end));
            }
        }
        let Some((a, b)) = ends.filter(|_| straight) else {
            report(building, at, "grid", "not_straight", cx)?;
            continue;
        };
        let a = output
            .grid
            .position(a)
            .map_err(|_| Error::Input("drawing_grid"))?;
        let b = output
            .grid
            .position(b)
            .map_err(|_| Error::Input("drawing_grid"))?;
        let delta = [b[0] - a[0], b[1] - a[1]];
        let length = libm::hypot(delta[0], delta[1]);
        if !length.is_finite() || length <= 0.0 {
            report(building, at, "grid", "length", cx)?;
            continue;
        }
        for (end, sign) in [(a, -1.0), (b, 1.0)] {
            let center = [
                end[0] + sign * delta[0] / length * offset / scale,
                end[1] + sign * delta[1] / length * offset / scale,
            ];
            if center.iter().any(|v| !v.is_finite()) {
                return Err(Error::Input("grid_symbol_size"));
            }
            append(
                profile,
                building,
                output,
                decorations,
                Added {
                    at,
                    kind: "grid-circle",
                    shape: d::Shape::Circle {
                        center: point(center),
                        radius: d::Length::Paper(d::PaperLength(radius)),
                    },
                },
                cx,
            )?;
            cx.check(Kind::DecodedBytes, tag.len() as u64)?;
            cx.check(Kind::Work, tag.len() as u64)?;
            let shape = d::Shape::Text(d::Text {
                content: tag.into(),
                height: d::Height {
                    value: d::Length::Paper(d::PaperLength(text_height)),
                    meaning: d::HeightMeaning::Frame,
                },
                placement: d::Placement::Anchor {
                    point: point(center),
                    horizontal: d::Horizontal::Center,
                    vertical: d::Vertical::Middle,
                },
                rotation: 0.0,
                width_factor: 1.0,
                oblique: 0.0,
                backward: false,
                upside_down: false,
                style: String::new(),
            });
            append(
                profile,
                building,
                output,
                decorations,
                Added {
                    at,
                    kind: "grid-label",
                    shape,
                },
                cx,
            )?;
        }
    }
    Ok(())
}

fn paper_scale(output: &b::Output) -> Result<f64, Error> {
    let view = output
        .drawing
        .sheets
        .first()
        .and_then(|s| s.views.first())
        .ok_or(Error::Input("drawing_paper"))?;
    let scale = view.transform.m[0];
    if !scale.is_finite()
        || scale <= 0.0
        || view.transform.m[4] != scale
        || view.transform.m[1] != 0.0
        || view.transform.m[3] != 0.0
    {
        return Err(Error::Input("drawing_paper"));
    }
    Ok(scale)
}
fn point([x, y]: [f64; 2]) -> d::ModelPoint {
    d::ModelPoint {
        space: d::SpaceId(0),
        x,
        y,
    }
}
fn collinear(a: Point, b: Point, p: Point) -> bool {
    let [ax, ay] = a.coordinates().map(i128::from);
    let [bx, by] = b.coordinates().map(i128::from);
    let [px, py] = p.coordinates().map(i128::from);
    (bx - ax) * (py - ay) == (by - ay) * (px - ax)
}
struct Added {
    at: ElementRef,
    kind: &'static str,
    shape: d::Shape<d::ModelPoint>,
}
fn append(
    profile: &DrawingProfile,
    building: &Building,
    output: &mut b::Output,
    decorations: &mut Vec<Decoration>,
    added: Added,
    cx: &mut Cx<'_>,
) -> Result<(), Error> {
    let Added { at, kind, shape } = added;
    cx.check(Kind::Work, profile.settings.style_work)?;
    cx.check(Kind::Work, decorations.len() as u64)?;
    let ordinal = decorations
        .iter()
        .filter(|d| d.element == at && d.kind == kind)
        .count();
    let (style_index, style) = profile
        .settings
        .select(
            building,
            at,
            if kind == "hatch" {
                "hatch"
            } else {
                "annotation"
            },
        )
        .ok_or(Error::Input("drawing_style_ambiguous"))?;
    let element_id = building.id(at).ok_or(Error::Input("drawing_source"))?;
    cx.check(Kind::DecodedBytes, 2048)?;
    let id = format!("building/{element_id}/profile/{kind}/{ordinal}");
    let drawing = &mut output.drawing;
    let model = drawing
        .spaces
        .first_mut()
        .ok_or(Error::Input("drawing_space"))?;
    let index = model.elements.len();
    let drawing_id = format!("space/0/e{index}");
    let element = d::Element {
        source: Ref::default().with_id(id.clone()),
        layer: d::LayerId(
            (drawing.layers.len() - profile.settings.styles.len() + style_index) as u32,
        ),
        style: settings::element_style(
            style,
            drawing.line_types.len() - profile.settings.lines.len(),
        ),
        shape,
    };
    let basis = d::Basis::Hash(d::Hash::element_checked(&element, cx)?);
    reserve(&mut model.elements, 1, 512, cx)?;
    reserve(&mut model.order, 1, 8, cx)?;
    model.elements.push(element);
    model.order.push(index);
    reserve(&mut drawing.extensions, 1, 256, cx)?;
    let fields = [
        ("id", id.clone()),
        ("element", element_id.to_string()),
        ("kind", kind.into()),
    ]
    .map(|(key, value)| d::Field {
        key: key.into(),
        code: None,
        value: d::FieldValue::Text(value),
    })
    .into();
    drawing.extensions.push(d::Extension {
        namespace: "zumen.profile.building-drawing".into(),
        name: "decoration".into(),
        version: None,
        schema: 1,
        target: d::Target::Element(drawing_id.clone()),
        basis,
        fields,
        source: Ref::default().with_id(element_id.to_string()),
    });
    reserve(decorations, 1, 96, cx)?;
    decorations.push(Decoration {
        id,
        drawing_id,
        index,
        element: at,
        kind,
    });
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use zumen_base::{Budget, CancelToken, Diagnostics};
    fn with<T>(run: impl FnOnce(&mut Cx<'_>) -> T) -> T {
        run(&mut Cx::new(
            &mut Budget::default(),
            &mut Diagnostics::new(),
            &CancelToken::new(),
        ))
    }
    #[test]
    fn open_and_branched_boundaries_are_not_closed_by_guessing() {
        let grid = t::Grid::new([0.0; 2], 0.01).expect("固定した格子");
        let points = [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0], [2.0, 0.0]]
            .map(|p| grid.snap(p).expect("固定した点").point);
        let line = |a, b| Segment {
            start: points[a],
            end: points[b],
        };
        let closed = vec![line(0, 1), line(1, 2), line(2, 3), line(3, 0)];
        assert!(with(|cx| contours(closed.clone(), cx)).is_ok());
        assert!(with(|cx| contours(vec![line(0, 1), line(1, 2), line(2, 3)], cx)).is_err());
        let mut branch = closed;
        branch.push(line(1, 4));
        assert!(with(|cx| contours(branch, cx)).is_err());
    }
    #[test]
    fn fixed_allocation_slots_cover_platform_layouts() {
        assert!(size_of::<d::Element<d::ModelPoint>>() <= 512);
        assert!(size_of::<d::HatchLoop<d::ModelPoint>>() <= 80);
        assert!(size_of::<d::PatternLine>() <= 64);
        assert!(size_of::<b::AttributeSymbol<'_>>() <= 128);
        assert!(size_of::<Decoration>() <= 96);
        assert!(size_of::<d::Extension>() <= 256);
        assert!(size_of::<d::Field>() <= 80);
    }
}
