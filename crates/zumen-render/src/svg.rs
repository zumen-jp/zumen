//! SVG 1.1 の 7（座標）・8（パス）・11（塗り）・14（切り抜き）に従って書く。

use zumen_base::{Code, Cx, Diagnostic, Kind, Ref, codes};
use zumen_scene::layout as l;

use crate::{
    Error, Options,
    scene::{STREAMS, Scene},
    writer::Writer,
};

struct Camera {
    scale: f64,
    offset: [f64; 2],
    size: [f64; 2],
    region: [f64; 4],
}

impl Camera {
    fn new(o: &Options) -> Result<Self, Error> {
        let [xmin, ymin, xmax, ymax] = o.region;
        let width = xmax - xmin;
        let height = ymax - ymin;
        if o.width == 0
            || o.height == 0
            || !o.dpi.is_finite()
            || o.dpi <= 0.0
            || !o.region.iter().all(|v| v.is_finite())
            || width <= 0.0
            || height <= 0.0
            || !width.is_finite()
            || !height.is_finite()
        {
            return Err(Error::Options);
        }
        let scale = (f64::from(o.width) / width).min(f64::from(o.height) / height);
        if !scale.is_finite() || scale <= 0.0 {
            return Err(Error::Options);
        }
        let size = [width * scale, height * scale];
        if size.iter().any(|v| !v.is_finite() || *v <= 0.0) {
            return Err(Error::Options);
        }
        Ok(Self {
            scale,
            offset: [
                (f64::from(o.width) - width * scale) / 2.0,
                (f64::from(o.height) - height * scale) / 2.0,
            ],
            size,
            region: o.region,
        })
    }

    fn point(&self, origin: [f64; 2], local: [f64; 2]) -> [f64; 2] {
        // 絶対座標に戻してから引くと、原点が大きいシーンの相対座標が丸まる。
        let [ox, oy] = origin;
        let [x, y] = local;
        let [xmin, _, _, ymax] = self.region;
        let [dx, dy] = self.offset;
        [
            ((ox - xmin) + x) * self.scale + dx,
            ((ymax - oy) - y) * self.scale + dy,
        ]
    }
}

struct Render<'s, 'c, 'a> {
    scene: Scene<'s>,
    options: &'s Options,
    camera: Camera,
    out: Writer<'c, 'a>,
}

pub(crate) fn render(bytes: &[u8], options: &Options, cx: &mut Cx<'_>) -> Result<Vec<u8>, Error> {
    cx.check(Kind::Work, 1)?;
    let camera = Camera::new(options)?;
    let scene = Scene::read(bytes, cx)?;
    let mut renderer = Render {
        scene,
        options,
        camera,
        out: Writer::new(cx),
    };
    renderer.document()?;
    Ok(renderer.out.finish())
}

impl Render<'_, '_, '_> {
    fn document(&mut self) -> Result<(), Error> {
        let (width, height) = (self.options.width, self.options.height);
        self.out.formatted(format_args!("<svg xmlns=\"http://www.w3.org/2000/svg\" version=\"1.1\" width=\"{width}\" height=\"{height}\" viewBox=\"0 0 {width} {height}\">\n<defs><clipPath id=\"viewport\" clipPathUnits=\"userSpaceOnUse\"><rect"))?;
        let [left, top] = self.camera.offset;
        let [view_width, view_height] = self.camera.size;
        for (name, value) in [
            ("x", left),
            ("y", top),
            ("width", view_width),
            ("height", view_height),
        ] {
            self.out.formatted(format_args!(" {name}=\""))?;
            self.out.number(value)?;
            self.out.text("\"")?;
        }
        self.out.text("/></clipPath>\n")?;
        for polygon in 0..self.scene.rows(l::POLYGON_VERTICES) {
            self.out.cx.check(Kind::Work, 1)?;
            self.out.formatted(format_args!("<clipPath id=\"p{polygon}\" clipPathUnits=\"userSpaceOnUse\"><path clip-rule=\"evenodd\" d=\""))?;
            if self.scene.uint(l::POLYGON_FLAGS, polygon, 0)? == 1 {
                self.out
                    .formatted(format_args!("M0 0H{width}V{height}H0Z"))?;
            }
            let range = self.scene.range(l::POLYGON_VERTICES, l::CLIP_XY, polygon)?;
            for (index, row) in range.enumerate() {
                let xy = self.scene.xy(l::CLIP_XY, row)?;
                self.vertex(index, self.camera.point(xy, [0.0; 2]))?;
            }
            self.out.text("Z\"/></clipPath>\n")?;
        }
        self.out.text("</defs>\n")?;
        self.out
            .formatted(format_args!("<rect width=\"{width}\" height=\"{height}\""))?;
        self.paint(self.options.background, false)?;
        self.out.text("/>\n")?;
        self.out.text("<g clip-path=\"url(#viewport)\">\n")?;
        for chunk in 0..self.scene.chunks {
            self.out.cx.check(Kind::Work, 1)?;
            let origin = self.scene.xy(l::CHUNK_ORIGINS, chunk)?;
            let mut cursors = [(0, 0); 5];
            for ((start, end), (starts, units)) in cursors.iter_mut().zip(STREAMS) {
                let range = self.scene.range(starts, units, chunk)?;
                *start = range.start;
                *end = range.end;
            }
            loop {
                self.out.cx.check(Kind::Work, 1)?;
                let mut next: Option<(usize, usize, usize)> = None;
                for (kind, (&(row, end), (_, units))) in cursors.iter().zip(STREAMS).enumerate() {
                    if row == end {
                        continue;
                    }
                    let unit = self.scene.uint(units, row, 0)? as usize;
                    if next.is_none_or(|(_, _, previous)| unit < previous) {
                        next = Some((kind, row, unit));
                    }
                }
                let Some((kind, row, unit)) = next else {
                    break;
                };
                self.primitive(kind, row, unit, origin)?;
                let (cursor, _) = cursors.get_mut(kind).ok_or(Error::Scene)?;
                *cursor += 1;
            }
        }
        self.out.text("</g>\n</svg>\n")
    }

    fn paint(&mut self, [r, g, b, a]: [u8; 4], stroke: bool) -> Result<(), Error> {
        self.out.formatted(format_args!(
            " fill=\"#{r:02x}{g:02x}{b:02x}\" fill-opacity=\""
        ))?;
        self.out.number(f64::from(a) / 255.0)?;
        self.out.text("\"")?;
        if stroke {
            self.out.formatted(format_args!(
                " stroke=\"#{r:02x}{g:02x}{b:02x}\" stroke-opacity=\""
            ))?;
            self.out.number(f64::from(a) / 255.0)?;
            self.out.text("\"")?;
        }
        Ok(())
    }

    fn diagnostic(&mut self, code: Code, unit: usize) -> Result<(), Error> {
        let entity = self.scene.uint(l::UNIT_ENTITIES, unit, 0)? as usize;
        let length = u64::from(self.scene.uint(l::ENTITY_IDS, entity, 1)?);
        self.out.cx.check(Kind::Work, length)?;
        // 集約の表・値・場所の分は環境に依らない固定量で数え、ID の複写分を足す。
        self.out.cx.check(Kind::DecodedBytes, 1024 + length)?;
        let id = self.scene.text(l::ENTITY_IDS, entity, 0)?;
        self.out.cx.report(
            Diagnostic::new(code)
                .with("unit", unit as u64)
                .at(Ref::default().with_id(id)),
        );
        Ok(())
    }

    fn primitive(
        &mut self,
        kind: usize,
        row: usize,
        unit: usize,
        origin: [f64; 2],
    ) -> Result<(), Error> {
        self.out.cx.check(Kind::Elements, 1)?;
        if kind == 4 {
            return self.diagnostic(codes::RENDER_IMAGE_DROPPED, unit);
        }
        if kind == 2 && self.glyph_range(row)?.is_empty() {
            if self.scene.uint(l::TEXT_STRINGS, row, 1)? != 0 {
                self.diagnostic(codes::RENDER_TEXT_DROPPED, unit)?;
            }
            return Ok(());
        }
        if kind == 3
            && (self.scene.uint(l::POINT_MARKERS, row, 1)? != 0
                || self.scene.uint(l::POINT_MARKERS, row, 3)? != 0)
        {
            return self.diagnostic(codes::RENDER_MARKER_DROPPED, unit);
        }
        let color = self.scene.uint(l::UNIT_COLORS, unit, 0)? as usize;
        let flags = self.scene.uint(l::COLOR_FLAGS, color, 0)?;
        let rgba = if flags & u32::from(l::color_flag::FOREGROUND) != 0 {
            self.options.foreground
        } else if flags & u32::from(l::color_flag::UNRESOLVED) != 0 {
            self.diagnostic(codes::RENDER_COLOR_ASSUMED, unit)?;
            self.options.foreground
        } else {
            [
                self.scene.uint(l::COLOR_RGBA, color, 0)? as u8,
                self.scene.uint(l::COLOR_RGBA, color, 1)? as u8,
                self.scene.uint(l::COLOR_RGBA, color, 2)? as u8,
                self.scene.uint(l::COLOR_RGBA, color, 3)? as u8,
            ]
        };
        let clip = self.scene.uint(l::UNIT_CLIPS, unit, 0)?;
        let clips = if clip == l::NONE {
            0..0
        } else {
            self.scene
                .range(l::CLIP_POLYGONS, l::POLYGON_VERTICES, clip as usize)?
        };
        // 失敗しても、呼び出す側の depth を戻す。全体を一度に確かめてからタグを書く。
        let depth = clips.len() as u64;
        self.out.cx.check(Kind::Depth, depth)?;
        let result = self.clipped_primitive(kind, row, origin, unit, rgba, clips);
        self.out.cx.budget.leave(depth);
        result
    }

    fn clipped_primitive(
        &mut self,
        kind: usize,
        row: usize,
        origin: [f64; 2],
        unit: usize,
        rgba: [u8; 4],
        clips: std::ops::Range<usize>,
    ) -> Result<(), Error> {
        for clip in clips.clone() {
            self.out
                .formatted(format_args!("<g clip-path=\"url(#p{clip})\">"))?;
        }
        self.out.text("<g")?;
        self.paint(rgba, true)?;
        let weight = self.scene.number(l::UNIT_WEIGHTS, unit, 0)?;
        let weight = if weight == 0.0 {
            1.0
        } else {
            weight * (self.options.dpi / 25.4)
        };
        if weight <= 0.0 {
            return Err(Error::Options);
        }
        self.out.text(" stroke-width=\"")?;
        self.out.number(weight)?;
        self.out.text(
            "\" stroke-linecap=\"butt\" stroke-linejoin=\"miter\" stroke-miterlimit=\"4\">",
        )?;
        match kind {
            0 => {
                self.out.text("<path fill=\"none\" d=\"")?;
                self.path(l::LINE_VERTICES, l::LINE_XY, row, origin, false)?;
                self.out.text("\"/>")?;
            }
            1 => {
                self.out
                    .text("<path stroke=\"none\" fill-rule=\"evenodd\" d=\"")?;
                for ring in self.scene.range(l::FACE_RINGS, l::RING_VERTICES, row)? {
                    self.out.cx.check(Kind::Work, 1)?;
                    self.path(l::RING_VERTICES, l::FACE_XY, ring, origin, true)?;
                }
                self.out.text("\"/>")?;
            }
            2 => self.text(row, origin)?,
            3 => self.point(row, origin)?,
            _ => return Err(Error::Scene),
        }
        self.out.text("</g>")?;
        for _ in clips {
            self.out.text("</g>")?;
        }
        self.out.text("\n")
    }

    fn vertex(&mut self, index: usize, [x, y]: [f64; 2]) -> Result<(), Error> {
        self.out.cx.check(Kind::Segments, 1)?;
        self.out.text(if index == 0 { "M" } else { "L" })?;
        self.out.number(x)?;
        self.out.text(" ")?;
        self.out.number(y)
    }

    fn path(
        &mut self,
        starts: usize,
        xy: usize,
        row: usize,
        origin: [f64; 2],
        closed: bool,
    ) -> Result<(), Error> {
        let range = self.scene.range(starts, xy, row)?;
        let empty = range.is_empty();
        for (index, vertex) in range.enumerate() {
            self.vertex(index, self.camera.point(origin, self.scene.xy(xy, vertex)?))?;
        }
        if closed && !empty {
            self.out.text("Z")?;
        }
        Ok(())
    }

    fn glyph_range(&self, row: usize) -> Result<std::ops::Range<usize>, Error> {
        if self.scene.rows(l::TEXT_GLYPHS) == 0 {
            Ok(0..0)
        } else {
            self.scene.range(l::TEXT_GLYPHS, l::GLYPH_SHAPES, row)
        }
    }

    fn text(&mut self, row: usize, origin: [f64; 2]) -> Result<(), Error> {
        // SVG 11.3.1。字形は nonzero。各字形の alpha を独立に合成する。
        for glyph in self.glyph_range(row)? {
            self.out.cx.check(Kind::Work, 1)?;
            let shape = self.scene.uint(l::GLYPH_SHAPES, glyph, 0)? as usize;
            let [e, f] = self.scene.xy(l::GLYPH_XY, glyph)?;
            let a = self.scene.number(l::GLYPH_AXES, glyph, 0)?;
            let b = self.scene.number(l::GLYPH_AXES, glyph, 1)?;
            let c = self.scene.number(l::GLYPH_AXES, glyph, 2)?;
            let d = self.scene.number(l::GLYPH_AXES, glyph, 3)?;
            self.out
                .text("<path stroke=\"none\" fill-rule=\"nonzero\" d=\"")?;
            for ring in self
                .scene
                .range(l::SHAPE_RINGS, l::OUTLINE_VERTICES, shape)?
            {
                self.out.cx.check(Kind::Work, 1)?;
                let range = self.scene.range(l::OUTLINE_VERTICES, l::OUTLINE_XY, ring)?;
                let empty = range.is_empty();
                for (index, vertex) in range.enumerate() {
                    let [u, v] = self.scene.xy(l::OUTLINE_XY, vertex)?;
                    self.vertex(
                        index,
                        self.camera
                            .point(origin, [a * u + c * v + e, b * u + d * v + f]),
                    )?;
                }
                if !empty {
                    self.out.text("Z")?;
                }
            }
            self.out.text("\"/>")?;
        }
        Ok(())
    }

    fn point(&mut self, row: usize, origin: [f64; 2]) -> Result<(), Error> {
        let center = self.camera.point(origin, self.scene.xy(l::POINT_XY, row)?);
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
        if !radius.is_finite() {
            return Err(Error::Options);
        }
        if mark == l::point_mark::DOT {
            self.circle(center, 0.5, true)?;
        }
        if frames & l::point_frame::CIRCLE != 0 {
            self.circle(center, radius, false)?;
        }
        let angle = self.scene.number(l::POINT_ROTATIONS, row, 0)? * (std::f64::consts::PI / 180.0);
        let (sin, cos) = (libm::sin(angle), libm::cos(angle));
        let [x, y] = center;
        let rotate = |[u, v]: [f64; 2]| {
            [
                x + radius * (u * cos - v * sin),
                y - radius * (u * sin + v * cos),
            ]
        };
        self.out.text("<path fill=\"none\" d=\"")?;
        let lines: &[[[f64; 2]; 2]] = match mark {
            l::point_mark::PLUS => &[[[-1.0, 0.0], [1.0, 0.0]], [[0.0, -1.0], [0.0, 1.0]]],
            l::point_mark::CROSS => &[[[-1.0, -1.0], [1.0, 1.0]], [[-1.0, 1.0], [1.0, -1.0]]],
            l::point_mark::TICK => &[[[0.0, 0.0], [0.0, 1.0]]],
            _ => &[],
        };
        for line in lines {
            for (index, xy) in line.iter().enumerate() {
                self.vertex(index, rotate(*xy))?;
            }
        }
        if frames & l::point_frame::SQUARE != 0 {
            for (index, xy) in [[-1.0, -1.0], [1.0, -1.0], [1.0, 1.0], [-1.0, 1.0]]
                .into_iter()
                .enumerate()
            {
                self.vertex(index, rotate(xy))?;
            }
            self.out.text("Z")?;
        }
        self.out.text("\"/>")
    }

    fn circle(&mut self, [x, y]: [f64; 2], radius: f64, fill: bool) -> Result<(), Error> {
        self.out.cx.check(Kind::Segments, 1)?;
        self.out.text(if fill {
            "<circle stroke=\"none\" cx=\""
        } else {
            "<circle fill=\"none\" cx=\""
        })?;
        self.out.number(x)?;
        self.out.text("\" cy=\"")?;
        self.out.number(y)?;
        self.out.text("\" r=\"")?;
        self.out.number(radius)?;
        self.out.text("\"/>")
    }
}
