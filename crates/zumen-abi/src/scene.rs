//! `scene` の op（決定 0172）。起点と色の表を読み、`zumen-scene` でシーンを作る。

use zumen_base::Cx;
use zumen_drawing::layout::Reference;
use zumen_drawing::{ColorSystem, Drawing2D, Origin, Unit, id};
use zumen_json::Value;
use zumen_scene::{Chunking, FontSet, Options, Palette};

use crate::messages::{self, Extent, HeightReference, SceneCounts, SceneRequest, SceneResult};
use crate::session::{Failed, fail, invalid_value};

/// 要求の値を、図面を変換する前に確かめる
pub(crate) fn check(request: &SceneRequest, cx: &mut Cx<'_>) -> Result<(), Failed> {
    match request.region.as_deref() {
        None => {}
        Some(&[min_x, min_y, max_x, max_y]) if min_x <= max_x && min_y <= max_y => {}
        Some(_) => return Err(fail(cx, invalid_value("/region"))),
    }
    if request
        .tolerance
        .is_some_and(|t| !(t.is_finite() && t > 0.0))
    {
        return Err(fail(cx, invalid_value("/tolerance")));
    }
    Ok(())
}

/// 起点の ID を、たどる起点と座標の単位にする
fn origin(
    drawing: &Drawing2D,
    request: &SceneRequest,
    cx: &mut Cx<'_>,
) -> Result<(Origin, messages::Unit), Failed> {
    let from = request.from.as_str();
    let space = drawing
        .spaces
        .iter()
        .find(|space| id::space(space.id) == from)
        .or_else(|| {
            let mut namer = id::DefinitionNamer::default();
            drawing
                .symbols
                .iter()
                .find(|definition| namer.next(definition) == from)
                .map(|definition| &definition.space)
        });
    if let Some(space) = space {
        if request.view.is_some() {
            return Err(fail(cx, invalid_value("/view")));
        }
        return Ok((Origin::Space(space.id), crate::session::unit_of(space.unit)));
    }
    let mut namer = id::SheetNamer::default();
    let Some(sheet) = drawing
        .sheets
        .iter()
        .find(|sheet| namer.next(sheet) == from)
    else {
        return Err(fail(cx, invalid_value("/from")));
    };
    let origin = match request.view {
        None => Origin::Sheet(sheet.id),
        Some(view) => match usize::try_from(view) {
            Ok(view) if view < sheet.views.len() => Origin::View {
                sheet: sheet.id,
                view,
            },
            _ => return Err(fail(cx, invalid_value("/view"))),
        },
    };
    Ok((origin, crate::session::unit_of(Unit::Millimeter)))
}

/// 色の表の JSON（プロファイルの色の語彙と同じ形）を読む
pub(crate) fn palette(bytes: &[u8]) -> Option<Palette> {
    let value = zumen_json::parse(bytes).ok()?;
    let Value::Array(values) = value.pointer("/values")? else {
        return None;
    };
    let mut palette = Palette::default();
    for entry in values {
        let Value::Object(entry) = entry else {
            return None;
        };
        let Some(Value::String(rgb)) = entry.get("rgb") else {
            return None;
        };
        let rgb = hex(rgb)?;
        for (key, system) in [
            ("aci", ColorSystem::Aci),
            ("sxf", ColorSystem::Sxf),
            ("jww", ColorSystem::Jww),
        ] {
            match entry.get(key) {
                None => {}
                Some(Value::Number(number)) => {
                    let code = u32::try_from(number.to_safe_integer()?).ok()?;
                    palette.insert(system, code, rgb);
                }
                Some(_) => return None,
            }
        }
    }
    Some(palette)
}

/// `#rrggbb` の RGB
fn hex(text: &str) -> Option<[u8; 3]> {
    let digits = text.strip_prefix('#')?;
    if digits.len() != 6 || !digits.is_ascii() {
        return None;
    }
    let byte = |at: usize| u8::from_str_radix(digits.get(at..at + 2)?, 16).ok();
    Some([byte(0)?, byte(2)?, byte(4)?])
}

/// 宣言された高さが当たるフォントの寸法（決定 0188）
pub(crate) fn reference(value: HeightReference) -> Reference {
    match value {
        HeightReference::CapHeight => Reference::CapHeight,
        HeightReference::Em => Reference::Em,
        HeightReference::Frame => Reference::Frame,
        HeightReference::CapitalA => Reference::CapitalA,
    }
}

/// シーンを作り、結果とバッファーを返す。結果の `blob` は呼ぶ側が書く
pub(crate) fn scene(
    drawing: &Drawing2D,
    request: &SceneRequest,
    palette: Palette,
    fonts: &FontSet<'_>,
    cx: &mut Cx<'_>,
) -> Result<(SceneResult, Vec<u8>), Failed> {
    let (origin, unit) = origin(drawing, request, cx)?;
    let region = match request.region.as_deref() {
        Some(&[a, b, c, d]) => Some([a, b, c, d]),
        _ => None,
    };
    let options = Options {
        origin,
        tolerance: request.tolerance,
        print: request.print.unwrap_or(false),
        region,
        palette,
        chunking: Chunking::PROVISIONAL,
    };
    let scene = zumen_scene::build_with(drawing, &options, fonts, cx)
        .map_err(|(stop, at)| fail(cx, stop.to_diagnostic().at(at)))?;
    let counts = scene.counts;
    Ok((
        SceneResult {
            from: request.from.clone(),
            view: request.view,
            unit,
            tolerance: scene.tolerance,
            counts: SceneCounts {
                chunks: counts.chunks,
                lines: counts.lines,
                vertices: counts.vertices,
                faces: counts.faces,
                texts: counts.texts,
                points: counts.points,
                images: counts.images,
                glyphs: counts.glyphs,
            },
            extent: scene.extent.map(|[min_x, min_y, max_x, max_y]| Extent {
                min_x,
                min_y,
                max_x,
                max_y,
            }),
            blob: 0,
        },
        scene.bytes,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_palettes_shaped_like_profile_vocabularies() {
        let palette = palette(
            r##"{"values":[{"name":"赤","rgb":"#ff0000","sxf":2},{"rgb":"#00FF00","aci":3,"jww":3}]}"##
                .as_bytes(),
        )
        .unwrap();
        let mut expected = Palette::default();
        expected.insert(ColorSystem::Sxf, 2, [255, 0, 0]);
        expected.insert(ColorSystem::Aci, 3, [0, 255, 0]);
        expected.insert(ColorSystem::Jww, 3, [0, 255, 0]);
        assert_eq!(palette, expected);
        assert!(super::palette(br#"{"values":[{"rgb":"red"}]}"#).is_none());
        assert!(super::palette(br##"{"values":[{"rgb":"#000000","aci":-1}]}"##).is_none());
        assert!(super::palette(b"[]").is_none());
    }
}
