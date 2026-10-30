//! 画層・線種・寸法スタイル・文字スタイルの表のレコードと、図形の画層・色・線種・線幅を移す
//! （決定 0141・0143・0157・0158）。

use super::{Lifted, Lifter, Out, aci, invalid, true_type_face};
use crate::db::{AcadDb, Data, Object};
use zumen_base::{Cx, Diagnostic, Ref, codes};
use zumen_drawing::{
    Arrowhead, Color, DimensionStyle, Font, FontKind, Inherit, Layer, LayerId, LayerState,
    LayerStyle, LineType, LineTypeDefinition, LineTypeId, LineTypeMark, LineWeight, MarkContent,
    MarkRotation, PaperLength, PatternBasis, Style, TextStyle, TextStyleDefinition,
};

impl Lifter<'_> {
    /// 図形の画層（8）。表にない名前なら、定義のない画層として足す
    pub(super) fn layer(&self, object: &Object, out: &mut Out) -> Lifted<LayerId> {
        let name = self
            .string(object, 8)?
            .ok_or_else(|| invalid(&object.kind, 8))?;
        if let Some(id) = out.by_name.get(&name.to_ascii_lowercase()) {
            return Ok(*id);
        }
        self.add_layer(&object.kind, name, None, Ref::default(), out)
    }

    /// 図形の色（62・420）・線種（6）・線種の倍率（48）・線幅（370）（DXF Reference「Common Group Codes for
    /// Entities」）
    pub(super) fn style(&self, object: &Object, out: &mut Out) -> Lifted<Style> {
        let kind = object.kind.as_slice();
        let rgb = self.rgb(object, 420)?;
        let color = match (self.int(object, 62, 256)?, rgb) {
            (index @ 1..=255, _) => Inherit::Own(Color {
                code: Some(aci(index)),
                rgb,
            }),
            (_, Some(rgb)) => Inherit::Own(Color {
                code: None,
                rgb: Some(rgb),
            }),
            (0, None) => Inherit::Block,
            (256, None) => Inherit::Layer,
            _ => return Err(invalid(kind, 62)),
        };
        let line_type = match self.string(object, 6)? {
            None => Inherit::Layer,
            Some(name) if name.eq_ignore_ascii_case("ByLayer") => Inherit::Layer,
            Some(name) if name.eq_ignore_ascii_case("ByBlock") => Inherit::Block,
            Some(name) => Inherit::Own(self.line_type(kind, name, out)?),
        };
        let line_type_scale = self.real(object, 48, Some(1.0))?;
        if line_type_scale <= 0.0 {
            return Err(invalid(kind, 48));
        }
        let line_weight = match object.first(370) {
            None | Some(Data::Int(-1)) => Some(Inherit::Layer),
            Some(Data::Int(-2)) => Some(Inherit::Block),
            Some(_) => Some(Inherit::Own(self.line_weight(object)?)),
        };
        Ok(Style {
            color,
            line_type,
            line_type_scale,
            line_weight,
        })
    }

    /// 真の色（420、背景は 421）。0x00RRGGBB
    pub(super) fn rgb(&self, object: &Object, code: i16) -> Lifted<Option<[u8; 3]>> {
        match object.first(code) {
            None => Ok(None),
            Some(Data::Int(value @ 0..=0xFF_FFFF)) => {
                let [_, r, g, b] = (*value as u32).to_be_bytes();
                Ok(Some([r, g, b]))
            }
            Some(_) => Err(invalid(&object.kind, code)),
        }
    }

    /// 線幅（370）。-3 は文書の既定、0〜211 は 1/100 mm
    fn line_weight(&self, object: &Object) -> Lifted<LineWeight> {
        match object.first(370) {
            Some(Data::Int(-3)) => Ok(LineWeight::Default),
            Some(Data::Int(width @ 0..=211)) => {
                Ok(LineWeight::Width(PaperLength(*width as f64 / 100.0)))
            }
            _ => Err(invalid(&object.kind, 370)),
        }
    }

    /// 名前の線種。表になければ、定義のない線種として足す
    fn line_type(&self, kind: &[u8], name: String, out: &mut Out) -> Lifted<LineTypeId> {
        match out.line_type_names.get(&name.to_ascii_lowercase()) {
            Some(id) => Ok(*id),
            None => self.add_line_type(kind, name, None, Ref::default(), out),
        }
    }

    /// 線種の表のレコード（DXF Reference「LTYPE」）。線と間の長さ（49）の列を移す。形か文字を持つ要素
    /// （74 が 0 でない）は、長さだけを移して形と文字を落とし、診断する
    pub(super) fn line_type_record(
        &self,
        db: &AcadDb,
        object: &Object,
        source: Ref,
        out: &mut Out,
        cx: &mut Cx,
    ) -> Lifted<()> {
        let kind = object.kind.as_slice();
        let name = self.string(object, 2)?.ok_or_else(|| invalid(kind, 2))?;
        let description = self.string(object, 3)?.unwrap_or_default();
        let mut pattern = Vec::new();
        let mut dashes: Vec<Dash> = Vec::new();
        for value in &object.values {
            match (value.code, &value.data, dashes.last_mut()) {
                (49, Data::Real(length), _) if length.is_finite() => {
                    pattern.push(*length);
                    dashes.push(Dash::default());
                }
                (49, _, _) => return Err(invalid(kind, 49)),
                (74, Data::Int(flags), Some(element)) => element.flags = *flags,
                (75, Data::Int(number), Some(element)) => element.number = *number,
                (340, Data::Handle(handle), Some(element)) => element.style = *handle,
                (46, Data::Real(scale), Some(element)) => element.scale = *scale,
                (50, Data::Real(rotation), Some(element)) => element.rotation = *rotation,
                (44, Data::Real(x), Some(element)) => element.offset[0] = *x,
                (45, Data::Real(y), Some(element)) => element.offset[1] = *y,
                (9, data, Some(element)) => element.text = Some(self.decode(kind, 9, data)?),
                (74 | 75 | 340 | 46 | 50 | 44 | 45 | 9, _, _) => {
                    return Err(invalid(kind, value.code));
                }
                _ => {}
            }
        }
        let mut marks = Vec::new();
        let mut unread = false;
        for (index, dash) in dashes.into_iter().enumerate() {
            match self.line_type_mark(db, index, dash) {
                Ok(Some(mark)) => marks.push(mark),
                Ok(None) => {}
                Err(()) => unread = true,
            }
        }
        if unread {
            cx.report(
                Diagnostic::new(codes::DXF_LINE_TYPE_ELEMENT_DROPPED)
                    .with("name", name.clone())
                    .at(source.clone()),
            );
        }
        let definition = LineTypeDefinition {
            description,
            pattern,
            basis: PatternBasis::Element,
            marks,
        };
        let id = self.add_line_type(kind, name, Some(definition), source, out)?;
        if let Some(handle) = object.handle() {
            out.line_type_handles.insert(handle, id);
        }
        Ok(())
    }

    /// 線種の列の要素に添える形か文字（DXF Reference「LTYPE」）。74 の 2 は文字、4 は形、1 は回転を文書の
    /// x 軸から測ること、8 は文字を逆さにしないことを表す（8 は推定。R2013 からの DXF だけが書く）。形も
    /// 文字もなければ `None`。形の番号（75）が 0 の形は、形がないとみなす（推定）。形と文字を兼ねるもの、
    /// 番号が範囲の外のもの、知らない印のもの、有限でない値のものは読めないとする
    fn line_type_mark(
        &self,
        db: &AcadDb,
        index: usize,
        dash: Dash,
    ) -> Result<Option<LineTypeMark>, ()> {
        let Dash {
            flags,
            number,
            style,
            scale,
            rotation,
            offset,
            text,
        } = dash;
        let style = db
            .get(style)
            .filter(|record| style != 0 && record.kind == b"STYLE");
        let content = match flags & 6 {
            0 => return Ok(None),
            2 => MarkContent::Text {
                content: text.ok_or(())?,
                style: style
                    .map(|record| self.string(record, 2))
                    .transpose()
                    .map_err(|_| ())?
                    .flatten(),
            },
            4 if number == 0 => return Ok(None),
            4 => MarkContent::Shape {
                font: style
                    .map(|record| self.string(record, 3))
                    .transpose()
                    .map_err(|_| ())?
                    .flatten()
                    .map(|name| Font {
                        name,
                        kind: FontKind::Shape,
                    }),
                number: u16::try_from(number).map_err(|_| ())?,
            },
            _ => return Err(()),
        };
        if flags & !15 != 0
            || ![scale, rotation, offset[0], offset[1]]
                .iter()
                .all(|v| v.is_finite())
        {
            return Err(());
        }
        Ok(Some(LineTypeMark {
            index,
            content,
            scale,
            rotation,
            rotation_basis: match (flags & 1, flags & 8) {
                (1, _) => MarkRotation::Absolute,
                (_, 8) => MarkRotation::Upright,
                _ => MarkRotation::Relative,
            },
            offset,
        }))
    }

    /// 線種を足す。同じ名前がすでにあれば、名前で引くのは先のものにする
    fn add_line_type(
        &self,
        kind: &[u8],
        name: String,
        definition: Option<LineTypeDefinition>,
        source: Ref,
        out: &mut Out,
    ) -> Lifted<LineTypeId> {
        let id = LineTypeId(u32::try_from(out.line_types.len()).map_err(|_| invalid(kind, 6))?);
        out.line_type_names
            .entry(name.to_ascii_lowercase())
            .or_insert(id);
        out.line_types.push(LineType {
            id,
            name,
            definition,
            source,
        });
        Ok(id)
    }

    /// 画層の表のレコード（DXF Reference「LAYER」）。凍結（70 の 1）か色（62）が負なら表示せず、290 が偽なら
    /// 印刷せず、ロック（70 の 4）なら編集できないとする。凍結は、表示しないこととは別にも持つ（決定 0141）。
    /// 新しいビューポートで凍結（70 の 2）は、これから作るビューのための値なので移さない
    pub(super) fn layer_record(&self, object: &Object, source: Ref, out: &mut Out) -> Lifted<()> {
        let kind = object.kind.as_slice();
        let name = self.string(object, 2)?.ok_or_else(|| invalid(kind, 2))?;
        let flags = self.int(object, 70, 0)?;
        let state = LayerState {
            visible: flags & 1 == 0 && self.int(object, 62, 7)? >= 0,
            frozen: flags & 1 != 0,
            printable: match object.first(290) {
                Some(Data::Bool(printable)) => *printable,
                Some(_) => return Err(invalid(kind, 290)),
                None => true,
            },
            editable: flags & 4 == 0,
        };
        // 色は 62 の絶対値か 420。線種は、DXF では名前、DWG ではレコードのハンドル
        let rgb = self.rgb(object, 420)?;
        let color = match self.int(object, 62, 0)?.unsigned_abs() {
            index @ 1..=255 => Some(Color {
                code: Some(aci(index as i64)),
                rgb,
            }),
            0 => rgb.map(|rgb| Color {
                code: None,
                rgb: Some(rgb),
            }),
            _ => return Err(invalid(kind, 62)),
        };
        let line_type = match object.first(6) {
            None => None,
            Some(Data::Text(_)) => {
                let name = self.string(object, 6)?.unwrap_or_default();
                Some(self.line_type(kind, name, out)?)
            }
            Some(Data::Handle(handle)) => Some(
                *out.line_type_handles
                    .get(handle)
                    .ok_or_else(|| invalid(kind, 6))?,
            ),
            Some(_) => return Err(invalid(kind, 6)),
        };
        let line_weight = match object.first(370) {
            None => None,
            Some(_) => Some(self.line_weight(object)?),
        };
        let style = LayerStyle {
            color,
            line_type,
            line_weight,
        };
        let id = self.add_layer(kind, name, Some((state, style)), source, out)?;
        if let Some(handle) = object.handle() {
            out.by_handle.insert(handle, id);
        }
        Ok(())
    }

    /// 寸法スタイルの表のレコード（DXF Reference「DIMSTYLE」）。端部記号は、ブロックのレコードのハンドル
    /// （DIMBLK の 342、DIMBLK1・DIMBLK2 の 343・344、DIMLDRBLK の 341）を名前にし、0 かなければ既定の矢印と
    /// する。両端を別にする（DIMSAH の 173）のでなければ、両端とも 342 にする。値がなければ DXF の既定の値
    /// （DIMSCALE の 40 は 1、DIMASZ の 41 は 0.18、DIMTSZ の 142 は 0）とする（推定。docs/formats/dxf.md）
    pub(super) fn dimension_style_record(
        &self,
        db: &AcadDb,
        object: &Object,
        source: Ref,
        out: &mut Out,
    ) -> Lifted<()> {
        let kind = object.kind.as_slice();
        let name = self.string(object, 2)?.ok_or_else(|| invalid(kind, 2))?;
        let arrow = |code: i16| -> Lifted<Arrowhead> {
            Ok(self
                .block_key(db, object, code)?
                .map_or(Arrowhead::Default, Arrowhead::Symbol))
        };
        let separate = match object.first(173) {
            None => false,
            Some(Data::Int(flag)) => *flag != 0,
            Some(Data::Bool(flag)) => *flag,
            Some(_) => return Err(invalid(kind, 173)),
        };
        let ends = if separate {
            [arrow(343)?, arrow(344)?]
        } else {
            let both = arrow(342)?;
            [both.clone(), both]
        };
        let non_negative = |code: i16, default: f64| -> Lifted<f64> {
            Some(self.real(object, code, Some(default))?)
                .filter(|value| *value >= 0.0)
                .ok_or_else(|| invalid(kind, code))
        };
        let tick = non_negative(142, 0.0)?;
        out.dimension_styles.push(DimensionStyle {
            name,
            ends,
            leader: arrow(341)?,
            size: non_negative(41, 0.18)?,
            tick: (tick > 0.0).then_some(tick),
            scale: non_negative(40, 1.0)?,
            source,
        });
        Ok(())
    }

    /// 文字スタイルの表のレコード（DXF Reference「STYLE」。決定 0158）。形のファイルのレコード（70 の 1）は
    /// 文字スタイルにしない。主のフォントは 3、大きいフォントは 4 とし、TrueType のファイル（拡張子が
    /// `.ttf`・`.ttc`・`.otf`）と、XDATA の ACAD に書体の名前（1000）を持つものは名前で指すフォントにする。
    /// 1071 は、下の 8 ビットをピッチと系統、次の 8 ビットを文字集合、ビット 24 を斜体、ビット 25 を太字とする
    /// （推定。文字集合とピッチは試料の値で確かめた）。最後に使った高さ（42）は移さない（決定 0157）
    pub(super) fn text_style_record(
        &self,
        object: &Object,
        source: Ref,
        out: &mut Out,
    ) -> Lifted<()> {
        let kind = object.kind.as_slice();
        let flags = self.int(object, 70, 0)?;
        if flags & 1 != 0 {
            return Ok(());
        }
        let name = self.string(object, 2)?.ok_or_else(|| invalid(kind, 2))?;
        let height = self.real(object, 40, Some(0.0))?;
        if height < 0.0 {
            return Err(invalid(kind, 40));
        }
        let width = self.real(object, 41, Some(1.0))?;
        if width <= 0.0 {
            return Err(invalid(kind, 41));
        }
        let generation = self.int(object, 71, 0)?;
        let file = self.string(object, 3)?.unwrap_or_default();
        let big = self.string(object, 4)?.unwrap_or_default();
        let face = true_type_face(object)
            .map(|(face, bits)| Ok::<_, Box<Diagnostic>>((self.decode(kind, 1000, face)?, bits)))
            .transpose()?;
        let true_type = [".ttf", ".ttc", ".otf"].iter().any(|extension| {
            file.len() >= extension.len()
                && file
                    .get(file.len() - extension.len()..)
                    .is_some_and(|tail| tail.eq_ignore_ascii_case(extension))
        });
        let font = match face {
            Some((face, bits)) => Some(Font {
                name: face,
                kind: named_font(bits),
            }),
            None if file.is_empty() => None,
            None if true_type => Some(Font {
                name: file,
                kind: named_font(None),
            }),
            None => Some(Font {
                name: file,
                kind: FontKind::Shape,
            }),
        };
        out.text_styles.push(TextStyle {
            name,
            definition: Some(TextStyleDefinition {
                font,
                big_font: (!big.is_empty()).then_some(Font {
                    name: big,
                    kind: FontKind::Shape,
                }),
                height: (height > 0.0).then_some(height),
                width,
                oblique: self.real(object, 50, Some(0.0))?,
                backward: generation & 2 != 0,
                upside_down: generation & 4 != 0,
                vertical: flags & 4 != 0,
            }),
            source,
        });
        Ok(())
    }

    /// 画層を足す。同じ名前がすでにあれば、名前で引くのは先のものにする
    fn add_layer(
        &self,
        kind: &[u8],
        name: String,
        defined: Option<(LayerState, LayerStyle)>,
        source: Ref,
        out: &mut Out,
    ) -> Lifted<LayerId> {
        let id = LayerId(u32::try_from(out.layers.len()).map_err(|_| invalid(kind, 8))?);
        out.by_name.entry(name.to_ascii_lowercase()).or_insert(id);
        out.layers.push(Layer {
            id,
            // ブロックの中のレイヤ 0 の図形は、配置のレイヤに従う（決定 0143）
            follows_placement: name == "0",
            name,
            state: defined.map(|(state, _)| state),
            style: defined.map(|(_, style)| style),
            source,
        });
        Ok(id)
    }
}

/// 線種の列の 1 つの要素の、形か文字の値（DXF Reference「LTYPE」の 74〜9）
#[derive(Default)]
struct Dash {
    flags: i64,
    number: i64,
    style: u64,
    scale: f64,
    rotation: f64,
    offset: [f64; 2],
    text: Option<String>,
}

/// 1071 の印から、名前で指すフォントの種類を作る
fn named_font(bits: Option<i64>) -> FontKind {
    match bits {
        Some(bits) => FontKind::Named {
            bold: bits & 0x0200_0000 != 0,
            italic: bits & 0x0100_0000 != 0,
            charset: Some(((bits >> 8) & 0xFF) as u32),
            pitch: Some((bits & 0xFF) as u32),
        },
        None => FontKind::Named {
            bold: false,
            italic: false,
            charset: None,
            pitch: None,
        },
    }
}
