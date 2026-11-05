//! 借用したシーンを、SVG と同じ座標・描画順・資源の規則で走査する。

use zumen_base::{Cx, Kind};
use zumen_scene::layout as l;

use crate::{
    Error, Options, order, paint,
    png::Encoder,
    raster::{self, Buffer, Path, Point, Shape, Surface},
    scene::Scene,
    svg::Camera,
};

struct Render<'s, 'c, 'a> {
    scene: Scene<'s>,
    options: &'s Options,
    camera: Camera,
    surface: Surface,
    cx: &'c mut Cx<'a>,
}

pub(crate) fn render(bytes: &[u8], options: &Options, cx: &mut Cx<'_>) -> Result<Vec<u8>, Error> {
    cx.check(Kind::Work, 1)?;
    let camera = Camera::new(options)?;
    let scene = Scene::read(bytes, cx)?;
    // 完成した PNG の容量も、画素を初期化する前に検査する。
    let encoder = Encoder::new(options, cx)?;
    let surface = Surface::new(options, cx)?;
    let mut render = Render {
        scene,
        options,
        camera,
        surface,
        cx,
    };
    for chunk in 0..render.scene.chunks {
        render.cx.check(Kind::Work, 1)?;
        let origin = render.scene.xy(l::CHUNK_ORIGINS, chunk)?;
        for [unit, kind, row] in order::build(&render.scene, chunk, render.cx)? {
            render.primitive(kind as usize, row as usize, unit as usize, origin)?;
        }
    }
    encoder.finish(&render.surface.pixels, render.cx)
}

impl Render<'_, '_, '_> {
    fn primitive(
        &mut self,
        kind: usize,
        row: usize,
        unit: usize,
        origin: Point,
    ) -> Result<(), Error> {
        let Some(rgba) = paint::color(&self.scene, self.options, kind, row, unit, self.cx)? else {
            return Ok(());
        };
        let clip = self.scene.uint(l::UNIT_CLIPS, unit, 0)?;
        let range = if clip == l::NONE {
            0..0
        } else {
            self.scene
                .range(l::CLIP_POLYGONS, l::POLYGON_VERTICES, clip as usize)?
        };
        let depth = range.len() as u64;
        self.cx.check(Kind::Depth, depth)?;
        let result = self.clipped(kind, row, unit, origin, rgba, range);
        self.cx.budget.leave(depth);
        result
    }

    fn clipped(
        &mut self,
        kind: usize,
        row: usize,
        unit: usize,
        origin: Point,
        rgba: [u8; 4],
        range: std::ops::Range<usize>,
    ) -> Result<(), Error> {
        let mut clips = Buffer::new(96);
        let [x, y] = self.camera.offset;
        let [w, h] = self.camera.size;
        let mut viewport = Path::new(true);
        viewport.polygon(&[[x, y], [x + w, y], [x + w, y + h], [x, y + h]], self.cx)?;
        clips.push(viewport, self.cx)?;
        for clip in range {
            self.cx.check(Kind::Work, 1)?;
            let mut path = Path::new(true);
            path.inverse = self.scene.uint(l::POLYGON_FLAGS, clip, 0)? == 1;
            self.ring(&mut path, l::POLYGON_VERTICES, l::CLIP_XY, clip, [0.0; 2])?;
            clips.push(path, self.cx)?;
        }
        let weight = self.scene.number(l::UNIT_WEIGHTS, unit, 0)?;
        let weight = if weight == 0.0 {
            1.0
        } else {
            weight * (self.options.dpi / 25.4)
        };
        raster::point([weight, 0.0])?;
        if weight <= 0.0 {
            return Err(Error::Options);
        }
        match kind {
            0 => {
                let mut points = Buffer::new(16);
                for vertex in self.scene.range(l::LINE_VERTICES, l::LINE_XY, row)? {
                    points.push(
                        raster::point(
                            self.camera
                                .point(origin, self.scene.xy(l::LINE_XY, vertex)?),
                        )?,
                        self.cx,
                    )?;
                }
                let mut path = Path::new(false);
                path.stroke(&points.items, weight, false, self.cx)?;
                self.surface
                    .paint(Shape::Path(&path), &clips.items, rgba, self.cx)
            }
            1 => {
                let mut path = Path::new(true);
                for ring in self.scene.range(l::FACE_RINGS, l::RING_VERTICES, row)? {
                    self.cx.check(Kind::Work, 1)?;
                    self.ring(&mut path, l::RING_VERTICES, l::FACE_XY, ring, origin)?;
                }
                self.surface
                    .paint(Shape::Path(&path), &clips.items, rgba, self.cx)
            }
            2 => self.text(row, origin, &clips.items, rgba),
            3 => self.point(row, origin, weight, &clips.items, rgba),
            _ => Err(Error::Scene),
        }
    }

    fn ring(
        &mut self,
        path: &mut Path,
        starts: usize,
        xy: usize,
        row: usize,
        origin: Point,
    ) -> Result<(), Error> {
        let mut first = None;
        let mut previous = None;
        for vertex in self.scene.range(starts, xy, row)? {
            self.cx.check(Kind::Work, 1)?;
            let p = if xy == l::CLIP_XY {
                self.camera.point(self.scene.xy(xy, vertex)?, [0.0; 2])
            } else {
                self.camera.point(origin, self.scene.xy(xy, vertex)?)
            };
            raster::point(p)?;
            if let Some(previous) = previous {
                path.edge(previous, p, self.cx)?;
            } else {
                first = Some(p);
            }
            previous = Some(p);
        }
        if let (Some(a), Some(b)) = (previous, first) {
            path.edge(a, b, self.cx)?;
        }
        Ok(())
    }

    fn text(
        &mut self,
        row: usize,
        origin: Point,
        clips: &[Path],
        rgba: [u8; 4],
    ) -> Result<(), Error> {
        for glyph in paint::glyph_range(&self.scene, row)? {
            self.cx.check(Kind::Work, 1)?;
            let shape = self.scene.uint(l::GLYPH_SHAPES, glyph, 0)? as usize;
            let [e, f] = self.scene.xy(l::GLYPH_XY, glyph)?;
            let a = self.scene.number(l::GLYPH_AXES, glyph, 0)?;
            let b = self.scene.number(l::GLYPH_AXES, glyph, 1)?;
            let c = self.scene.number(l::GLYPH_AXES, glyph, 2)?;
            let d = self.scene.number(l::GLYPH_AXES, glyph, 3)?;
            let mut path = Path::new(false);
            for ring in self
                .scene
                .range(l::SHAPE_RINGS, l::OUTLINE_VERTICES, shape)?
            {
                self.cx.check(Kind::Work, 1)?;
                let mut first = None;
                let mut previous = None;
                for vertex in self.scene.range(l::OUTLINE_VERTICES, l::OUTLINE_XY, ring)? {
                    self.cx.check(Kind::Work, 1)?;
                    let [u, v] = self.scene.xy(l::OUTLINE_XY, vertex)?;
                    let p = raster::point(
                        self.camera
                            .point(origin, [a * u + c * v + e, b * u + d * v + f]),
                    )?;
                    if let Some(previous) = previous {
                        path.edge(previous, p, self.cx)?;
                    } else {
                        first = Some(p);
                    }
                    previous = Some(p);
                }
                if let (Some(a), Some(b)) = (previous, first) {
                    path.edge(a, b, self.cx)?;
                }
            }
            self.surface
                .paint(Shape::Path(&path), clips, rgba, self.cx)?;
        }
        Ok(())
    }

    fn point(
        &mut self,
        row: usize,
        origin: Point,
        weight: f64,
        clips: &[Path],
        rgba: [u8; 4],
    ) -> Result<(), Error> {
        let center = raster::point(self.camera.point(origin, self.scene.xy(l::POINT_XY, row)?))?;
        let (mark, frames, size) = if self.scene.rows(l::POINT_STYLE) == 0 {
            (0, 0, 0.0)
        } else {
            let value = self.scene.number(l::POINT_STYLE, 0, 3)?;
            let size = if self.scene.number(l::POINT_STYLE, 0, 2)? == 0.0 {
                value * self.camera.scale
            } else {
                value * f64::from(self.options.height) / 100.0
            };
            (
                self.scene.number(l::POINT_STYLE, 0, 0)? as u8,
                self.scene.number(l::POINT_STYLE, 0, 1)? as u8,
                size,
            )
        };
        let radius = size * self.scene.number(l::POINT_SCALES, row, 0)?.abs() / 2.0;
        raster::point([radius, 0.0])?;
        if mark == l::point_mark::DOT {
            self.cx.check(Kind::Segments, 1)?;
            self.surface.paint(
                Shape::Circle {
                    at: center,
                    outer: 0.5,
                    inner: 0.0,
                },
                clips,
                rgba,
                self.cx,
            )?;
        }
        if frames & l::point_frame::CIRCLE != 0 && radius > 0.0 {
            self.cx.check(Kind::Segments, 1)?;
            self.surface.paint(
                Shape::Circle {
                    at: center,
                    outer: radius + weight / 2.0,
                    inner: (radius - weight / 2.0).max(0.0),
                },
                clips,
                rgba,
                self.cx,
            )?;
        }
        let angle = self.scene.number(l::POINT_ROTATIONS, row, 0)? * (std::f64::consts::PI / 180.0);
        let (sin, cos) = (libm::sin(angle), libm::cos(angle));
        let [x, y] = center;
        let rotate = |[u, v]: Point| {
            [
                x + radius * (u * cos - v * sin),
                y - radius * (u * sin + v * cos),
            ]
        };
        let lines: &[[Point; 2]] = match mark {
            l::point_mark::PLUS => &[[[-1.0, 0.0], [1.0, 0.0]], [[0.0, -1.0], [0.0, 1.0]]],
            l::point_mark::CROSS => &[[[-1.0, -1.0], [1.0, 1.0]], [[-1.0, 1.0], [1.0, -1.0]]],
            l::point_mark::TICK => &[[[0.0, 0.0], [0.0, 1.0]]],
            _ => &[],
        };
        let mut path = Path::new(false);
        for &[a, b] in lines {
            path.stroke(&[rotate(a), rotate(b)], weight, false, self.cx)?;
        }
        if frames & l::point_frame::SQUARE != 0 {
            path.stroke(
                &[
                    rotate([-1.0, -1.0]),
                    rotate([1.0, -1.0]),
                    rotate([1.0, 1.0]),
                    rotate([-1.0, 1.0]),
                ],
                weight,
                true,
                self.cx,
            )?;
        }
        self.surface.paint(Shape::Path(&path), clips, rgba, self.cx)
    }
}
