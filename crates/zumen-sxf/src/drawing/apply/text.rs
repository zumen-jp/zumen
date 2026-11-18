//! SXF Ver.3.1 フィーチャ仕様編の文字フィーチャと、附属書 SFC 編の文字列。
//! P21 は同じ引数を STEP AP202 サブセット編の文字の実体に対応させる。

use encoding_rs::SHIFT_JIS;
use zumen_base::{Kind, Ref};
use zumen_drawing::{
    Flow, Font, FontKind, Format, FormattedText, Height, HeightMeaning, Horizontal, Length,
    LineSpacing, Piece, Placement, Rotation, Shape, Span, Vertical, transform,
};
use zumen_sdk::{Cx, Error};

use super::{Args, Arguments, FeatureRecord, work};

pub(in crate::drawing) const NAME: &str = "text_string";

pub(super) struct Prepared<P> {
    pub(super) args: Arguments,
    pub(super) shape: Shape<P>,
    pub(super) font: Option<String>,
}

pub(in crate::drawing) fn count<P>(shape: &Shape<P>, cx: &mut Cx) -> Result<(), Error> {
    let size = match shape {
        Shape::Text(text) => text.content.len().saturating_add(text.style.len()),
        Shape::Formatted(text) => text.content.len().saturating_add(
            text.spans
                .first()
                .and_then(|s| s.format.font.as_ref())
                .map_or(0, |f| f.name.len()),
        ),
        _ => return Ok(()),
    } as u64;
    work(cx, 1 + size, &Ref::default())?;
    cx.check(
        Kind::DecodedBytes,
        size.saturating_mul(16).saturating_add(2048),
    )
    .map_err(|stop| {
        cx.report(stop.to_diagnostic());
        Error::Stopped(stop)
    })
}

pub(super) fn prepare<P: transform::Point + PartialEq>(
    shape: &Shape<P>,
    original: Option<&FeatureRecord>,
) -> Option<Prepared<P>> {
    let (mut text, factor) = match shape {
        Shape::Text(text) => {
            let Placement::Anchor {
                point,
                horizontal,
                vertical,
            } = text.placement
            else {
                return None;
            };
            if text.backward
                || text.upside_down
                || !matches!(
                    text.height.meaning,
                    HeightMeaning::Frame | HeightMeaning::Declared(_)
                )
                || !text.width_factor.is_finite()
                || text.width_factor <= 0.0
            {
                return None;
            }
            let font = (!text.style.is_empty()).then(|| font(&text.style));
            (
                FormattedText {
                    content: text.content.clone(),
                    spans: vec![Span {
                        piece: Piece::Text(text.content.clone()),
                        format: Format {
                            font,
                            oblique: Some(text.oblique),
                            ..Format::default()
                        },
                    }],
                    height: Height {
                        value: text.height.value,
                        meaning: HeightMeaning::Frame,
                    },
                    point,
                    horizontal,
                    vertical,
                    rotation: Rotation::Angle(text.rotation),
                    width: None,
                    frame_height: Some(text.height.value),
                    flow: Flow::LeftToRight,
                    line_spacing: LineSpacing {
                        factor: 1.0,
                        exact: false,
                    },
                    background: None,
                    style: String::new(),
                },
                text.width_factor,
            )
        }
        Shape::Formatted(text) if text.spans.len() == 1 => (text.clone(), 1.0),
        _ => return None,
    };
    let [span] = text.spans.as_slice() else {
        return None;
    };
    let fmt = &span.format;
    let name = fmt.font.as_ref().map(|font| font.name.clone());
    if span.piece != Piece::Text(text.content.clone())
        || *fmt
            != (Format {
                font: fmt.font.clone(),
                oblique: fmt.oblique,
                ..Format::default()
            })
        || fmt.font.as_ref().is_some_and(|f| *f != font(&f.name))
        || text.width.is_some()
        || text.frame_height.is_some_and(|h| {
            h != text.height.value
                && original.and_then(|r| Args { values: &r.args }.num(6)) != Some(length(h))
        })
        || text.background.is_some()
        || text.line_spacing
            != (LineSpacing {
                factor: 1.0,
                exact: false,
            })
        || !text.style.is_empty()
        || !matches!(
            text.height.meaning,
            HeightMeaning::Frame | HeightMeaning::Declared(_)
        )
        || !valid_string(&text.content)
    {
        return None;
    }
    let height = length(text.height.value);
    let slant = fmt
        .oblique
        .or_else(|| original.and_then(|r| Args { values: &r.args }.num(10)))
        .unwrap_or(0.0);
    let Rotation::Angle(angle) = text.rotation else {
        return None;
    };
    let [x, y] = text.point.xy();
    let column = match text.horizontal {
        Horizontal::Left => 0,
        Horizontal::Center => 1,
        Horizontal::Right => 2,
        _ => return None,
    };
    let row = match text.vertical {
        Vertical::Bottom => 0,
        Vertical::Middle => 1,
        Vertical::Top => 2,
        _ => return None,
    };
    let direction = match text.flow {
        Flow::LeftToRight => 1,
        Flow::TopToBottom => 2,
        Flow::ByStyle => return None,
    };
    let (width, spacing) = match original {
        Some(original) => {
            let args = Args {
                values: &original.args,
            };
            let old_height = args.num(6).filter(|v| *v > 0.0)?;
            let ratio = height / old_height;
            (args.num(7)? * ratio * factor, args.num(8)? * ratio)
        }
        None => {
            // フォントの字形を得ない幅の見積もり。半角を高さの半分、ほかを高さとし、必ず診断する。
            let cells = text.content.chars().fold(0.0, |sum, ch| {
                sum + if ch.is_ascii() || ('\u{ff61}'..='\u{ff9f}').contains(&ch) {
                    0.5
                } else {
                    1.0
                }
            });
            (height * cells * factor, 0.0)
        }
    };
    if height <= 0.0
        || width < 0.0
        || ![x, y, height, width, spacing, angle, slant]
            .iter()
            .all(|n| n.is_finite())
    {
        return None;
    }
    let values = vec![
        String::new(),
        String::new(),
        String::new(),
        text.content.clone(),
        x.to_string(),
        y.to_string(),
        height.to_string(),
        width.to_string(),
        spacing.to_string(),
        angle.to_string(),
        slant.to_string(),
        (row * 3 + column + 1).to_string(),
        direction.to_string(),
    ];
    text.height.meaning = HeightMeaning::Frame;
    text.frame_height = Some(text.height.value);
    if let Some(span) = text.spans.first_mut() {
        span.format.oblique = (slant != 0.0).then_some(slant);
    }
    Some(Prepared {
        args: Arguments { name: NAME, values },
        shape: Shape::Formatted(text),
        font: name,
    })
}

pub(in crate::drawing) fn font(name: &str) -> Font {
    Font {
        name: name.into(),
        kind: FontKind::Named {
            bold: false,
            italic: false,
            charset: None,
            pitch: None,
        },
    }
}

fn length(value: Length) -> f64 {
    match value {
        Length::Paper(value) => value.0,
        Length::Model(value) => value.value,
    }
}

pub(super) fn valid_string(value: &str) -> bool {
    !value.chars().any(char::is_control)
}

/// SFC の文字列は Shift_JIS と `\'…\'`。2 バイト文字の後半の 0x5c はエスケープしない。
pub(in crate::drawing) fn sfc_string(value: &str) -> Option<Vec<u8>> {
    if !valid_string(value) || value.contains("*/") {
        return None;
    }
    let (encoded, _, errors) = SHIFT_JIS.encode(value);
    if errors || SHIFT_JIS.decode_without_bom_handling(&encoded).0 != value {
        return None;
    }
    let mut out = b"\\'".to_vec();
    let mut bytes = encoded.iter().copied();
    while let Some(byte) = bytes.next() {
        out.push(byte);
        if matches!(byte, 0x81..=0x9f | 0xe0..=0xfc) {
            out.push(bytes.next()?);
        } else if byte == b'\\' {
            out.push(byte);
        }
    }
    out.extend(b"\\'");
    Some(out)
}

pub(in crate::drawing) fn is_string(args: &Arguments, at: usize) -> bool {
    (args.name == NAME && at == 3)
        || (matches!(args.name, "layer" | "text_font") && at == 0)
        || (args.name == "sfig_locate" && at == 1)
}

/// P21 の文字列は ASCII と UTF-16 の X2 エスケープで環境によらず書く。
pub(in crate::drawing) fn p21_string(value: &str) -> Option<String> {
    if !valid_string(value) {
        return None;
    }
    let mut out = String::from("'");
    for ch in value.chars() {
        match ch {
            '\'' => out.push_str("''"),
            '\\' => out.push_str("\\\\"),
            ch if ch.is_ascii() => out.push(ch),
            ch => {
                out.push_str("\\X2\\");
                let mut buf = [0; 2];
                for unit in ch.encode_utf16(&mut buf) {
                    out.push_str(&format!("{unit:04X}"));
                }
                out.push_str("\\X0\\");
            }
        }
    }
    out.push('\'');
    Some(out)
}

pub(super) fn near<P: transform::Point + PartialEq>(
    a: &FormattedText<P>,
    b: &FormattedText<P>,
) -> bool {
    let close = |a: f64, b: f64| (a - b).abs() <= 1e-9 * a.abs().max(b.abs()).max(1.0);
    let (Rotation::Angle(a_angle), Rotation::Angle(b_angle)) = (a.rotation, b.rotation) else {
        return false;
    };
    let turn = (a_angle - b_angle).rem_euclid(360.0);
    let [ax, ay] = a.point.xy();
    let [bx, by] = b.point.xy();
    let mut expected = b.clone();
    expected.point = a.point;
    expected.rotation = a.rotation;
    if let ([a_span], [expected_span]) = (a.spans.as_slice(), expected.spans.as_mut_slice()) {
        if !close(
            a_span.format.oblique.unwrap_or(0.0),
            expected_span.format.oblique.unwrap_or(0.0),
        ) {
            return false;
        }
        expected_span.format.oblique = a_span.format.oblique;
    }
    a.point.at([bx, by]) == b.point
        && close(ax, bx)
        && close(ay, by)
        && (close(turn, 0.0) || close(turn, 360.0))
        && *a == expected
}
