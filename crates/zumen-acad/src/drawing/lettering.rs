//! TEXT の 1 行の文字と、MTEXT の書式のある文字（背景の塗りを含む）を、図面モデルの文字にする。
//! 枠への位置合わせと書き進める向きは、表のセルと MULTILEADER の文字も使う。

use super::{Lifted, Lifter, aci, columns, invalid, mtext, non_planar};
use crate::db::{Data, Object};
use zumen_drawing::{
    Background, BackgroundFill, Color, Fit, Flow, FormattedText, Height, HeightMeaning, Horizontal,
    Length, LineSpacing, Placement, Rotation, Text, Vertical,
};

impl Lifter<'_> {
    /// TEXT と ATTRIB の文字。縦の位置合わせは、TEXT では 73、ATTRIB では 74 にある。`key` は高さの欄
    pub(super) fn text<P: Copy>(
        &self,
        object: &Object,
        point: &impl Fn(f64, f64) -> P,
        length: &impl Fn(f64) -> Length,
        vertical: i16,
        key: &'static str,
    ) -> Lifted<Text<P>> {
        let kind = object.kind.as_slice();
        let vertical_code = vertical;
        let at = |x: i16| -> Lifted<P> {
            if self.real(object, x + 20, Some(0.0))? != 0.0 {
                return Err(non_planar(kind));
            }
            Ok(point(
                self.real(object, x, None)?,
                self.real(object, x + 10, None)?,
            ))
        };
        let first = at(10)?;
        let horizontal = self.int(object, 72, 0)?;
        let vertical = self.int(object, vertical_code, 0)?;
        let placement = match (horizontal, vertical) {
            (0, 0) => Placement::Anchor {
                point: first,
                horizontal: Horizontal::Left,
                vertical: Vertical::Baseline,
            },
            (3, _) | (5, _) => Placement::Between {
                first,
                second: at(11)?,
                fit: if horizontal == 3 {
                    Fit::Aligned
                } else {
                    Fit::Fit
                },
            },
            _ => Placement::Anchor {
                point: at(11)?,
                horizontal: match horizontal {
                    0 => Horizontal::Left,
                    1 => Horizontal::Center,
                    2 => Horizontal::Right,
                    4 => Horizontal::Middle,
                    _ => return Err(invalid(kind, 72)),
                },
                vertical: match vertical {
                    0 => Vertical::Baseline,
                    1 => Vertical::Bottom,
                    2 => Vertical::Middle,
                    3 => Vertical::Top,
                    _ => return Err(invalid(kind, vertical_code)),
                },
            },
        };
        let flags = self.int(object, 71, 0)?;
        Ok(Text {
            content: self.string(object, 1)?.ok_or_else(|| invalid(kind, 1))?,
            height: Height {
                value: length(self.real(object, 40, None)?),
                meaning: HeightMeaning::Declared(key),
            },
            placement,
            rotation: self.real(object, 50, Some(0.0))?,
            width_factor: self.real(object, 41, Some(1.0))?,
            oblique: self.real(object, 51, Some(0.0))?,
            backward: flags & 2 != 0,
            upside_down: flags & 4 != 0,
            style: self
                .string(object, 7)?
                .unwrap_or_else(|| "STANDARD".to_owned()),
        })
    }

    /// MTEXT（DXF Reference「MTEXT」）。内容は、250 バイトずつの 3 の後に、残りの 1 を続けたもの。向きは X 軸の
    /// 向き（11・21）があればそれを、なければ回転（50、ラジアン）を移す。枠の幅（41）と高さ（46）は 0 なら
    /// 持たない。文字の範囲の大きさ（42・43）は内容から決まる値なので移さない。段組み（75）は、まだ移さない
    pub(super) fn formatted<P: Copy>(
        &self,
        object: &Object,
        point: &impl Fn(f64, f64) -> P,
        length: &impl Fn(f64) -> Length,
    ) -> Lifted<FormattedText<P>> {
        let kind = object.kind.as_slice();
        if self.int(object, 75, 0)? != 0 {
            return Err(columns());
        }
        let mut content = String::new();
        let mut ended = false;
        for value in object.values.iter().filter(|v| matches!(v.code, 1 | 3)) {
            if ended {
                return Err(invalid(kind, value.code));
            }
            content.push_str(&self.decode(kind, value.code, &value.data)?);
            ended = value.code == 1;
        }
        if !ended {
            return Err(invalid(kind, 1));
        }
        let (horizontal, vertical) = attachment(kind, self.int(object, 71, 1)?, 71)?;
        let rotation = if object.first(11).is_some() {
            if self.real(object, 31, Some(0.0))? != 0.0 {
                return Err(non_planar(kind));
            }
            let direction = [
                self.real(object, 11, None)?,
                self.real(object, 21, Some(0.0))?,
            ];
            if direction == [0.0, 0.0] {
                return Err(invalid(kind, 11));
            }
            Rotation::Direction(direction)
        } else {
            Rotation::Angle(self.real(object, 50, Some(0.0))?.to_degrees())
        };
        let frame = |code: i16| -> Lifted<Option<Length>> {
            match self.real(object, code, Some(0.0))? {
                0.0 => Ok(None),
                size if size > 0.0 => Ok(Some(length(size))),
                _ => Err(invalid(kind, code)),
            }
        };
        let flow = flow(kind, self.int(object, 72, 1)?, 72)?;
        let exact = match self.int(object, 73, 1)? {
            1 => false,
            2 => true,
            _ => return Err(invalid(kind, 73)),
        };
        let factor = self.real(object, 44, Some(1.0))?;
        if factor <= 0.0 {
            return Err(invalid(kind, 44));
        }
        Ok(FormattedText {
            spans: mtext::spans(&content, length),
            content,
            height: Height {
                value: length(self.real(object, 40, None)?),
                meaning: HeightMeaning::Declared("dxf.MTEXT.40"),
            },
            point: self.planar_point(object, 10, point)?,
            horizontal,
            vertical,
            rotation,
            width: frame(41)?,
            frame_height: frame(46)?,
            flow,
            line_spacing: LineSpacing { factor, exact },
            background: self.background(object)?,
            style: self
                .string(object, 7)?
                .unwrap_or_else(|| "STANDARD".to_owned()),
        })
    }

    /// MTEXT の背景（90）。1 は色（63・421）で、2 は表示の背景の色で塗り、16 は枠の線を描く。倍率（45）の
    /// 既定は 1.5 とする（推定。docs/formats/dxf.md）
    fn background(&self, object: &Object) -> Lifted<Option<Background>> {
        let kind = object.kind.as_slice();
        let flags = self.int(object, 90, 0)?;
        if flags & !(1 | 2 | 16) != 0 {
            return Err(invalid(kind, 90));
        }
        if flags == 0 {
            return Ok(None);
        }
        let fill = if flags & 2 != 0 {
            Some(BackgroundFill::Screen)
        } else if flags & 1 != 0 {
            let code = match self.int(object, 63, 0)? {
                0 => None,
                index @ 1..=255 => Some(aci(index)),
                _ => return Err(invalid(kind, 63)),
            };
            let rgb = self.rgb(object, 421)?;
            if code.is_none() && rgb.is_none() {
                return Err(invalid(kind, 63));
            }
            Some(BackgroundFill::Color(Color { code, rgb }))
        } else {
            None
        };
        let scale = self.real(object, 45, Some(1.5))?;
        if scale <= 0.0 {
            return Err(invalid(kind, 45));
        }
        let transparency = match object.first(441) {
            None => None,
            Some(Data::Int(value)) => Some(u32::try_from(*value).map_err(|_| invalid(kind, 441))?),
            Some(_) => return Err(invalid(kind, 441)),
        };
        Ok(Some(Background {
            fill,
            frame: flags & 16 != 0,
            scale,
            transparency,
        }))
    }
}

/// 枠への位置合わせ。1〜9 は、上・中央・下の行の、左・中央・右
pub(super) fn attachment(kind: &[u8], value: i64, code: i16) -> Lifted<(Horizontal, Vertical)> {
    if !(1..=9).contains(&value) {
        return Err(invalid(kind, code));
    }
    let vertical = match (value - 1) / 3 {
        0 => Vertical::Top,
        1 => Vertical::Middle,
        _ => Vertical::Bottom,
    };
    let horizontal = match (value - 1) % 3 {
        0 => Horizontal::Left,
        1 => Horizontal::Center,
        _ => Horizontal::Right,
    };
    Ok((horizontal, vertical))
}

/// 書き進める向き。1 は左から右、3 は上から下、5 は文字スタイルに従う
pub(super) fn flow(kind: &[u8], value: i64, code: i16) -> Lifted<Flow> {
    match value {
        1 => Ok(Flow::LeftToRight),
        3 => Ok(Flow::TopToBottom),
        5 => Ok(Flow::ByStyle),
        _ => Err(invalid(kind, code)),
    }
}
