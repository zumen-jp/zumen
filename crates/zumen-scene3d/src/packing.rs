//! 局所のメッシュを共有し、要素ごとに塊へ詰める。

use crate::{
    Chunk, Color, Data, Element, Error, Instance, Mesh, Options, Scene,
    building::{Bounds, mesh_bounds, overlaps, point, reserve, union},
    selection::{Selection, is_void},
};
use std::collections::BTreeMap;
use zumen_base::{Cx, Diagnostic, Kind, codes};
use zumen_building::{
    Building, ElementRef,
    evaluation::{ElementMesh, MeshPart},
};
use zumen_geom3d::Transform;

struct Pending<'a> {
    source: &'a MeshPart,
    center: [f64; 3],
    mesh: u32,
    absolute: Transform,
}

fn center([lo, hi]: Bounds) -> [f64; 3] {
    [
        lo[0] * 0.5 + hi[0] * 0.5,
        lo[1] * 0.5 + hi[1] * 0.5,
        lo[2] * 0.5 + hi[2] * 0.5,
    ]
}

fn rounded(value: f64) -> Result<f32, Error> {
    let value = value as f32;
    if !value.is_finite() {
        return Err(Error::Invalid("f32_range"));
    }
    Ok(if value == 0.0 { 0.0 } else { value })
}

fn mesh_key(mesh: &Mesh) -> [u8; 32] {
    let mut h = blake3::Hasher::new();
    for n in [mesh.vertices.len(), mesh.triangles.len(), mesh.edges.len()] {
        h.update(&(n as u64).to_le_bytes());
    }
    for v in mesh.vertices.iter().flatten() {
        h.update(&v.to_le_bytes());
    }
    for v in mesh
        .triangles
        .iter()
        .flatten()
        .chain(mesh.edges.iter().flatten())
    {
        h.update(&v.to_le_bytes());
    }
    *h.finalize().as_bytes()
}

fn mesh_size(mesh: &Mesh) -> u64 {
    mesh.vertices.len() as u64 * 12 + mesh.triangles.len() as u64 * 12 + mesh.edges.len() as u64 * 8
}

fn prepare<'a>(
    source: &'a MeshPart,
    scale: f64,
    data: &mut Data,
    sharing: &mut BTreeMap<[u8; 32], Vec<u32>>,
    cx: &mut Cx,
) -> Result<Option<Pending<'a>>, Error> {
    cx.check(Kind::Work, source.mesh.positions.len() as u64)?;
    let Some(bounds) = source.mesh.bounds() else {
        return Ok(None);
    };
    let center = center(bounds);
    let mut absolute = source.placement;
    let origin = absolute.point(center).map(|v| v / scale);
    for (row, value) in absolute.0.iter_mut().zip(origin) {
        row[3] = value;
    }
    if !absolute.is_finite() {
        return Err(Error::Invalid("coordinate"));
    }
    let mut mesh = Mesh::default();
    reserve(&mut mesh.vertices, source.mesh.positions.len(), 12, cx)?;
    reserve(&mut mesh.triangles, source.mesh.triangles.len(), 12, cx)?;
    reserve(&mut mesh.edges, source.mesh.edges.len(), 8, cx)?;
    cx.check(Kind::Segments, source.mesh.positions.len() as u64)?;
    for &p in &source.mesh.positions {
        mesh.vertices.push([
            rounded((p[0] - center[0]) / scale)?,
            rounded((p[1] - center[1]) / scale)?,
            rounded((p[2] - center[2]) / scale)?,
        ]);
    }
    mesh.triangles.extend_from_slice(&source.mesh.triangles);
    mesh.edges.extend_from_slice(&source.mesh.edges);
    cx.check(
        Kind::Work,
        mesh_size(&mesh) + 32 * (sharing.len() as u64 + 1).ilog2() as u64 + 1,
    )?;
    let key = mesh_key(&mesh);
    if let Some(candidates) = sharing.get(&key) {
        for &index in candidates {
            cx.check(Kind::Work, mesh_size(&mesh))?;
            if data.meshes.get(index as usize) == Some(&mesh) {
                return Ok(Some(Pending {
                    source,
                    center,
                    mesh: index,
                    absolute,
                }));
            }
        }
    }
    let index = u32::try_from(data.meshes.len()).map_err(|_| Error::Invalid("size"))?;
    reserve(&mut data.meshes, 1, 96, cx)?;
    data.meshes.push(mesh);
    if !sharing.contains_key(&key) {
        cx.check(Kind::DecodedBytes, 256)?;
    }
    let candidates = sharing.entry(key).or_default();
    reserve(candidates, 1, 4, cx)?;
    candidates.push(index);
    Ok(Some(Pending {
        source,
        center,
        mesh: index,
        absolute,
    }))
}

fn color(building: &Building, at: ElementRef, cx: &mut Cx) -> Color {
    let style = building.components.style.get(at);
    let rgb = style
        .and_then(|s| s.surface_color)
        .filter(|rgb| rgb.iter().all(|v| v.is_finite() && (0.0..=1.0).contains(v)));
    let alpha = style.and_then(|s| s.transparency).unwrap_or(0.0);
    let alpha_valid = alpha.is_finite() && (0.0..=1.0).contains(&alpha);
    let unresolved = rgb.is_none() || !alpha_valid;
    if unresolved {
        cx.report(Diagnostic::new(codes::SCENE3D_COLOR_MISSING).at(building.place(at)));
    }
    let [r, g, b] = if unresolved {
        [0.0; 3]
    } else {
        rgb.unwrap_or_default()
    };
    Color {
        rgba: [
            r as f32,
            g as f32,
            b as f32,
            if alpha_valid {
                (1.0 - alpha) as f32
            } else {
                1.0
            },
        ],
        unresolved,
    }
}

fn color_index(
    value: Color,
    data: &mut Data,
    colors: &mut BTreeMap<([u32; 4], bool), u32>,
    cx: &mut Cx,
) -> Result<u32, Error> {
    cx.check(Kind::Work, (colors.len() as u64 + 1).ilog2() as u64 + 1)?;
    let key = (value.rgba.map(f32::to_bits), value.unresolved);
    if let Some(&index) = colors.get(&key) {
        return Ok(index);
    }
    let index = u32::try_from(data.colors.len()).map_err(|_| Error::Invalid("size"))?;
    reserve(&mut data.colors, 1, 32, cx)?;
    cx.check(Kind::DecodedBytes, 192)?;
    data.colors.push(value);
    colors.insert(key, index);
    Ok(index)
}

fn relative(absolute: Transform, origin: [f64; 3]) -> Result<[f32; 12], Error> {
    let mut out = [0.0; 12];
    for ((row, output), origin) in absolute
        .0
        .iter()
        .zip(out.as_chunks_mut::<4>().0)
        .zip(origin)
    {
        for (dst, &src) in output.iter_mut().zip(row).take(3) {
            *dst = rounded(src)?;
        }
        output[3] = rounded(row[3] - origin)?;
    }
    let [a, b, c, _, d, e, f, _, g, h, i, _] = out.map(f64::from);
    if a * (e * i - f * h) - b * (d * i - f * g) + c * (d * h - e * g) == 0.0 {
        return Err(Error::Invalid("singular_f32"));
    }
    Ok(out)
}

fn needs_origin(pending: &[Pending<'_>], origin: [f64; 3], tolerance: f64) -> bool {
    pending.iter().any(|p| {
        p.absolute.column(3).into_iter().zip(origin).any(|(v, o)| {
            let difference = v - o;
            (difference - f64::from(difference as f32)).abs() > tolerance * 0.25
                || !(difference as f32).is_finite()
        })
    })
}

fn shape_error(
    error: Error,
    at: ElementRef,
    building: &Building,
    cx: &mut Cx,
) -> Result<(), Error> {
    match error {
        Error::Stopped(_) => Err(error),
        Error::Invalid(reason) => {
            cx.report(
                Diagnostic::new(codes::SCENE3D_SHAPE_DROPPED)
                    .with("reason", reason)
                    .at(building.place(at)),
            );
            cx.check(Kind::Work, 0)?;
            Ok(())
        }
    }
}

fn placed_bounds(
    pending: &Pending<'_>,
    mesh: &Mesh,
    transform: [f32; 12],
    origin: [f64; 3],
    scale: f64,
    cx: &mut Cx,
) -> Result<(Bounds, f64), Error> {
    let m = transform.map(f64::from);
    let transform = Transform([
        [m[0], m[1], m[2], m[3]],
        [m[4], m[5], m[6], m[7]],
        [m[8], m[9], m[10], m[11]],
    ]);
    let mut bounds = None;
    let mut error = 0.0_f64;
    cx.check(Kind::Work, mesh.vertices.len() as u64 * 2)?;
    for (&local, &source) in mesh.vertices.iter().zip(&pending.source.mesh.positions) {
        let actual = transform.point(local.map(f64::from));
        point(&mut bounds, actual)?;
        let local = [
            (source[0] - pending.center[0]) / scale,
            (source[1] - pending.center[1]) / scale,
            (source[2] - pending.center[2]) / scale,
        ];
        let delta = pending.absolute.vector(local);
        let translation = pending.absolute.column(3);
        let difference = [
            actual[0] - (delta[0] + (translation[0] - origin[0])),
            actual[1] - (delta[1] + (translation[1] - origin[1])),
            actual[2] - (delta[2] + (translation[2] - origin[2])),
        ];
        error = error.max(libm::hypot(
            libm::hypot(difference[0], difference[1]),
            difference[2],
        ));
    }
    let bounds = bounds.ok_or(Error::Invalid("empty_mesh"))?;
    // 塊の範囲は f32 で囲む。範囲を作れない形はインスタンスを登録する前に除く。
    outward(bounds)?;
    Ok((bounds, error))
}

fn outward([lo, hi]: Bounds) -> Result<[f32; 6], Error> {
    let mut out = [0.0; 6];
    for (i, value) in lo.into_iter().chain(hi).enumerate() {
        let rounded = rounded(value)?;
        let outward = if i < 3 {
            libm::nextafterf(rounded, f32::NEG_INFINITY)
        } else {
            libm::nextafterf(rounded, f32::INFINITY)
        };
        if !outward.is_finite() {
            return Err(Error::Invalid("f32_bounds"));
        }
        let slot = out.get_mut(i).ok_or(Error::Invalid("bounds"))?;
        *slot = outward;
    }
    Ok(out)
}

fn finish_chunk(
    data: &mut Data,
    bounds: Option<Bounds>,
    extent: &mut Option<Bounds>,
) -> Result<(), Error> {
    if let Some(bounds) = bounds {
        let chunk = data.chunks.last_mut().ok_or(Error::Invalid("chunk"))?;
        chunk.bounds = outward(bounds)?;
        let b = chunk.bounds.map(f64::from);
        let o = chunk.origin;
        union(
            extent,
            [
                [b[0] + o[0], b[1] + o[1], b[2] + o[2]],
                [b[3] + o[0], b[4] + o[1], b[5] + o[2]],
            ],
        );
    }
    Ok(())
}

pub(crate) fn pack(
    building: &Building,
    meshes: &[ElementMesh],
    selected: Selection,
    options: Options<'_>,
    tolerance: f64,
    scale: f64,
    cx: &mut Cx,
) -> Result<Scene, Error> {
    let n = building.elements().len();
    cx.check(Kind::DecodedBytes, n as u64 * 6)?;
    let mut keep = vec![false; n];
    let mut draw = vec![false; n];
    let mut rows = vec![u32::MAX; n];
    for &at in &selected.elements {
        cx.check(Kind::Work, 1)?;
        let element = meshes.get(at.0 as usize).ok_or(Error::Invalid("element"))?;
        let (bounds, valid) = match mesh_bounds(element, scale, cx) {
            Ok(bounds) => (bounds, true),
            Err(error) => {
                shape_error(error, at, building, cx)?;
                (None, false)
            }
        };
        if bounds.is_some_and(|b| options.region.is_some_and(|r| !overlaps(b, r))) {
            continue;
        }
        *keep
            .get_mut(at.0 as usize)
            .ok_or(Error::Invalid("element"))? = true;
        *draw
            .get_mut(at.0 as usize)
            .ok_or(Error::Invalid("element"))? = valid && !is_void(building, at);
        if let Some(floor) = selected.storeys.get(at.0 as usize).copied().flatten() {
            *keep
                .get_mut(floor.0 as usize)
                .ok_or(Error::Invalid("storey"))? = true;
        }
    }
    let mut data = Data::default();
    for (index, (&included, row)) in keep.iter().zip(&mut rows).enumerate() {
        if !included {
            continue;
        }
        cx.check(Kind::Work, 1)?;
        *row = u32::try_from(data.elements.len()).map_err(|_| Error::Invalid("size"))?;
        let at = ElementRef(index as u32);
        let class = building
            .components
            .class
            .get(at)
            .map(|c| c.name.as_str())
            .unwrap_or("");
        let name = building
            .components
            .identity
            .get(at)
            .and_then(|i| i.name.as_deref())
            .unwrap_or("");
        cx.check(
            Kind::DecodedBytes,
            24 + class.len() as u64 + name.len() as u64,
        )?;
        reserve(&mut data.elements, 1, 128, cx)?;
        cx.check(Kind::Elements, 1)?;
        data.elements.push(Element {
            id: building.id(at).ok_or(Error::Invalid("id"))?.to_string(),
            class: class.to_owned(),
            name: name.to_owned(),
            storey: None,
        });
    }
    for (index, &row) in rows.iter().enumerate() {
        if row == u32::MAX {
            continue;
        }
        let floor = selected
            .storeys
            .get(index)
            .copied()
            .flatten()
            .filter(|s| s.0 as usize != index);
        if let Some(floor) = floor {
            let storey = *rows.get(floor.0 as usize).ok_or(Error::Invalid("storey"))?;
            if storey != u32::MAX {
                data.elements
                    .get_mut(row as usize)
                    .ok_or(Error::Invalid("element"))?
                    .storey = Some(storey);
            }
        }
    }
    let mut sharing = BTreeMap::new();
    let mut colors = BTreeMap::new();
    let mut chunk_bounds = None;
    let mut extent = None;
    for &at in &selected.elements {
        if !draw.get(at.0 as usize).copied().unwrap_or(false) {
            continue;
        }
        cx.check(Kind::Work, 1)?;
        let evaluated = meshes.get(at.0 as usize).ok_or(Error::Invalid("element"))?;
        let mut pending = Vec::new();
        let mut bounds = None;
        for source in &evaluated.parts {
            match prepare(source, scale, &mut data, &mut sharing, cx) {
                Ok(Some(value)) => {
                    cx.check(Kind::Work, source.mesh.positions.len() as u64)?;
                    for &p in &source.mesh.positions {
                        point(&mut bounds, source.placement.point(p).map(|v| v / scale))?;
                    }
                    reserve(&mut pending, 1, 256, cx)?;
                    pending.push(value);
                }
                Ok(None) => {}
                Err(error) => shape_error(error, at, building, cx)?,
            }
        }
        let Some(bounds) = bounds else {
            continue;
        };
        cx.check(Kind::Work, pending.len() as u64)?;
        let new_chunk = data.chunks.last().is_none_or(|c| {
            data.instances.len() as u64 - u64::from(c.instances) + pending.len() as u64
                > u64::from(options.chunk_instances)
                || needs_origin(&pending, c.origin, tolerance)
        });
        if new_chunk {
            finish_chunk(&mut data, chunk_bounds, &mut extent)?;
            chunk_bounds = None;
            reserve(&mut data.chunks, 1, 64, cx)?;
            data.chunks.push(Chunk {
                origin: center(bounds),
                bounds: [0.0; 6],
                instances: u32::try_from(data.instances.len())
                    .map_err(|_| Error::Invalid("size"))?,
            });
        }
        let origin = data.chunks.last().ok_or(Error::Invalid("chunk"))?.origin;
        let color = color(building, at, cx);
        let color = color_index(color, &mut data, &mut colors, cx)?;
        for pending in pending {
            let result = (|| {
                let transform = relative(pending.absolute, origin)?;
                let mesh = data
                    .meshes
                    .get(pending.mesh as usize)
                    .ok_or(Error::Invalid("mesh"))?;
                let (bounds, error) = placed_bounds(&pending, mesh, transform, origin, scale, cx)?;
                reserve(&mut data.instances, 1, 64, cx)?;
                data.instances.push(Instance {
                    mesh: pending.mesh,
                    element: *rows.get(at.0 as usize).ok_or(Error::Invalid("element"))?,
                    color,
                    transform,
                });
                union(&mut chunk_bounds, bounds);
                if error > 0.0 {
                    cx.report(
                        Diagnostic::new(codes::SCENE3D_COORDINATE_APPROXIMATED)
                            .with("error", error)
                            .at(building.place(at)),
                    );
                }
                Ok(())
            })();
            if let Err(error) = result {
                shape_error(error, at, building, cx)?;
            }
        }
        // 1 つも登録できなかった最後の空の塊は残さない。
        if data
            .chunks
            .last()
            .is_some_and(|c| c.instances as usize == data.instances.len())
        {
            data.chunks.pop();
            chunk_bounds = None;
        }
    }
    finish_chunk(&mut data, chunk_bounds, &mut extent)?;
    cx.check(Kind::Work, 0)?;
    Ok(Scene {
        data,
        tolerance,
        extent,
    })
}
