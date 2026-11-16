//! 型を持つ値を、列のバッファーへ詰める。
use crate::{Data, Error, MAX_BYTES, PointData, layout as l};
use std::ops::Range;
use zumen_base::{Cx, Kind};

fn sum(mut values: impl Iterator<Item = usize>) -> Result<u64, Error> {
    values.try_fold(0_u64, |total, size| {
        total
            .checked_add(size as u64)
            .filter(|&v| v <= MAX_BYTES)
            .ok_or(Error::Invalid("size"))
    })
}

fn count(size: usize) -> Result<u32, Error> {
    u32::try_from(size).map_err(|_| Error::Invalid("size"))
}

fn lengths(data: &Data, points: Option<&PointData>) -> Result<[u64; l::SECTIONS.len()], Error> {
    let vertices = sum(data.meshes.iter().map(|m| m.vertices.len()))?;
    let triangles = sum(data.meshes.iter().map(|m| m.triangles.len()))?;
    let edges = sum(data.meshes.iter().map(|m| m.edges.len()))?;
    let strings = sum(data
        .elements
        .iter()
        .flat_map(|e| [e.id.len(), e.class.len(), e.name.len()]))?;
    let mut rows = [0_u64; l::SECTIONS.len()];
    for index in [l::CHUNK_ORIGINS, l::CHUNK_BOUNDS, l::CHUNK_INSTANCES] {
        *rows.get_mut(index).ok_or(Error::Invalid("layout"))? = data.chunks.len() as u64;
    }
    for index in [l::MESH_VERTICES, l::MESH_TRIANGLES, l::MESH_EDGES] {
        *rows.get_mut(index).ok_or(Error::Invalid("layout"))? = data.meshes.len() as u64;
    }
    for index in [
        l::INSTANCE_MESHES,
        l::INSTANCE_ELEMENTS,
        l::INSTANCE_COLORS,
        l::INSTANCE_TRANSFORMS,
    ] {
        *rows.get_mut(index).ok_or(Error::Invalid("layout"))? = data.instances.len() as u64;
    }
    for index in [
        l::ELEMENT_IDS,
        l::ELEMENT_CLASSES,
        l::ELEMENT_NAMES,
        l::ELEMENT_STOREYS,
    ] {
        *rows.get_mut(index).ok_or(Error::Invalid("layout"))? = data.elements.len() as u64;
    }
    for index in [l::COLOR_RGBA, l::COLOR_FLAGS] {
        *rows.get_mut(index).ok_or(Error::Invalid("layout"))? = data.colors.len() as u64;
    }
    for (index, value) in [
        (l::VERTEX_XYZ, vertices),
        (l::TRIANGLE_VERTICES, triangles),
        (l::EDGE_VERTICES, edges),
        (l::STRINGS, strings),
    ] {
        *rows.get_mut(index).ok_or(Error::Invalid("layout"))? = value;
    }
    if let Some(points) = points {
        for (index, value) in [
            (l::CHUNK_POINTS, points.chunks.len()),
            (l::CHUNK_NODES, points.chunks.len()),
            (l::POINT_XYZ, points.positions.len()),
            (l::POINT_RGBA, points.rgba.as_ref().map_or(0, Vec::len)),
            (
                l::POINT_INTENSITY,
                points.intensity.as_ref().map_or(0, Vec::len),
            ),
            (
                l::POINT_CLASSIFICATION,
                points.classification.as_ref().map_or(0, Vec::len),
            ),
        ] {
            *rows.get_mut(index).ok_or(Error::Invalid("layout"))? = value as u64;
        }
    }
    for (rows, definition) in rows.iter_mut().zip(l::SECTIONS) {
        *rows = rows
            .checked_mul((definition.width * definition.ty.size()) as u64)
            .filter(|&bytes| bytes <= MAX_BYTES)
            .ok_or(Error::Invalid("size"))?;
    }
    Ok(rows)
}

struct Writer {
    bytes: Vec<u8>,
    ranges: [Range<usize>; l::SECTIONS.len()],
}

impl Writer {
    fn values<const N: usize>(
        &mut self,
        column: usize,
        values: impl IntoIterator<Item = [u8; N]>,
    ) -> Result<(), Error> {
        let range = self
            .ranges
            .get(column)
            .ok_or(Error::Invalid("layout"))?
            .clone();
        let data = self.bytes.get_mut(range).ok_or(Error::Invalid("layout"))?;
        let mut position = 0;
        for value in values {
            let end = position + N;
            data.get_mut(position..end)
                .ok_or(Error::Invalid("row_count"))?
                .copy_from_slice(&value);
            position = end;
        }
        if position != data.len() {
            return Err(Error::Invalid("row_count"));
        }
        Ok(())
    }
}

/// 型を持つシーンを ZMS3 にする。値と参照を検査し、失敗ではバッファーを返さない。
pub fn encode(data: &Data, cx: &mut Cx) -> Result<Vec<u8>, Error> {
    encode_inner(data, None, cx)
}

/// 元の節への参照を持つ点の列を加え、ZMS3 1.1 を書く。
pub fn encode_with_points(data: &Data, points: &PointData, cx: &mut Cx) -> Result<Vec<u8>, Error> {
    encode_inner(data, Some(points), cx)
}

fn encode_inner(data: &Data, points: Option<&PointData>, cx: &mut Cx) -> Result<Vec<u8>, Error> {
    cx.check(
        Kind::Work,
        (data.meshes.len() as u64 + data.elements.len() as u64) * 4 + 1,
    )?;
    let lengths = lengths(data, points)?;
    let sections = if points.is_some() {
        l::SECTIONS.len()
    } else {
        l::STRINGS + 1
    };
    let minor = if points.is_some() { l::MINOR } else { 0 };
    let mut end = 16 + sections as u64 * 8;
    let mut ranges = std::array::from_fn(|_| 0..0);
    for (length, range) in lengths.into_iter().zip(&mut ranges).take(sections) {
        let start = (end + 7) & !7;
        end = start
            .checked_add(length)
            .filter(|&n| n <= MAX_BYTES)
            .ok_or(Error::Invalid("size"))?;
        *range = start as usize..end as usize;
    }
    cx.check(Kind::Work, end)?;
    cx.check(Kind::DecodedBytes, end)?;
    let mut writer = Writer {
        bytes: vec![0; end as usize],
        ranges,
    };
    let header = writer.bytes.get_mut(..16).ok_or(Error::Invalid("size"))?;
    header
        .get_mut(..4)
        .ok_or(Error::Invalid("size"))?
        .copy_from_slice(&l::MAGIC);
    header
        .get_mut(4..6)
        .ok_or(Error::Invalid("size"))?
        .copy_from_slice(&l::MAJOR.to_le_bytes());
    header
        .get_mut(6..8)
        .ok_or(Error::Invalid("size"))?
        .copy_from_slice(&minor.to_le_bytes());
    header
        .get_mut(8..12)
        .ok_or(Error::Invalid("size"))?
        .copy_from_slice(&count(data.chunks.len())?.to_le_bytes());
    header
        .get_mut(12..16)
        .ok_or(Error::Invalid("size"))?
        .copy_from_slice(&count(sections)?.to_le_bytes());
    for (index, range) in writer.ranges.iter().enumerate().take(sections) {
        let entry = writer
            .bytes
            .get_mut(16 + index * 8..24 + index * 8)
            .ok_or(Error::Invalid("size"))?;
        entry
            .get_mut(..4)
            .ok_or(Error::Invalid("size"))?
            .copy_from_slice(&count(range.start)?.to_le_bytes());
        entry
            .get_mut(4..8)
            .ok_or(Error::Invalid("size"))?
            .copy_from_slice(&count(range.len())?.to_le_bytes());
    }
    writer.values(
        l::CHUNK_ORIGINS,
        data.chunks
            .iter()
            .flat_map(|c| c.origin)
            .map(f64::to_le_bytes),
    )?;
    writer.values(
        l::CHUNK_BOUNDS,
        data.chunks
            .iter()
            .flat_map(|c| c.bounds)
            .map(f32::to_le_bytes),
    )?;
    writer.values(
        l::CHUNK_INSTANCES,
        data.chunks.iter().map(|c| c.instances.to_le_bytes()),
    )?;
    for (section, size) in [
        (
            l::MESH_VERTICES,
            (|m: &crate::Mesh| m.vertices.len()) as fn(&crate::Mesh) -> usize,
        ),
        (
            l::MESH_TRIANGLES,
            (|m: &crate::Mesh| m.triangles.len()) as fn(&crate::Mesh) -> usize,
        ),
        (
            l::MESH_EDGES,
            (|m: &crate::Mesh| m.edges.len()) as fn(&crate::Mesh) -> usize,
        ),
    ] {
        let mut start = 0_u32;
        writer.values(
            section,
            data.meshes.iter().map(|mesh| {
                let value = start;
                start += size(mesh) as u32;
                value.to_le_bytes()
            }),
        )?;
    }
    writer.values(
        l::VERTEX_XYZ,
        data.meshes
            .iter()
            .flat_map(|m| &m.vertices)
            .flatten()
            .copied()
            .map(f32::to_le_bytes),
    )?;
    writer.values(
        l::TRIANGLE_VERTICES,
        data.meshes
            .iter()
            .flat_map(|m| &m.triangles)
            .flatten()
            .copied()
            .map(u32::to_le_bytes),
    )?;
    writer.values(
        l::EDGE_VERTICES,
        data.meshes
            .iter()
            .flat_map(|m| &m.edges)
            .flatten()
            .copied()
            .map(u32::to_le_bytes),
    )?;
    writer.values(
        l::INSTANCE_MESHES,
        data.instances.iter().map(|i| i.mesh.to_le_bytes()),
    )?;
    writer.values(
        l::INSTANCE_ELEMENTS,
        data.instances.iter().map(|i| i.element.to_le_bytes()),
    )?;
    writer.values(
        l::INSTANCE_COLORS,
        data.instances.iter().map(|i| i.color.to_le_bytes()),
    )?;
    writer.values(
        l::INSTANCE_TRANSFORMS,
        data.instances
            .iter()
            .flat_map(|i| i.transform)
            .map(f32::to_le_bytes),
    )?;
    for (field, section) in [
        (0, l::ELEMENT_IDS),
        (1, l::ELEMENT_CLASSES),
        (2, l::ELEMENT_NAMES),
    ] {
        let mut start = 0_u32;
        writer.values(
            section,
            data.elements.iter().flat_map(|e| {
                let id = e.id.len() as u32;
                let class = e.class.len() as u32;
                let name = e.name.len() as u32;
                let pair = match field {
                    0 => [start, id],
                    1 => [start + id, class],
                    _ => [start + id + class, name],
                };
                start += id + class + name;
                pair.map(u32::to_le_bytes)
            }),
        )?;
    }
    writer.values(
        l::ELEMENT_STOREYS,
        data.elements
            .iter()
            .map(|e| e.storey.unwrap_or(l::NONE).to_le_bytes()),
    )?;
    writer.values(
        l::COLOR_RGBA,
        data.colors
            .iter()
            .flat_map(|c| c.rgba)
            .map(f32::to_le_bytes),
    )?;
    writer.values(
        l::COLOR_FLAGS,
        data.colors.iter().map(|c| {
            [if c.unresolved {
                l::color_flag::UNRESOLVED
            } else {
                0
            }]
        }),
    )?;
    writer.values(
        l::STRINGS,
        data.elements
            .iter()
            .flat_map(|e| [&e.id, &e.class, &e.name])
            .flat_map(|s| s.as_bytes())
            .map(|&b| [b]),
    )?;
    if let Some(points) = points {
        writer.values(
            l::CHUNK_POINTS,
            points.chunks.iter().map(|c| c.start.to_le_bytes()),
        )?;
        writer.values(
            l::CHUNK_NODES,
            points
                .chunks
                .iter()
                .flat_map(|c| c.node.unwrap_or([l::NONE; 4]))
                .map(u32::to_le_bytes),
        )?;
        writer.values(
            l::POINT_XYZ,
            points
                .positions
                .iter()
                .flatten()
                .copied()
                .map(f32::to_le_bytes),
        )?;
        writer.values(
            l::POINT_RGBA,
            points
                .rgba
                .iter()
                .flatten()
                .flatten()
                .copied()
                .map(u16::to_le_bytes),
        )?;
        writer.values(
            l::POINT_INTENSITY,
            points
                .intensity
                .iter()
                .flatten()
                .copied()
                .map(u16::to_le_bytes),
        )?;
        writer.values(
            l::POINT_CLASSIFICATION,
            points.classification.iter().flatten().map(|&v| [v]),
        )?;
    }
    crate::read::read_inner(&writer.bytes, cx)?;
    Ok(writer.bytes)
}
