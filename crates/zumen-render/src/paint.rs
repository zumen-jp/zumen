//! SVG と PNG の資源・色・診断をそろえる。

use zumen_base::{Code, Cx, Diagnostic, Kind, Ref, codes};
use zumen_scene::layout as l;

use crate::{Error, Options, scene::Scene};

pub(crate) fn diagnostic(
    scene: &Scene<'_>,
    code: Code,
    unit: usize,
    cx: &mut Cx<'_>,
) -> Result<(), Error> {
    let entity = scene.uint(l::UNIT_ENTITIES, unit, 0)? as usize;
    let length = u64::from(scene.uint(l::ENTITY_IDS, entity, 1)?);
    cx.check(Kind::Work, length)?;
    cx.check(Kind::DecodedBytes, 1024 + length)?;
    let id = scene.text(l::ENTITY_IDS, entity, 0)?;
    cx.report(
        Diagnostic::new(code)
            .with("unit", unit as u64)
            .at(Ref::default().with_id(id)),
    );
    Ok(())
}

pub(crate) fn glyph_range(scene: &Scene<'_>, row: usize) -> Result<std::ops::Range<usize>, Error> {
    if scene.rows(l::TEXT_GLYPHS) == 0 {
        Ok(0..0)
    } else {
        scene.range(l::TEXT_GLYPHS, l::GLYPH_SHAPES, row)
    }
}

pub(crate) fn color(
    scene: &Scene<'_>,
    options: &Options,
    kind: usize,
    row: usize,
    unit: usize,
    cx: &mut Cx<'_>,
) -> Result<Option<[u8; 4]>, Error> {
    if kind == 4 {
        diagnostic(scene, codes::RENDER_IMAGE_DROPPED, unit, cx)?;
        return Ok(None);
    }
    if kind == 2 && glyph_range(scene, row)?.is_empty() {
        if scene.uint(l::TEXT_STRINGS, row, 1)? != 0 {
            diagnostic(scene, codes::RENDER_TEXT_DROPPED, unit, cx)?;
        }
        return Ok(None);
    }
    if kind == 3
        && (scene.uint(l::POINT_MARKERS, row, 1)? != 0
            || scene.uint(l::POINT_MARKERS, row, 3)? != 0)
    {
        diagnostic(scene, codes::RENDER_MARKER_DROPPED, unit, cx)?;
        return Ok(None);
    }
    let color = scene.uint(l::UNIT_COLORS, unit, 0)? as usize;
    let flags = scene.uint(l::COLOR_FLAGS, color, 0)?;
    Ok(Some(if flags & u32::from(l::color_flag::FOREGROUND) != 0 {
        options.foreground
    } else if flags & u32::from(l::color_flag::UNRESOLVED) != 0 {
        diagnostic(scene, codes::RENDER_COLOR_ASSUMED, unit, cx)?;
        options.foreground
    } else {
        [
            scene.uint(l::COLOR_RGBA, color, 0)? as u8,
            scene.uint(l::COLOR_RGBA, color, 1)? as u8,
            scene.uint(l::COLOR_RGBA, color, 2)? as u8,
            scene.uint(l::COLOR_RGBA, color, 3)? as u8,
        ]
    }))
}
