//! ZMSC の借用した列を検査する（決定 0137・0178・0188）。

use std::ops::Range;
use zumen_base::{Cx, Diagnostic, Kind, codes};
use zumen_scene::layout::{self as l, SECTIONS, Type};

use crate::Error;

pub(crate) struct Scene<'a> {
    columns: [&'a [u8]; SECTIONS.len()],
    pub chunks: usize,
}

fn array<const N: usize>(bytes: &[u8], at: usize) -> Result<[u8; N], Error> {
    bytes
        .get(at..at.checked_add(N).ok_or(Error::Scene)?)
        .and_then(|b| b.try_into().ok())
        .ok_or(Error::Scene)
}

impl<'a> Scene<'a> {
    pub fn read(bytes: &'a [u8], cx: &mut Cx<'_>) -> Result<Self, Error> {
        cx.check(Kind::InputBytes, bytes.len() as u64)?;
        if array::<4>(bytes, 0)? != l::MAGIC || u16::from_le_bytes(array(bytes, 4)?) != l::MAJOR {
            return Err(Error::Scene);
        }
        let chunks = u32::from_le_bytes(array(bytes, 8)?) as usize;
        let sections = u32::from_le_bytes(array(bytes, 12)?) as usize;
        let table_end = sections
            .checked_mul(8)
            .and_then(|n| n.checked_add(16))
            .ok_or(Error::Scene)?;
        if table_end > bytes.len() {
            return Err(Error::Scene);
        }
        let mut scene = Self {
            columns: [&[]; SECTIONS.len()],
            chunks,
        };
        for index in 0..sections {
            cx.check(Kind::Work, 1)?;
            let at = 16 + index * 8;
            let offset = u32::from_le_bytes(array(bytes, at)?) as usize;
            let length = u32::from_le_bytes(array(bytes, at + 4)?) as usize;
            let next = offset.checked_add(length).ok_or(Error::Scene)?;
            // 列の物理的な並びは表の順とは限らない。借用だけなので重複しても書き換えない。
            if offset < table_end || !offset.is_multiple_of(8) || next > bytes.len() {
                return Err(Error::Scene);
            }
            if let (Some(column), Some(layout)) =
                (scene.columns.get_mut(index), SECTIONS.get(index))
            {
                if !length.is_multiple_of(layout.ty.size() * layout.width) {
                    return Err(Error::Scene);
                }
                *column = bytes.get(offset..next).ok_or(Error::Scene)?;
            } else if length != 0 {
                cx.check(Kind::DecodedBytes, 1024)?;
                cx.report(
                    Diagnostic::new(codes::RENDER_SECTION_DROPPED).with("section", index as u64),
                );
            }
        }
        scene.validate(cx)?;
        Ok(scene)
    }

    fn column(&self, section: usize) -> Result<&'a [u8], Error> {
        self.columns.get(section).copied().ok_or(Error::Scene)
    }

    pub fn rows(&self, section: usize) -> usize {
        self.columns
            .get(section)
            .zip(SECTIONS.get(section))
            .map_or(0, |(bytes, layout)| {
                bytes.len() / (layout.ty.size() * layout.width)
            })
    }

    fn cell(&self, section: usize, row: usize, column: usize) -> Result<usize, Error> {
        let layout = SECTIONS.get(section).ok_or(Error::Scene)?;
        if row >= self.rows(section) || column >= layout.width {
            return Err(Error::Scene);
        }
        row.checked_mul(layout.width)
            .and_then(|n| n.checked_add(column))
            .and_then(|n| n.checked_mul(layout.ty.size()))
            .ok_or(Error::Scene)
    }

    pub fn uint(&self, section: usize, row: usize, column: usize) -> Result<u32, Error> {
        let at = self.cell(section, row, column)?;
        let bytes = self.column(section)?;
        match SECTIONS.get(section).map(|s| s.ty) {
            Some(Type::U8) => bytes.get(at).copied().map(u32::from).ok_or(Error::Scene),
            Some(Type::U32) => Ok(u32::from_le_bytes(array(bytes, at)?)),
            _ => Err(Error::Scene),
        }
    }

    pub fn number(&self, section: usize, row: usize, column: usize) -> Result<f64, Error> {
        let at = self.cell(section, row, column)?;
        let bytes = self.column(section)?;
        match SECTIONS.get(section).map(|s| s.ty) {
            Some(Type::F32) => Ok(f64::from(f32::from_le_bytes(array(bytes, at)?))),
            Some(Type::F64) => Ok(f64::from_le_bytes(array(bytes, at)?)),
            _ => Err(Error::Scene),
        }
    }

    pub fn xy(&self, section: usize, row: usize) -> Result<[f64; 2], Error> {
        Ok([self.number(section, row, 0)?, self.number(section, row, 1)?])
    }

    pub fn range(&self, starts: usize, target: usize, row: usize) -> Result<Range<usize>, Error> {
        let start = self.uint(starts, row, 0)? as usize;
        let end = if row + 1 < self.rows(starts) {
            self.uint(starts, row + 1, 0)? as usize
        } else {
            self.rows(target)
        };
        if start > end || end > self.rows(target) {
            return Err(Error::Scene);
        }
        Ok(start..end)
    }

    pub fn text(&self, section: usize, row: usize, column: usize) -> Result<&'a str, Error> {
        let start = self.uint(section, row, column)? as usize;
        let length = self.uint(section, row, column + 1)? as usize;
        let end = start.checked_add(length).ok_or(Error::Scene)?;
        std::str::from_utf8(
            self.column(l::STRINGS)?
                .get(start..end)
                .ok_or(Error::Scene)?,
        )
        .map_err(|_| Error::Scene)
    }

    fn validate(&self, cx: &mut Cx<'_>) -> Result<(), Error> {
        // 並列の列は同じ行数である。字形の列だけは古いシーンではすべて空を認める。
        for (base, columns) in [
            (l::CHUNK_ORIGINS, l::CHUNK_ORIGINS..=l::CHUNK_IMAGES),
            (l::LINE_VERTICES, l::LINE_VERTICES..=l::LINE_UNITS),
            (l::FACE_RINGS, l::FACE_RINGS..=l::FACE_UNITS),
            (l::TEXT_STRINGS, l::TEXT_STRINGS..=l::TEXT_UNITS),
            (l::POINT_XY, l::POINT_XY..=l::POINT_UNITS),
            (l::IMAGE_NAMES, l::IMAGE_NAMES..=l::IMAGE_UNITS),
            (l::UNIT_ENTITIES, l::UNIT_ENTITIES..=l::UNIT_LAYERS),
            (l::ENTITY_IDS, l::ENTITY_IDS..=l::ENTITY_PATHS),
            (l::STEP_IDS, l::STEP_IDS..=l::STEP_CELLS),
            (l::COLOR_SYSTEMS, l::COLOR_SYSTEMS..=l::COLOR_FLAGS),
            (l::POLYGON_VERTICES, l::POLYGON_VERTICES..=l::POLYGON_FLAGS),
            (l::GLYPH_SHAPES, l::GLYPH_SHAPES..=l::GLYPH_XY),
        ] {
            for col in columns {
                if self.rows(col) != self.rows(base) {
                    return Err(Error::Scene);
                }
            }
        }
        if self.chunks != self.rows(l::CHUNK_ORIGINS) || self.rows(l::POINT_STYLE) > 1 {
            return Err(Error::Scene);
        }
        let glyphs = self.rows(l::TEXT_GLYPHS);
        if glyphs != 0 && glyphs != self.rows(l::TEXT_STRINGS)
            || glyphs == 0 && self.rows(l::GLYPH_SHAPES) != 0
        {
            return Err(Error::Scene);
        }
        for (starts, target) in [
            (l::CHUNK_LINES, l::LINE_VERTICES),
            (l::CHUNK_FACES, l::FACE_RINGS),
            (l::CHUNK_TEXTS, l::TEXT_STRINGS),
            (l::CHUNK_POINTS, l::POINT_XY),
            (l::CHUNK_IMAGES, l::IMAGE_NAMES),
            (l::LINE_VERTICES, l::LINE_XY),
            (l::FACE_RINGS, l::RING_VERTICES),
            (l::RING_VERTICES, l::FACE_XY),
            (l::ENTITY_PATHS, l::STEP_IDS),
            (l::CLIP_POLYGONS, l::POLYGON_VERTICES),
            (l::POLYGON_VERTICES, l::CLIP_XY),
            (l::TEXT_GLYPHS, l::GLYPH_SHAPES),
            (l::SHAPE_RINGS, l::OUTLINE_VERTICES),
            (l::OUTLINE_VERTICES, l::OUTLINE_XY),
        ] {
            if self.rows(starts) == 0 && self.rows(target) != 0 {
                return Err(Error::Scene);
            }
            if self.rows(starts) > 0 && self.uint(starts, 0, 0)? != 0 {
                return Err(Error::Scene);
            }
            for row in 0..self.rows(starts) {
                cx.check(Kind::Work, 1)?;
                self.range(starts, target, row)?;
            }
        }
        for (section, target, none) in [
            (l::LINE_UNITS, l::UNIT_ENTITIES, false),
            (l::FACE_UNITS, l::UNIT_ENTITIES, false),
            (l::TEXT_UNITS, l::UNIT_ENTITIES, false),
            (l::POINT_UNITS, l::UNIT_ENTITIES, false),
            (l::IMAGE_UNITS, l::UNIT_ENTITIES, false),
            (l::UNIT_ENTITIES, l::ENTITY_IDS, false),
            (l::UNIT_COLORS, l::COLOR_SYSTEMS, false),
            (l::UNIT_CLIPS, l::CLIP_POLYGONS, true),
            (l::UNIT_LAYERS, l::LAYER_IDS, true),
            (l::TEXT_FONTS, l::FONT_NAMES, true),
            (l::GLYPH_SHAPES, l::SHAPE_RINGS, false),
        ] {
            for row in 0..self.rows(section) {
                cx.check(Kind::Work, 1)?;
                let value = self.uint(section, row, 0)?;
                if !(none && value == l::NONE) && value as usize >= self.rows(target) {
                    return Err(Error::Scene);
                }
            }
        }
        for section in [
            l::TEXT_STRINGS,
            l::IMAGE_NAMES,
            l::IMAGE_PAGES,
            l::ENTITY_IDS,
            l::STEP_IDS,
            l::LAYER_IDS,
            l::FONT_NAMES,
            l::POINT_MARKERS,
        ] {
            for row in 0..self.rows(section) {
                for col in (0..SECTIONS.get(section).ok_or(Error::Scene)?.width).step_by(2) {
                    // 文字列の参照が重なる場合も、その長さだけ読む作業を数える。
                    cx.check(Kind::Work, 1 + u64::from(self.uint(section, row, col + 1)?))?;
                    self.text(section, row, col)?;
                }
            }
        }
        for (section, layout) in SECTIONS.iter().enumerate() {
            if !matches!(layout.ty, Type::F32 | Type::F64) {
                continue;
            }
            for row in 0..self.rows(section) {
                cx.check(Kind::Work, layout.width as u64)?;
                if section == l::TEXT_SECOND {
                    let [x, y] = self.xy(section, row)?;
                    if x.is_nan() && y.is_nan() {
                        continue;
                    }
                }
                for col in 0..layout.width {
                    if !self.number(section, row, col)?.is_finite() {
                        return Err(Error::Scene);
                    }
                }
            }
        }
        for row in 0..self.rows(l::UNIT_WEIGHTS) {
            cx.check(Kind::Work, 1)?;
            if self.number(l::UNIT_WEIGHTS, row, 0)? < 0.0 {
                return Err(Error::Scene);
            }
        }
        for (section, mask) in [(l::COLOR_FLAGS, 3), (l::POLYGON_FLAGS, 1)] {
            for row in 0..self.rows(section) {
                cx.check(Kind::Work, 1)?;
                if self.uint(section, row, 0)? & !mask != 0 {
                    return Err(Error::Scene);
                }
            }
        }
        for polygon in 0..self.rows(l::POLYGON_VERTICES) {
            cx.check(Kind::Work, 1)?;
            if self.range(l::POLYGON_VERTICES, l::CLIP_XY, polygon)?.len() < 3 {
                return Err(Error::Scene);
            }
        }
        if self.rows(l::POINT_STYLE) != 0 {
            for (col, max) in [(0, 4.0), (1, 3.0), (2, 1.0)] {
                let value = self.number(l::POINT_STYLE, 0, col)?;
                if value < 0.0 || value > max || value != f64::from(value as u8) {
                    return Err(Error::Scene);
                }
            }
            if self.number(l::POINT_STYLE, 0, 3)? < 0.0 {
                return Err(Error::Scene);
            }
        }
        // 各塊・種類で unit が昇順なので、5 列だけを逐次マージできる。
        for chunk in 0..self.chunks {
            for (starts, units) in STREAMS {
                let mut previous = 0;
                for row in self.range(starts, units, chunk)? {
                    cx.check(Kind::Work, 1)?;
                    let unit = self.uint(units, row, 0)?;
                    if unit < previous {
                        return Err(Error::Scene);
                    }
                    previous = unit;
                }
            }
        }
        Ok(())
    }
}

pub(crate) const STREAMS: [(usize, usize); 5] = [
    (l::CHUNK_LINES, l::LINE_UNITS),
    (l::CHUNK_FACES, l::FACE_UNITS),
    (l::CHUNK_TEXTS, l::TEXT_UNITS),
    (l::CHUNK_POINTS, l::POINT_UNITS),
    (l::CHUNK_IMAGES, l::IMAGE_UNITS),
];
