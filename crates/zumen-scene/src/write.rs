//! シーンのバッファーを、`schema/scene.toml` の配置で書く（決定 0137・0178）。

use zumen_drawing::Clipping;

use crate::layout::{self, NONE, SECTIONS};
use crate::{Chunk, ColorRow, Counts, Entity, Strings, Unit, Xy};

/// 塊のほかに書く表
pub(crate) struct Tables<'a> {
    pub units: &'a [Unit],
    pub entities: &'a [Entity],
    pub steps: &'a [([u32; 2], [u32; 2])],
    pub colors: &'a [ColorRow],
    pub clips: &'a [Vec<Clipping>],
    pub layers: &'a [[u32; 2]],
    pub fonts: &'a [String],
    pub point_style: Option<[f32; 4]>,
}

/// 区画ごとの列
struct Columns(Vec<Vec<u8>>);

impl Columns {
    fn u8(&mut self, section: usize, values: &[u8]) {
        if let Some(column) = self.0.get_mut(section) {
            column.extend_from_slice(values);
        }
    }

    fn u32(&mut self, section: usize, values: &[u32]) {
        if let Some(column) = self.0.get_mut(section) {
            values
                .iter()
                .for_each(|v| column.extend_from_slice(&v.to_le_bytes()));
        }
    }

    fn f32(&mut self, section: usize, values: &[f32]) {
        if let Some(column) = self.0.get_mut(section) {
            values
                .iter()
                .for_each(|v| column.extend_from_slice(&v.to_le_bytes()));
        }
    }

    fn f64(&mut self, section: usize, values: &[f64]) {
        if let Some(column) = self.0.get_mut(section) {
            values
                .iter()
                .for_each(|v| column.extend_from_slice(&v.to_le_bytes()));
        }
    }
}

/// 行の番号。u32 に入らなければ「なし」
fn index(value: usize) -> u32 {
    u32::try_from(value).unwrap_or(NONE)
}

/// 塊の原点からの相対の f32
fn relative(origin: Xy, [x, y]: Xy) -> [f32; 2] {
    [(x - origin[0]) as f32, (y - origin[1]) as f32]
}

/// バッファーと、描くものの数と、塊の範囲を返す
pub(crate) fn write(
    chunks: &[&Chunk],
    tables: &Tables<'_>,
    strings: &mut Strings,
) -> (Vec<u8>, Counts, Option<[f64; 4]>) {
    let mut c = Columns(vec![Vec::new(); SECTIONS.len()]);
    let mut counts = Counts::default();
    let mut extent = crate::Bounds::default();
    let (mut lines, mut line_xy, mut faces, mut rings, mut face_xy) = (0, 0, 0, 0, 0);
    let (mut texts, mut points, mut images) = (0, 0, 0);
    for chunk in chunks {
        let bounds = chunk.bounds.get().unwrap_or([0.0; 4]);
        extent = extent.union(chunk.bounds);
        let origin = [bounds[0], bounds[1]];
        c.f64(layout::CHUNK_ORIGINS, &origin);
        let [w, h] = relative(origin, [bounds[2], bounds[3]]);
        c.f32(layout::CHUNK_BOUNDS, &[0.0, 0.0, w, h]);
        c.u32(layout::CHUNK_LINES, &[index(lines)]);
        c.u32(layout::CHUNK_FACES, &[index(faces)]);
        c.u32(layout::CHUNK_TEXTS, &[index(texts)]);
        c.u32(layout::CHUNK_POINTS, &[index(points)]);
        c.u32(layout::CHUNK_IMAGES, &[index(images)]);
        for line in &chunk.lines {
            c.u32(layout::LINE_VERTICES, &[index(line_xy)]);
            c.u32(layout::LINE_UNITS, &[line.unit]);
            for p in &line.points {
                c.f32(layout::LINE_XY, &relative(origin, *p));
            }
            line_xy += line.points.len();
            lines += 1;
        }
        for face in &chunk.faces {
            c.u32(layout::FACE_RINGS, &[index(rings)]);
            c.u32(layout::FACE_UNITS, &[face.unit]);
            for ring in &face.rings {
                c.u32(layout::RING_VERTICES, &[index(face_xy)]);
                for p in ring {
                    c.f32(layout::FACE_XY, &relative(origin, *p));
                }
                face_xy += ring.len();
                rings += 1;
            }
            faces += 1;
        }
        for text in &chunk.texts {
            c.u32(layout::TEXT_STRINGS, &text.string);
            c.f32(layout::TEXT_XY, &relative(origin, text.at));
            let second = text
                .second
                .map_or([f32::NAN; 2], |second| relative(origin, second));
            c.f32(layout::TEXT_SECOND, &second);
            c.f32(layout::TEXT_HEIGHTS, &[text.height as f32]);
            c.f32(layout::TEXT_WIDTHS, &[text.width as f32]);
            c.f32(layout::TEXT_ROTATIONS, &[text.rotation as f32]);
            c.f32(layout::TEXT_OBLIQUES, &[text.oblique as f32]);
            c.f32(layout::TEXT_BOXES, &[text.box_width as f32]);
            c.u8(layout::TEXT_ALIGNS, &text.align);
            c.u8(layout::TEXT_FLAGS, &[text.flags]);
            c.u32(layout::TEXT_LINES, &[text.line]);
            c.u32(layout::TEXT_FONTS, &[text.font]);
            c.u32(layout::TEXT_UNITS, &[text.unit]);
            texts += 1;
        }
        for point in &chunk.points {
            c.f32(layout::POINT_XY, &relative(origin, point.at));
            c.f32(layout::POINT_ROTATIONS, &[point.rotation as f32]);
            c.u32(layout::POINT_MARKERS, &point.marker);
            c.f32(layout::POINT_SCALES, &[point.scale as f32]);
            c.u32(layout::POINT_UNITS, &[point.unit]);
            points += 1;
        }
        for image in &chunk.images {
            c.u32(layout::IMAGE_NAMES, &image.name);
            c.u32(layout::IMAGE_PAGES, &image.page);
            c.f32(layout::IMAGE_XY, &relative(origin, image.at));
            let [u, v] = image.edges;
            c.f32(
                layout::IMAGE_EDGES,
                &[u[0] as f32, u[1] as f32, v[0] as f32, v[1] as f32],
            );
            c.u8(layout::IMAGE_KINDS, &[image.kind]);
            c.u8(layout::IMAGE_FRAMES, &[image.frame]);
            c.u32(layout::IMAGE_UNITS, &[image.unit]);
            images += 1;
        }
    }
    if let Some(style) = tables.point_style {
        c.f32(layout::POINT_STYLE, &style);
    }
    for unit in tables.units {
        c.u32(layout::UNIT_ENTITIES, &[unit.entity]);
        c.u32(layout::UNIT_COLORS, &[unit.color]);
        c.f32(layout::UNIT_WEIGHTS, &[unit.weight]);
        c.u32(layout::UNIT_CLIPS, &[unit.clip]);
        c.u32(layout::UNIT_LAYERS, &[unit.layer]);
    }
    for entity in tables.entities {
        c.u32(layout::ENTITY_IDS, &entity.id);
        c.u8(layout::ENTITY_KINDS, &[entity.kind]);
        c.u32(layout::ENTITY_PATHS, &[entity.path]);
    }
    for (id, cell) in tables.steps {
        c.u32(layout::STEP_IDS, id);
        c.u32(layout::STEP_CELLS, cell);
    }
    for color in tables.colors {
        c.u8(layout::COLOR_SYSTEMS, &[color.system]);
        c.u32(layout::COLOR_CODES, &[color.code]);
        c.u8(layout::COLOR_FILES, &color.file);
        c.u8(layout::COLOR_RGBA, &color.rgba);
        c.u8(layout::COLOR_FLAGS, &[color.flags]);
    }
    let (mut polygons, mut clip_xy) = (0, 0);
    for clip in tables.clips {
        c.u32(layout::CLIP_POLYGONS, &[index(polygons)]);
        for polygon in clip {
            c.u32(layout::POLYGON_VERTICES, &[index(clip_xy)]);
            c.u8(layout::POLYGON_FLAGS, &[u8::from(polygon.inverted)]);
            for p in &polygon.polygon {
                c.f64(layout::CLIP_XY, p);
            }
            clip_xy += polygon.polygon.len();
            polygons += 1;
        }
    }
    for layer in tables.layers {
        c.u32(layout::LAYER_IDS, layer);
    }
    for font in tables.fonts {
        let name = strings.add(font);
        c.u32(layout::FONT_NAMES, &name);
    }
    c.u8(layout::STRINGS, &strings.bytes);

    counts.chunks = chunks.len() as u64;
    counts.lines = lines as u64;
    counts.vertices = (line_xy + face_xy) as u64;
    counts.faces = faces as u64;
    counts.texts = texts as u64;
    counts.points = points as u64;
    counts.images = images as u64;
    (assemble(index(chunks.len()), &c.0), counts, extent.get())
}

/// 見出し・区画の表・区画を、8 バイトの境界に並べる（決定 0137）
fn assemble(count: u32, columns: &[Vec<u8>]) -> Vec<u8> {
    let align = |n: usize| n.div_ceil(8) * 8;
    let head = 16 + 8 * columns.len();
    let total = columns
        .iter()
        .fold(align(head), |at, column| align(at + column.len()));
    let mut out = Vec::with_capacity(total);
    out.extend_from_slice(&layout::MAGIC);
    out.extend_from_slice(&layout::MAJOR.to_le_bytes());
    out.extend_from_slice(&layout::MINOR.to_le_bytes());
    out.extend_from_slice(&count.to_le_bytes());
    out.extend_from_slice(&index(columns.len()).to_le_bytes());
    let mut at = align(head);
    for column in columns {
        out.extend_from_slice(&index(at).to_le_bytes());
        out.extend_from_slice(&index(column.len()).to_le_bytes());
        at = align(at + column.len());
    }
    for column in columns {
        out.resize(align(out.len()), 0);
        out.extend_from_slice(column);
    }
    out.resize(align(out.len()), 0);
    out
}
