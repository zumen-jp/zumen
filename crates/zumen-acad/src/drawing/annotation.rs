//! DIMENSION と ARC_DIMENSION を寸法に、LEADER を引出線に、MULTILEADER を注記にする。MULTILEADER の線と注記の色・線種・線幅も読む。

use super::lettering::{attachment, flow};
use super::{Lifted, Lifter, Out, aci, columns, invalid, mtext, non_planar, source};
use crate::db::{AcadDb, Data, Object, Origin, Value};
use zumen_base::{Cx, Diagnostic, codes};
use zumen_drawing::{
    Arrowhead, Axis, Background, BackgroundFill, Branch, Callout, CalloutContent, Color, Dimension,
    DimensionKind, DimensionText, Dogleg, FormattedText, Height, HeightMeaning, Horizontal,
    Inherit, Label, Leader, Length, LineSpacing, LineWeight, Measured, PaperLength, PathKind,
    Rotation, Style, Symbol, Vertical,
};

impl Lifter<'_> {
    /// DIMENSION（DXF Reference「DIMENSION」「Common Dimension Group Codes」）。70 の下の 5 ビットが種類で、
    /// 64 は座標寸法の X、128 は利用者が動かした文字。利用者の文字（1）は、ないか空か `<>` なら測定値を、
    /// 空白 1 つなら書かないことを表す。複製の挿入点（12）・行間（41・72）・水平の向き（51）は移さない
    /// （推定。docs/formats/dxf.md）。ARC_DIMENSION（DXF Reference に節がない）は、70 の種類によらず弧長の寸法にし、
    /// 円弧の中心（15）・始めと終わりの点（13・14）を、弧長寸法の欄（`arc_part`）から読む。部分の印（70）と
    /// 角度（40・41）は、始めと終わりの点が部分の両端になるので移さない。16・17 は試料ではいつも 13・14 と同じで、
    /// 意味が分からないので移さない（推定。AutoCAD で作った試料で確かめた。docs/formats/dxf.md）
    pub(super) fn dimension<P: Copy>(
        &self,
        object: &Object,
        point: &impl Fn(f64, f64) -> P,
        length: &impl Fn(f64) -> Length,
    ) -> Lifted<Dimension<P>> {
        let kind = object.kind.as_slice();
        let at = |x: i16| self.planar_point(object, x, point);
        let real = |code: i16, default: Option<f64>| self.real(object, code, default);
        let flags = self.int(object, 70, 0)?;
        let dimension = if kind == b"ARC_DIMENSION" {
            let arc = arc_part(object)?;
            DimensionKind::ArcLength {
                center: self.planar_point(&arc, 15, point)?,
                first: self.planar_point(&arc, 13, point)?,
                second: self.planar_point(&arc, 14, point)?,
                arc: at(10)?,
            }
        } else {
            match flags & 0b1_1111 {
                0 => DimensionKind::Linear {
                    first: at(13)?,
                    second: at(14)?,
                    line: at(10)?,
                    rotation: real(50, Some(0.0))?,
                    oblique: real(52, Some(0.0))?,
                },
                1 => DimensionKind::Aligned {
                    first: at(13)?,
                    second: at(14)?,
                    line: at(10)?,
                    oblique: real(52, Some(0.0))?,
                },
                2 => DimensionKind::AngularLines {
                    first: [at(13)?, at(14)?],
                    second: [at(15)?, at(10)?],
                    arc: at(16)?,
                },
                3 => DimensionKind::Diameter {
                    chord: at(15)?,
                    opposite: at(10)?,
                    leader: length(real(40, None)?),
                },
                4 => DimensionKind::Radius {
                    center: at(10)?,
                    chord: at(15)?,
                    leader: length(real(40, None)?),
                },
                5 => DimensionKind::Angular {
                    vertex: at(15)?,
                    first: at(13)?,
                    second: at(14)?,
                    arc: at(10)?,
                },
                6 => DimensionKind::Ordinate {
                    origin: at(10)?,
                    feature: at(13)?,
                    leader_end: at(14)?,
                    axis: if flags & 64 != 0 { Axis::X } else { Axis::Y },
                },
                _ => return Err(invalid(kind, 70)),
            }
        };
        let label = match self.string(object, 1)?.as_deref() {
            None | Some("") | Some("<>") => Label::Measurement,
            Some(" ") => Label::Hidden,
            Some(text) => Label::Override {
                spans: mtext::dimension_spans(text, length),
                content: text.to_owned(),
            },
        };
        // 71 は 1〜9 で、上・中・下の段ごとに左・中央・右
        let attachment = match object.first(71) {
            None => None,
            Some(_) => match self.int(object, 71, 0)? {
                at @ 1..=9 => Some((
                    match (at - 1) % 3 {
                        0 => Horizontal::Left,
                        1 => Horizontal::Center,
                        _ => Horizontal::Right,
                    },
                    match (at - 1) / 3 {
                        0 => Vertical::Top,
                        1 => Vertical::Middle,
                        _ => Vertical::Bottom,
                    },
                )),
                _ => return Err(invalid(kind, 71)),
            },
        };
        let position = match object.first(11) {
            Some(_) => Some(at(11)?),
            None => None,
        };
        let rotation = match object.first(53) {
            Some(_) => Some(real(53, None)?),
            None => None,
        };
        Ok(Dimension {
            kind: dimension,
            measurement: match object.first(42) {
                Some(_) => Some(Measured {
                    value: real(42, None)?,
                    key: "dxf.DIMENSION.42".into(),
                }),
                None => None,
            },
            label,
            text: DimensionText {
                position,
                moved: flags & 128 != 0,
                rotation,
                attachment,
            },
            style: self
                .string(object, 3)?
                .unwrap_or_else(|| "STANDARD".to_owned()),
            block: self.string(object, 2)?,
        })
    }

    /// 弧長寸法の引出線を持つか（ARC_DIMENSION の 100 の AcDbArcDimension の後の 71）
    pub(super) fn arc_leader(&self, object: &Object) -> Lifted<bool> {
        Ok(self.int(&arc_part(object)?, 71, 0)? != 0)
    }

    /// LEADER（DXF Reference「LEADER」）。頂点（10）は数（76）と合わなければ読めないとする。矢印（71）がなければ
    /// 寸法スタイルに従うとして `None` にする。注記（340）は、指すオブジェクトの出所にする。鉤の線（74・75）・
    /// 注記の種類と大きさ（73・40・41）・ずれ（212・213）は移さない（推定。docs/formats/dxf.md）
    pub(super) fn leader<P: Copy>(
        &self,
        db: &AcadDb,
        object: &Object,
        point: &impl Fn(f64, f64) -> P,
    ) -> Lifted<Leader<P>> {
        let kind = object.kind.as_slice();
        let mut path = Vec::new();
        let mut current: Option<[Option<f64>; 3]> = None;
        let finish = |current: Option<[Option<f64>; 3]>, path: &mut Vec<P>| -> Lifted<()> {
            if let Some([x, y, z]) = current {
                let (Some(x), Some(y)) = (x, y) else {
                    return Err(invalid(kind, 20));
                };
                if z.unwrap_or(0.0) != 0.0 {
                    return Err(non_planar(kind));
                }
                path.push(point(x, y));
            }
            Ok(())
        };
        for value in &object.values {
            let slot = match value.code {
                10 => {
                    finish(current.take(), &mut path)?;
                    0
                }
                20 => 1,
                30 => 2,
                _ => continue,
            };
            let Data::Real(number) = value.data else {
                return Err(invalid(kind, value.code));
            };
            let slot = current.get_or_insert([None; 3]).get_mut(slot);
            match slot {
                Some(slot @ None) if number.is_finite() => *slot = Some(number),
                _ => return Err(invalid(kind, value.code)),
            }
        }
        finish(current, &mut path)?;
        let count = self.int(object, 76, -1)?;
        if path.len() < 2 || usize::try_from(count).ok() != Some(path.len()) {
            return Err(invalid(kind, 76));
        }
        let annotation = match object.first(340) {
            Some(Data::Handle(handle)) => {
                Some(source(db.get(*handle).ok_or_else(|| invalid(kind, 340))?))
            }
            Some(_) => return Err(invalid(kind, 340)),
            None => None,
        };
        let arrow = match object.first(71) {
            Some(_) => Some(self.int(object, 71, 0)? != 0),
            None => None,
        };
        Ok(Leader {
            path,
            spline: match self.int(object, 72, 0)? {
                0 => false,
                1 => true,
                _ => return Err(invalid(kind, 72)),
            },
            arrow,
            annotation,
            style: self
                .string(object, 3)?
                .unwrap_or_else(|| "STANDARD".to_owned()),
        })
    }

    /// 平面の点（`x` と `x + 10`）。Z（`x + 20`）がなければ 0 とし、0 でなければ移さない
    /// MULTILEADER（DXF Reference「MLEADER」）。値を、注記の文脈（300〜301）・枝（302〜303）・線（304〜305）の
    /// 区切りで分けてから読む。注記は、文脈の 290 が真なら書式つきの文字、296 が真なら記号とする。公差の注記
    /// （172 = 3）と段組み（173）は、まだ移さない。線と注記の色・線種・線幅は `callout_style` で読む。
    /// 線ごとの上書きと、寄せ方の欄は、まだ移さない（docs/formats/dxf.md）
    pub(super) fn callout<P: Copy>(
        &self,
        db: &AcadDb,
        object: &Object,
        point: &impl Fn(f64, f64) -> P,
        length: &impl Fn(f64) -> Length,
    ) -> Lifted<Callout<P>> {
        let kind = object.kind.as_slice();
        let parts = split_callout(object)?;
        let part = |values: Vec<Value>| Object {
            values,
            ..object.clone()
        };
        let entity = part(parts.entity);
        let context = part(parts.context);
        if self.int(&entity, 172, 0)? == 3 {
            return Err(invalid(kind, 172));
        }
        let content = match (
            self.flag(&context, 290, false)?,
            self.flag(&context, 296, false)?,
        ) {
            (false, false) => CalloutContent::None,
            (true, false) => {
                CalloutContent::Text(Box::new(self.callout_text(db, &context, point, length)?))
            }
            (false, true) => CalloutContent::Symbol {
                symbol: Box::new(self.callout_symbol(db, &context, point)?),
                values: self.attribute_values(db, &entity)?,
            },
            (true, true) => return Err(invalid(kind, 296)),
        };
        let doglegs = self.flag(&entity, 291, true)?;
        let mut branches = Vec::new();
        for (leader, lines) in parts.leaders {
            let leader = part(leader);
            let end = if self.flag(&leader, 290, false)? {
                Some(self.planar_point(&leader, 10, point)?)
            } else {
                None
            };
            let dogleg = if doglegs && self.flag(&leader, 291, false)? {
                let direction = [self.real(&leader, 11, None)?, self.real(&leader, 21, None)?];
                if direction == [0.0, 0.0] || self.real(&leader, 31, Some(0.0))? != 0.0 {
                    return Err(invalid(kind, 11));
                }
                Some(Dogleg {
                    direction,
                    length: length(self.real(&leader, 40, None)?),
                })
            } else {
                None
            };
            let lines = lines
                .iter()
                .map(|line| self.vertices(kind, line, point))
                .collect::<Lifted<Vec<_>>>()?;
            branches.push(Branch { lines, end, dogleg });
        }
        let path = match self.int(&entity, 170, 1)? {
            0 => PathKind::Hidden,
            1 => PathKind::Straight,
            2 => PathKind::Spline,
            _ => return Err(invalid(kind, 170)),
        };
        // 端部記号の大きさは図形の 42 にあり、なければ文脈の 140 とする
        let arrow_size = match entity.first(42) {
            Some(_) => self.real(&entity, 42, None)?,
            None => self.real(&context, 140, None)?,
        };
        let scale = self.real(&context, 40, Some(1.0))?;
        if scale <= 0.0 || arrow_size < 0.0 {
            return Err(invalid(kind, if scale <= 0.0 { 40 } else { 42 }));
        }
        let style = match entity.first(340) {
            None => None,
            Some(Data::Handle(handle)) => {
                Some(source(db.get(*handle).ok_or_else(|| invalid(kind, 340))?))
            }
            Some(_) => return Err(invalid(kind, 340)),
        };
        Ok(Callout {
            branches,
            content,
            path,
            arrowhead: self
                .block_key(db, &entity, 342)?
                .map_or(Arrowhead::Default, Arrowhead::Symbol),
            arrow_size: length(arrow_size),
            landing: self.flag(&entity, 290, true)?,
            scale,
            // 線と注記の色・線種・線幅は、表の線種を引くので `element` で読む
            line_style: Style::BLOCK,
            content_color: Inherit::Block,
            style,
        })
    }

    /// MULTILEADER の線の色（91）・線種（341）・線幅（171）と、注記の色（文字は 92、記号は 93）（DXF
    /// Reference「MLEADER」）。値がなければ、注記の要素に従う（推定。docs/formats/dxf.md）。線ごとの上書き
    /// （線の 93 が 0 でないか、線の色（92）が注記の要素に従わない）は移さず、診断する
    pub(super) fn callout_style(
        &self,
        db: &AcadDb,
        object: &Object,
        symbol: bool,
        out: &mut Out,
        cx: &mut Cx,
    ) -> Lifted<(Style, Inherit<Color>)> {
        let kind = object.kind.as_slice();
        let parts = split_callout(object)?;
        // DXF の AcDbEntity の欄は、代わりの図形の大きさ（92）を持つ。MULTILEADER の欄は、最後の区切り（100）の後から読む
        let start = parts
            .entity
            .iter()
            .rposition(|value| value.code == 100)
            .map_or(0, |at| at + 1);
        let entity = Object {
            values: parts.entity.get(start..).unwrap_or_default().to_vec(),
            ..object.clone()
        };
        let line_type = match entity.first(341) {
            None | Some(Data::Handle(0)) => Inherit::Block,
            Some(Data::Handle(handle)) => {
                let record = db
                    .get(*handle)
                    .filter(|record| record.kind == b"LTYPE")
                    .ok_or_else(|| invalid(kind, 341))?;
                match self.string(record, 2)? {
                    Some(name) if name.eq_ignore_ascii_case("ByLayer") => Inherit::Layer,
                    Some(name) if name.eq_ignore_ascii_case("ByBlock") => Inherit::Block,
                    _ => Inherit::Own(
                        *out.line_type_handles
                            .get(handle)
                            .ok_or_else(|| invalid(kind, 341))?,
                    ),
                }
            }
            Some(_) => return Err(invalid(kind, 341)),
        };
        let line_weight = match entity.first(171) {
            None | Some(Data::Int(-2)) => Inherit::Block,
            Some(Data::Int(-1)) => Inherit::Layer,
            Some(Data::Int(-3)) => Inherit::Own(LineWeight::Default),
            Some(Data::Int(width @ 0..=211)) => {
                Inherit::Own(LineWeight::Width(PaperLength(*width as f64 / 100.0)))
            }
            Some(_) => return Err(invalid(kind, 171)),
        };
        let line_style = Style {
            color: self.inherited_color(&entity, 91)?,
            line_type,
            line_type_scale: 1.0,
            line_weight: Some(line_weight),
            // 引出線は透過性の欄を持たないので、注記の要素に従う
            transparency: Inherit::Block,
        };
        let content_color = self.inherited_color(&entity, if symbol { 93 } else { 92 })?;
        for line in parts.leaders.iter().flat_map(|(_, lines)| lines) {
            let line = Object {
                values: line.clone(),
                ..object.clone()
            };
            let overridden = match line.first(93) {
                None | Some(Data::Int(0)) => false,
                Some(Data::Int(_)) => true,
                Some(_) => return Err(invalid(kind, 93)),
            };
            if overridden
                || line.first(92).is_some() && self.inherited_color(&line, 92)? != Inherit::Block
            {
                cx.report(Diagnostic::new(codes::DXF_LEADER_LINE_STYLE_DROPPED).at(source(object)));
            }
        }
        Ok((line_style, content_color))
    }

    /// 継承を含む色の値（AcCmColor）。上の 1 バイトが 0xC0 ならレイヤ、0xC1 なら配置から継承する（推定。
    /// docs/formats/dxf.md）。ほかは `raw_color` で読む。なければ配置から継承する
    fn inherited_color(&self, object: &Object, code: i16) -> Lifted<Inherit<Color>> {
        match object.first(code) {
            None => Ok(Inherit::Block),
            Some(Data::Int(value)) => match (*value as u32).to_be_bytes() {
                [0xC0, ..] => Ok(Inherit::Layer),
                [0xC1, ..] => Ok(Inherit::Block),
                _ => Ok(Inherit::Own(self.raw_color(object, code)?)),
            },
            Some(_) => Err(invalid(&object.kind, code)),
        }
    }

    /// MULTILEADER の文脈の書式つきの文字。欄は MTEXT と同じ意味で、符号が違う（DXF Reference「MLEADER」）。
    /// 背景の色（91）は、ほかの色と同じく元の形式の色の値とする
    fn callout_text<P: Copy>(
        &self,
        db: &AcadDb,
        context: &Object,
        point: &impl Fn(f64, f64) -> P,
        length: &impl Fn(f64) -> Length,
    ) -> Lifted<FormattedText<P>> {
        let kind = context.kind.as_slice();
        if self.int(context, 173, 0)? != 0 {
            return Err(columns());
        }
        let content = self
            .string(context, 304)?
            .ok_or_else(|| invalid(kind, 304))?;
        let (horizontal, vertical) = attachment(kind, self.int(context, 171, 1)?, 171)?;
        let direction = [
            self.real(context, 13, Some(0.0))?,
            self.real(context, 23, Some(0.0))?,
        ];
        if self.real(context, 33, Some(0.0))? != 0.0 {
            return Err(non_planar(kind));
        }
        let rotation = if direction == [0.0, 0.0] {
            Rotation::Angle(self.real(context, 42, Some(0.0))?.to_degrees())
        } else {
            Rotation::Direction(direction)
        };
        let frame = |code: i16| -> Lifted<Option<Length>> {
            match self.real(context, code, Some(0.0))? {
                0.0 => Ok(None),
                size if size > 0.0 => Ok(Some(length(size))),
                _ => Err(invalid(kind, code)),
            }
        };
        let background = if self.flag(context, 292, false)? {
            let fill = if self.flag(context, 291, false)? {
                BackgroundFill::Screen
            } else {
                BackgroundFill::Color(self.raw_color(context, 91)?)
            };
            let scale = self.real(context, 141, Some(1.5))?;
            if scale <= 0.0 {
                return Err(invalid(kind, 141));
            }
            let transparency = match context.first(92) {
                None => None,
                Some(Data::Int(value)) => {
                    Some(u32::try_from(*value).map_err(|_| invalid(kind, 92))?)
                }
                Some(_) => return Err(invalid(kind, 92)),
            };
            Some(Background {
                fill: Some(fill),
                frame: false,
                scale,
                transparency,
            })
        } else {
            None
        };
        let style = match context.first(340) {
            None => "STANDARD".to_owned(),
            Some(Data::Handle(handle)) => db
                .get(*handle)
                .filter(|record| record.kind == b"STYLE")
                .map(|record| self.string(record, 2))
                .transpose()?
                .flatten()
                .ok_or_else(|| invalid(kind, 340))?,
            Some(_) => return Err(invalid(kind, 340)),
        };
        let factor = self.real(context, 45, Some(1.0))?;
        if factor <= 0.0 {
            return Err(invalid(kind, 45));
        }
        Ok(FormattedText {
            spans: mtext::spans(&content, length),
            content,
            height: Height {
                value: length(self.real(context, 41, None)?),
                meaning: HeightMeaning::Declared("dxf.MULTILEADER.41".into()),
            },
            point: self.planar_point(context, 12, point)?,
            horizontal,
            vertical,
            rotation,
            width: frame(43)?,
            frame_height: frame(44)?,
            flow: flow(kind, self.int(context, 172, 1)?, 172)?,
            line_spacing: LineSpacing {
                factor,
                exact: match self.int(context, 170, 1)? {
                    1 => false,
                    2 => true,
                    _ => return Err(invalid(kind, 170)),
                },
            },
            background,
            style,
        })
    }

    /// MULTILEADER の文脈の記号。ブロックのレコード（341）を指し、位置（15）・尺度（16・26）・回転（46、
    /// ラジアン）を持つ。変換の行列（47）は、これらから決まる値なので移さない
    fn callout_symbol<P: Copy>(
        &self,
        db: &AcadDb,
        context: &Object,
        point: &impl Fn(f64, f64) -> P,
    ) -> Lifted<Symbol<P>> {
        let kind = context.kind.as_slice();
        let key = self
            .block_key(db, context, 341)?
            .ok_or_else(|| invalid(kind, 341))?;
        let mut scale = [1.0; 2];
        for (code, slot) in [16, 26].into_iter().zip(&mut scale) {
            *slot = self.real(context, code, Some(1.0))?;
            if *slot == 0.0 {
                return Err(invalid(kind, code));
            }
        }
        Ok(Symbol {
            key,
            position: self.planar_point(context, 15, point)?,
            scale,
            rotation: self.real(context, 46, Some(0.0))?.to_degrees(),
            grid: None,
            attributes: Vec::new(),
            clip: None,
        })
    }

    /// 線の頂点（10・20・30）。1 つもなければ読めないとする
    fn vertices<P>(
        &self,
        kind: &[u8],
        values: &[Value],
        point: &impl Fn(f64, f64) -> P,
    ) -> Lifted<Vec<P>> {
        let vertices = self.points(kind, values, 10, point)?;
        if vertices.is_empty() {
            return Err(invalid(kind, 10));
        }
        Ok(vertices)
    }

    /// 元の形式の色の値（AcCmColor）。上の 1 バイトが 0xC2 なら下の 3 バイトが RGB、0xC3 なら下の 1 バイトが
    /// 色の番号（推定。docs/formats/dxf.md）。ほかは読めないとする
    fn raw_color(&self, object: &Object, code: i16) -> Lifted<Color> {
        let value = match object.first(code) {
            Some(Data::Int(value)) => *value as u32,
            _ => return Err(invalid(&object.kind, code)),
        };
        let [method, r, g, b] = value.to_be_bytes();
        match (method, b) {
            (0xC2, _) => Ok(Color {
                code: None,
                rgb: Some([r, g, b]),
            }),
            (0xC3, index @ 1..=255) => Ok(Color {
                code: Some(aci(i64::from(index))),
                rgb: None,
            }),
            _ => Err(invalid(&object.kind, code)),
        }
    }
}

/// MULTILEADER の値の区切り
struct CalloutParts {
    /// 図形の値
    entity: Vec<Value>,
    /// 注記の文脈の値
    context: Vec<Value>,
    /// 枝ごとの、枝の値と線ごとの値
    leaders: Vec<(Vec<Value>, Vec<Vec<Value>>)>,
}

/// ARC_DIMENSION の弧長寸法の欄。70・71 が共通の欄と重なるので分けて読む。DXF では 100 の AcDbArcDimension より
/// 後、DWG では共通の欄の後の最初の 13 から後とする（docs/formats/dwg.md）
fn arc_part(object: &Object) -> Lifted<Object> {
    let dwg = matches!(object.origin, Origin::Dwg { .. });
    let start = object
        .values
        .iter()
        .position(|value| match (value.code, &value.data) {
            (13, _) => dwg,
            (100, Data::Text(text)) => !dwg && text == b"AcDbArcDimension",
            _ => false,
        })
        .map(|at| if dwg { at } else { at + 1 })
        .ok_or_else(|| invalid(&object.kind, 100))?;
    let values = object
        .values
        .get(start..)
        .ok_or_else(|| invalid(&object.kind, 100))?;
    Ok(Object {
        values: values.to_vec(),
        ..object.clone()
    })
}

/// MULTILEADER の値を、区切り（300 `CONTEXT_DATA{`・301・302 `LEADER{`・303・304 `LEADER_LINE{`・305）で
/// 分ける。文脈の中の 304 は、`LEADER_LINE{` でなければ文字の内容とする。区切りが閉じなければ読めないとする
fn split_callout(object: &Object) -> Lifted<CalloutParts> {
    let kind = object.kind.as_slice();
    let mut parts = CalloutParts {
        entity: Vec::new(),
        context: Vec::new(),
        leaders: Vec::new(),
    };
    // 0 は図形、1 は文脈、2 は枝、3 は線の中
    let mut depth = 0;
    let mut seen = false;
    for value in &object.values {
        let marker = |text: &[u8]| matches!(&value.data, Data::Text(t) if t.as_slice() == text);
        match (depth, value.code) {
            (0, 300) if !seen => {
                depth = 1;
                seen = true;
            }
            (1, 301) => depth = 0,
            (1, 302) if marker(b"LEADER{") => {
                parts.leaders.push((Vec::new(), Vec::new()));
                depth = 2;
            }
            (2, 303) => depth = 1,
            (2, 304) if marker(b"LEADER_LINE{") => {
                if let Some((_, lines)) = parts.leaders.last_mut() {
                    lines.push(Vec::new());
                }
                depth = 3;
            }
            (3, 305) => depth = 2,
            (0, _) => parts.entity.push(value.clone()),
            (1, _) => parts.context.push(value.clone()),
            (2, _) => {
                if let Some((leader, _)) = parts.leaders.last_mut() {
                    leader.push(value.clone());
                }
            }
            _ => {
                if let Some(line) = parts.leaders.last_mut().and_then(|(_, l)| l.last_mut()) {
                    line.push(value.clone());
                }
            }
        }
    }
    match (seen, depth) {
        (false, _) => Err(invalid(kind, 300)),
        (true, 0) => Ok(parts),
        (true, _) => Err(invalid(kind, 301)),
    }
}
