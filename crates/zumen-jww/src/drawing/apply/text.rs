//! Jw_cad のデータ形式「CDataMoji」のフィールドを書き戻す。
//! 文字種・字の間隔と、変更していない文字列の符号化は元のまま保つ。

use encoding_rs::SHIFT_JIS;
use zumen_base::Kind;
use zumen_drawing::{
    Affine, Height, HeightMeaning, Horizontal, Length, ModelPoint, PaperLength, Placement, Shape,
    SpaceId, Text, Vertical,
};
use zumen_sdk::{Cx, Error};

use super::super::{SETTING_AT, decode, setting};
use super::{NEAR, Reader};

pub(super) fn count(shape: &Shape<ModelPoint>, cx: &mut Cx) -> Result<(), Error> {
    let Shape::Text(text) = shape else {
        return Ok(());
    };
    let size = (text.content.len() as u64).saturating_add(text.style.len() as u64);
    for (kind, amount) in [
        (Kind::Work, size.div_ceil(1024)),
        (
            Kind::DecodedBytes,
            size.saturating_mul(4).saturating_add(128),
        ),
    ] {
        cx.check(kind, amount).map_err(|stop| {
            cx.report(stop.to_diagnostic());
            Error::Stopped(stop)
        })?;
    }
    Ok(())
}

/// フィールドの配置は公式の jwdatafmt.txt の CDataMoji::Serialize に従う。
/// https://www.jwcad.net/jwdatafmt.txt
pub(in crate::drawing) fn bytes(
    text: &Text<ModelPoint>,
    space: SpaceId,
    scale: f64,
    original: Option<&[u8]>,
    movement: Option<&Affine>,
) -> Option<(Vec<u8>, Text<ModelPoint>)> {
    let Placement::Anchor {
        point,
        horizontal: Horizontal::Left,
        vertical: Vertical::Bottom,
    } = text.placement
    else {
        return None;
    };
    if point.space != space
        || !scale.is_finite()
        || scale <= 0.0
        || text.oblique != 0.0
        || text.backward
        || text.upside_down
        || text.content.starts_with("^@")
        || text.content.contains(['\n', '\r'])
    {
        return None;
    }
    let height = match text.height.value {
        Length::Paper(PaperLength(value)) => value,
        Length::Model(value) if value.space == space => value.value / scale,
        _ => return None,
    };
    let x = point.x / scale;
    let y = point.y / scale;
    let width = height * text.width_factor;
    if ![x, y, height, width, text.rotation]
        .iter()
        .all(|value| value.is_finite())
        || height <= 0.0
        || width <= 0.0
        || ([x, y] == SETTING_AT && setting(&text.content).is_some())
        || !matches!(text.height.meaning, HeightMeaning::Declared(_))
    {
        return None;
    }
    let mut prefix = match original {
        Some(bytes) => bytes.get(..68)?.to_vec(),
        None => vec![0; 68],
    };
    let real = |at: usize| -> Option<f64> {
        Some(f64::from_le_bytes(
            original?.get(at..at + 8)?.try_into().ok()?,
        ))
    };
    let set = |out: &mut [u8], at: usize, value: f64| -> Option<()> {
        out.get_mut(at..at + 8)?
            .copy_from_slice(&value.to_le_bytes());
        Some(())
    };
    if real(0).is_none_or(|old| old * scale != point.x) {
        set(&mut prefix, 0, x)?;
    }
    if real(8).is_none_or(|old| old * scale != point.y) {
        set(&mut prefix, 8, y)?;
    }
    let endpoint = if original.is_none() || (real(16) == real(0) && real(24) == real(8)) {
        // 始点と終点が同じ文字は Jw_cad が表示せず、別名保存で消える（#921）。
        // 字幅は全角1字の用紙幅。半角英数字・半角カナは半分の幅とする（推定）。
        let mut half_cells = 0u64;
        let mut characters = 0u64;
        for ch in text.content.chars() {
            characters += 1;
            half_cells += if ch.is_ascii() || ('\u{ff61}'..='\u{ff9f}').contains(&ch) {
                1
            } else {
                2
            };
        }
        let advance = width * (half_cells as f64 / 2.0)
            + real(52).unwrap_or(0.0) * characters.saturating_sub(1) as f64;
        let angle = text.rotation.to_radians();
        let end = (
            x + advance * libm::cos(angle),
            y + advance * libm::sin(angle),
        );
        if !text.content.is_empty() && (advance <= 0.0 || end == (x, y)) {
            return None;
        }
        Some(end)
    } else if let Some(transform) = movement {
        let (ex, ey) = transform.apply(real(16)? * scale, real(24)? * scale);
        Some((ex / scale, ey / scale))
    } else {
        None
    };
    if let Some((ex, ey)) = endpoint {
        if !ex.is_finite() || !ey.is_finite() {
            return None;
        }
        set(&mut prefix, 16, ex)?;
        set(&mut prefix, 24, ey)?;
    }
    if real(44) != Some(height)
        || real(36)
            .zip(real(44))
            .is_none_or(|(a, b)| a / b != text.width_factor)
    {
        set(&mut prefix, 36, width)?;
    }
    set(&mut prefix, 44, height)?;
    set(&mut prefix, 60, text.rotation)?;
    let mut out = prefix;
    let mut reader = original.map(|bytes| Reader::new(bytes, 68));
    for value in [&text.style, &text.content] {
        let mut unicode = false;
        if let Some(reader) = &mut reader {
            let begin = reader.pos;
            let (was_unicode, raw) = reader.string().ok()?;
            unicode = was_unicode;
            if decode(was_unicode, raw) == *value {
                out.extend_from_slice(original?.get(begin..reader.pos)?);
                continue;
            }
        }
        write_string(&mut out, value, unicode)?;
    }
    let mut normalized = text.clone();
    normalized.height = Height {
        value: Length::Paper(PaperLength(height)),
        meaning: HeightMeaning::Declared("jww.moji.size_y".into()),
    };
    Some((out, normalized))
}

pub(super) fn write_string(out: &mut Vec<u8>, text: &str, unicode: bool) -> Option<()> {
    let (encoded, _, errors) = SHIFT_JIS.encode(text);
    if !unicode && !errors && decode(false, &encoded) == text {
        write_length(out, encoded.len())?;
        out.extend_from_slice(&encoded);
    } else {
        out.extend([0xFF, 0xFE, 0xFF]);
        write_length(out, text.encode_utf16().count())?;
        for unit in text.encode_utf16() {
            out.extend(unit.to_le_bytes());
        }
    }
    Some(())
}

fn write_length(out: &mut Vec<u8>, length: usize) -> Option<()> {
    let length = u32::try_from(length).ok()?;
    if length < 0xFF {
        out.push(length as u8);
    } else if length < 0xFFFE {
        out.push(0xFF);
        out.extend((length as u16).to_le_bytes());
    } else {
        out.extend([0xFF, 0xFF, 0xFF]);
        out.extend(length.to_le_bytes());
    }
    Some(())
}

pub(super) fn near(a: &Text<ModelPoint>, b: &Text<ModelPoint>) -> bool {
    let close = |a: f64, b: f64| (a - b).abs() <= NEAR * a.abs().max(b.abs()).max(1.0);
    let (
        Placement::Anchor {
            point: p,
            horizontal: h,
            vertical: v,
        },
        Placement::Anchor {
            point: q,
            horizontal: j,
            vertical: w,
        },
    ) = (a.placement, b.placement)
    else {
        return false;
    };
    let (Length::Paper(PaperLength(x)), Length::Paper(PaperLength(y))) =
        (a.height.value, b.height.value)
    else {
        return false;
    };
    let turn = (a.rotation - b.rotation).rem_euclid(360.0);
    p.space == q.space
        && close(p.x, q.x)
        && close(p.y, q.y)
        && h == j
        && v == w
        && close(x, y)
        && a.height.meaning == b.height.meaning
        && (close(turn, 0.0) || close(turn, 360.0))
        && close(a.width_factor, b.width_factor)
        && a.oblique == b.oblique
        && a.backward == b.backward
        && a.upside_down == b.upside_down
        && a.content == b.content
        && a.style == b.style
}
