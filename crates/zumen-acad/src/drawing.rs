//! 共有のオブジェクトの表から図面モデル（L2）への変換（決定 0022・0023・0105・0106）。いまは ENTITIES の LINE・TEXT・
//! VIEWPORT と、画層と線種の表と、図形の色・線種・線幅と、空間の描く順だけを移す。
//! DXF も DWG も、表を作ってから同じ変換を通す。
//!
//! モデル空間の図形は実寸の空間に、ペーパー空間（67 = 1）の図形とビューポートは 1 枚の用紙に置く。
//! 実寸の単位は見出しの `$INSUNITS` で決め、分からなければ不明とする。用紙の点は `$PLIMMIN` を原点にした mm で、
//! 単位が分からなければ mm に直せないので、ペーパー空間の図形を移さない。
//! 画層は表の順に置き、表にない名前を図形が指せば、定義のない画層として足す。描く順は、モデル空間と
//! ペーパー空間のブロックのレコードを指す SORTENTSTABLE で決め、なければ入力の順とする。
//! 移さない図形と値は、元のレコードを指す診断で返す。診断の code は、表がグループ・コードで値を名付けるので、
//! DWG でも `dxf.` で始まるものを使う。何をどう読んだかの推定は `docs/formats/dxf.md` に記録した。

use std::collections::BTreeMap;

use crate::db::{AcadDb, Data, Object, Origin, Section, Value};
use crate::{DwgFile, DxfFile};
use zumen_base::{Cx, Diagnostic, Kind, Ref, codes};
use zumen_drawing::{
    Affine, Color, ColorCode, ColorSystem, Drawing2D, Element, Fit, Height, HeightMeaning,
    Horizontal, Inherit, Layer, LayerId, LayerState, LayerStyle, Length, LineType,
    LineTypeDefinition, LineTypeId, LineWeight, ModelLength, ModelPoint, ModelSpace, PaperLength,
    PaperPoint, PatternBasis, Placement, Shape, Sheet, SheetId, SheetItem, SpaceId, Style, Text,
    Unit, Vertical, View,
};
use zumen_sdk::{Error, Lift};

/// UTF-8 で書く最初の版（2007）
const UTF8: &[u8] = b"AC1021";

/// 移さない理由。診断は大きいので箱に入れる
type Lifted<T> = Result<T, Box<Diagnostic>>;

const MODEL: SpaceId = SpaceId(0);
const SHEET: SheetId = SheetId(0);

impl Lift<Drawing2D> for DxfFile {
    /// 表を作ってから変換する。型どおりに読めない値は、変換がその値を使うときに診断を返す
    fn lift(&self, cx: &mut Cx) -> Result<Drawing2D, Error> {
        lift(&self.table(cx, false)?, cx)
    }
}

impl Lift<Drawing2D> for DwgFile {
    /// 表を作ってから変換する。表を作るときの診断も返す
    fn lift(&self, cx: &mut Cx) -> Result<Drawing2D, Error> {
        lift(&self.objects(cx)?, cx)
    }
}

fn lift(db: &AcadDb, cx: &mut Cx) -> Result<Drawing2D, Error> {
    let unit = unit(db, cx);
    let limits = |name: &[u8]| {
        db.variable(name)
            .and_then(|values| Some([header_real(values, 10)?, header_real(values, 20)?]))
    };
    let version =
        db.variable(b"$ACADVER")
            .and_then(|values| match values.first().map(|v| &v.data) {
                Some(Data::Text(text)) => Some(text.trim_ascii()),
                _ => None,
            });
    let lifter = Lifter {
        unicode: version.is_some_and(|v| v >= UTF8),
        mm: unit.millimeters(),
        origin: limits(b"$PLIMMIN").unwrap_or_default(),
    };
    let mut out = Out::default();
    // 画層が線種を指すので、線種の表を先に移す
    for kind in [&b"LTYPE"[..], b"LAYER"] {
        for object in db.objects() {
            if object.section != Section::Tables || object.kind != kind {
                continue;
            }
            work(cx, 1)?;
            let source = source(object);
            let lifted = if kind == b"LTYPE" {
                lifter.line_type_record(object, source.clone(), &mut out, cx)
            } else {
                lifter.layer_record(object, source.clone(), &mut out)
            };
            if let Err(diagnostic) = lifted {
                cx.report((*diagnostic).at(source));
            }
        }
    }
    for object in db.objects() {
        if object.section != Section::Entities {
            continue;
        }
        work(cx, 1)?;
        let source = source(object);
        if let Err(diagnostic) = lifter.entity(object, source.clone(), &mut out, cx) {
            cx.report((*diagnostic).at(source));
        }
    }
    let [model_keys, paper_keys] = lifter.sort_keys(db, cx)?;
    // 見出しの $LTSCALE と、1/100 mm の $LWDEFAULT（DXF Reference「HEADER Section Group Codes」）
    let line_type_scale = db
        .variable(b"$LTSCALE")
        .and_then(|values| header_real(values, 40))
        .filter(|scale| scale.is_finite() && *scale > 0.0);
    let default_line_weight = db.variable(b"$LWDEFAULT").and_then(|values| {
        match values.iter().find(|v| v.code == 370)?.data {
            Data::Int(width @ 0..=211) => Some(PaperLength(width as f64 / 100.0)),
            _ => None,
        }
    });
    let mut drawing = Drawing2D {
        layers: out.layers,
        line_types: out.line_types,
        line_type_scale,
        default_line_weight,
        ..Drawing2D::default()
    };
    drawing.spaces.push(ModelSpace {
        id: MODEL,
        unit,
        elements: out.model,
        order: arrange(
            model_keys.as_ref(),
            out.model_handles.into_iter().enumerate().collect(),
            cx,
        ),
    });
    if !out.paper.is_empty() || !out.views.is_empty() {
        let size = match (limits(b"$PLIMMAX"), lifter.mm) {
            (Some([x, y]), Some(mm)) => {
                let [ox, oy] = lifter.origin;
                let size = [(x - ox) * mm, (y - oy) * mm];
                size.iter().all(|s| *s > 0.0).then(|| size.map(PaperLength))
            }
            _ => None,
        };
        let order = arrange(paper_keys.as_ref(), out.sheet, cx);
        drawing.sheets.push(Sheet {
            id: SHEET,
            size,
            elements: out.paper,
            views: out.views,
            order,
        });
    }
    Ok(drawing)
}

/// 決まった作業量を数え、上限を超えたら止める（決定 0010）
fn work(cx: &mut Cx, amount: u64) -> Result<(), Error> {
    cx.check(Kind::Work, amount).map_err(|stop| {
        cx.report(stop.to_diagnostic());
        Error::Stopped(stop)
    })
}

fn source(object: &Object) -> Ref {
    match object.origin {
        Origin::Dxf { record } => Ref::at_record(record as u64),
        Origin::Dwg { offset, end } => Ref::at_bytes(offset as u64, end as u64),
        Origin::New => Ref::default(),
    }
}

/// 空間の SORTENTSTABLE の出所と、図形のハンドルから並べ替えのハンドルへの対応
type SortKeys = (Ref, BTreeMap<u64, u64>);

/// 描く順に並べたもの。`items` は、入力の順のものとハンドルの組。並べ替えのハンドルの小さいものから描き、
/// 表にない図形は自分のハンドルで並べる（DXF Reference「SORTENTSTABLE」）。同じハンドルなら入力の順。
/// 表がなければ入力の順のまま。ハンドルのない図形があれば並べ替えられないので、入力の順のままにして診断する
fn arrange<T>(keys: Option<&SortKeys>, items: Vec<(T, Option<u64>)>, cx: &mut Cx) -> Vec<T> {
    let Some((table, sort)) = keys else {
        return items.into_iter().map(|(item, _)| item).collect();
    };
    if items.iter().any(|(_, handle)| handle.is_none()) {
        cx.report((*invalid(b"SORTENTSTABLE", 331)).at(table.clone()));
        return items.into_iter().map(|(item, _)| item).collect();
    }
    let mut keyed: Vec<(u64, T)> = items
        .into_iter()
        .map(|(item, handle)| {
            let handle = handle.unwrap_or_default();
            (sort.get(&handle).copied().unwrap_or(handle), item)
        })
        .collect();
    keyed.sort_by_key(|(key, _)| *key);
    keyed.into_iter().map(|(_, item)| item).collect()
}

/// 見出しの `$INSUNITS`（DXF Reference「HEADER Section Group Codes」）。長さの単位でなければ不明
fn unit(db: &AcadDb, cx: &mut Cx) -> Unit {
    let code = db
        .variable(b"$INSUNITS")
        .and_then(|values| values.iter().find(|v| v.code == 70))
        .and_then(|value| match value.data {
            Data::Int(code) => Some(code),
            _ => None,
        });
    let unit = match code {
        Some(1) => Unit::Inch,
        Some(2) => Unit::Foot,
        Some(4) => Unit::Millimeter,
        Some(5) => Unit::Centimeter,
        Some(6) => Unit::Meter,
        _ => Unit::Unknown,
    };
    if unit == Unit::Unknown {
        cx.report(Diagnostic::new(codes::DXF_UNITS_MISSING));
    }
    unit
}

fn header_real(values: &[crate::Value], code: i16) -> Option<f64> {
    match values.iter().find(|v| v.code == code)?.data {
        Data::Real(value) => Some(value),
        _ => None,
    }
}

#[derive(Default)]
struct Out {
    model: Vec<Element<ModelPoint>>,
    /// `model` の図形のハンドル
    model_handles: Vec<Option<u64>>,
    paper: Vec<Element<PaperPoint>>,
    views: Vec<View>,
    /// 用紙の上に描くものと、そのハンドル。順序は入力の順
    sheet: Vec<(SheetItem, Option<u64>)>,
    layers: Vec<Layer>,
    /// ASCII の大文字を小文字にした画層の名前。DXF の名前は大文字と小文字を区別しない（推定。docs/formats/dxf.md）
    by_name: BTreeMap<String, LayerId>,
    /// 画層の表のレコードのハンドル
    by_handle: BTreeMap<u64, LayerId>,
    line_types: Vec<LineType>,
    /// ASCII の大文字を小文字にした線種の名前
    line_type_names: BTreeMap<String, LineTypeId>,
    /// 線種の表のレコードのハンドル
    line_type_handles: BTreeMap<u64, LineTypeId>,
}

struct Lifter {
    /// 2007 以降の版なら、文字の値を UTF-8 として読む
    unicode: bool,
    /// 1 単位の mm の数。不明なら `None`
    mm: Option<f64>,
    /// 用紙の原点（`$PLIMMIN`、図面の単位）
    origin: [f64; 2],
}

impl Lifter {
    fn entity(&self, object: &Object, source: Ref, out: &mut Out, cx: &mut Cx) -> Lifted<()> {
        let kind = object.kind.as_slice();
        let paper = self.int(object, 67, 0)? == 1;
        match kind {
            // 69 = 1 は用紙そのものを表すビューポート
            b"VIEWPORT" if paper && self.int(object, 69, 0)? == 1 => {}
            b"VIEWPORT" if paper => {
                let mm = self.paper_mm(kind)?;
                let mut view = self.view(object, mm, source.clone())?;
                // 341 はビューポートで凍結した画層。指す画層がなければ、その指定だけを除く
                for value in object.values.iter().filter(|value| value.code == 341) {
                    let layer = match value.data {
                        Data::Handle(handle) => out.by_handle.get(&handle).copied(),
                        _ => None,
                    };
                    match layer {
                        Some(layer) => view.hidden_layers.push(layer),
                        None => cx.report((*invalid(kind, 341)).at(source.clone())),
                    }
                }
                out.sheet
                    .push((SheetItem::View(out.views.len()), object.handle()));
                out.views.push(view);
            }
            b"LINE" | b"TEXT" if paper => {
                let mm = self.paper_mm(kind)?;
                let [ox, oy] = self.origin;
                let point = |x: f64, y: f64| PaperPoint {
                    sheet: SHEET,
                    x: (x - ox) * mm,
                    y: (y - oy) * mm,
                };
                let length = |value: f64| Length::Paper(PaperLength(value * mm));
                let shape = self.shape(object, point, length)?;
                let style = self.style(object, out)?;
                let layer = self.layer(object, out)?;
                out.sheet
                    .push((SheetItem::Element(out.paper.len()), object.handle()));
                out.paper.push(Element {
                    source,
                    layer,
                    style,
                    shape,
                });
            }
            b"LINE" | b"TEXT" => {
                let point = |x: f64, y: f64| ModelPoint { space: MODEL, x, y };
                let length = |value: f64| {
                    Length::Model(ModelLength {
                        space: MODEL,
                        value,
                    })
                };
                let shape = self.shape(object, point, length)?;
                let style = self.style(object, out)?;
                let layer = self.layer(object, out)?;
                out.model_handles.push(object.handle());
                out.model.push(Element {
                    source,
                    layer,
                    style,
                    shape,
                });
            }
            _ => {
                return Err(Box::new(
                    Diagnostic::new(codes::DXF_LIFT_DROPPED).with("kind", text(kind)),
                ));
            }
        }
        Ok(())
    }

    fn shape<P: Copy>(
        &self,
        object: &Object,
        point: impl Fn(f64, f64) -> P,
        length: impl Fn(f64) -> Length,
    ) -> Lifted<Shape<P>> {
        let kind = object.kind.as_slice();
        self.planar(object)?;
        if kind == b"LINE" {
            // LINE（DXF Reference「LINE」）。Z のないものは 0 とみなす（推定。docs/formats/dxf.md）
            let end = |x: i16| -> Lifted<[f64; 3]> {
                let z = self.real(object, x + 20, Some(0.0))?;
                Ok([
                    self.real(object, x, None)?,
                    self.real(object, x + 10, None)?,
                    z,
                ])
            };
            let (start, end) = (end(10)?, end(11)?);
            if start[2] != 0.0 || end[2] != 0.0 {
                return Err(non_planar(kind));
            }
            return Ok(Shape::Line {
                start: point(start[0], start[1]),
                end: point(end[0], end[1]),
            });
        }
        // TEXT（DXF Reference「TEXT」）
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
        let vertical = self.int(object, 73, 0)?;
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
                    _ => return Err(invalid(kind, 73)),
                },
            },
        };
        let flags = self.int(object, 71, 0)?;
        Ok(Shape::Text(Text {
            content: self.string(object, 1)?.ok_or_else(|| invalid(kind, 1))?,
            height: Height {
                value: length(self.real(object, 40, None)?),
                meaning: HeightMeaning::Declared("dxf.TEXT.40"),
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
        }))
    }

    /// ペーパー空間のビューポート（DXF Reference「VIEWPORT」）。ねじれ・視点の向き・注視点のないものだけを扱う
    fn view(&self, object: &Object, mm: f64, source: Ref) -> Lifted<View> {
        let kind = object.kind.as_slice();
        let real = |code: i16, default: Option<f64>| self.real(object, code, default);
        let unsupported = |code: i16| {
            Box::new(Diagnostic::new(codes::DXF_VIEWPORT_UNSUPPORTED).with("code", i64::from(code)))
        };
        if [
            real(16, Some(0.0))?,
            real(26, Some(0.0))?,
            real(36, Some(1.0))?,
        ] != [0.0, 0.0, 1.0]
        {
            return Err(unsupported(16));
        }
        if [
            real(17, Some(0.0))?,
            real(27, Some(0.0))?,
            real(37, Some(0.0))?,
        ] != [0.0; 3]
        {
            return Err(unsupported(17));
        }
        if real(51, Some(0.0))? != 0.0 {
            return Err(unsupported(51));
        }
        let [px, py] = [real(10, None)?, real(20, None)?];
        let [width, height] = [real(40, None)?, real(41, None)?];
        let [vx, vy] = [real(12, Some(0.0))?, real(22, Some(0.0))?];
        let view_height = real(45, None)?;
        for (code, value) in [(40, width), (41, height), (45, view_height)] {
            if value <= 0.0 {
                return Err(invalid(kind, code));
            }
        }
        // 用紙の mm = mm × (用紙の中心 + 縮尺 × (実寸の点 − ビューの中心) − 用紙の原点)
        let [ox, oy] = self.origin;
        let scale = height / view_height * mm;
        let transform = Affine {
            m: [
                scale,
                0.0,
                (px - ox) * mm - scale * vx,
                0.0,
                scale,
                (py - oy) * mm - scale * vy,
            ],
        };
        let corner = |dx: f64, dy: f64| PaperPoint {
            sheet: SHEET,
            x: (px + dx * width / 2.0 - ox) * mm,
            y: (py + dy * height / 2.0 - oy) * mm,
        };
        Ok(View {
            space: MODEL,
            transform,
            sheet: SHEET,
            clip: vec![
                corner(-1.0, -1.0),
                corner(1.0, -1.0),
                corner(1.0, 1.0),
                corner(-1.0, 1.0),
            ],
            hidden_layers: Vec::new(),
            source,
        })
    }

    /// 押し出しの向き（210・220・230）が Z でなければ、平面に移さない
    fn planar(&self, object: &Object) -> Lifted<()> {
        let extrusion = [
            self.real(object, 210, Some(0.0))?,
            self.real(object, 220, Some(0.0))?,
            self.real(object, 230, Some(1.0))?,
        ];
        if extrusion == [0.0, 0.0, 1.0] {
            Ok(())
        } else {
            Err(non_planar(&object.kind))
        }
    }

    fn paper_mm(&self, kind: &[u8]) -> Lifted<f64> {
        self.mm.ok_or_else(|| {
            Box::new(Diagnostic::new(codes::DXF_PAPER_SPACE_DROPPED).with("kind", text(kind)))
        })
    }

    /// 図形の画層（8）。表にない名前なら、定義のない画層として足す
    fn layer(&self, object: &Object, out: &mut Out) -> Lifted<LayerId> {
        let name = self
            .string(object, 8)?
            .ok_or_else(|| invalid(&object.kind, 8))?;
        if let Some(id) = out.by_name.get(&name.to_ascii_lowercase()) {
            return Ok(*id);
        }
        self.add_layer(&object.kind, name, None, Ref::default(), out)
    }

    /// 図形の色（62・420）・線種（6）・線種の倍率（48）・線幅（370）（DXF Reference「Common Group Codes for
    /// Entities」）。DWG の読み手は図形の線幅を表に置かないので、DWG の図形の線幅は読めていないとする
    fn style(&self, object: &Object, out: &mut Out) -> Lifted<Style> {
        let kind = object.kind.as_slice();
        let rgb = self.rgb(object)?;
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
        let line_weight = match (object.first(370), &object.origin) {
            (None, Origin::Dwg { .. }) => None,
            (None, _) => Some(Inherit::Layer),
            (Some(Data::Int(-1)), _) => Some(Inherit::Layer),
            (Some(Data::Int(-2)), _) => Some(Inherit::Block),
            (Some(_), _) => Some(Inherit::Own(self.line_weight(object)?)),
        };
        Ok(Style {
            color,
            line_type,
            line_type_scale,
            line_weight,
        })
    }

    /// 真の色（420）。0x00RRGGBB
    fn rgb(&self, object: &Object) -> Lifted<Option<[u8; 3]>> {
        match object.first(420) {
            None => Ok(None),
            Some(Data::Int(value @ 0..=0xFF_FFFF)) => {
                let [_, r, g, b] = (*value as u32).to_be_bytes();
                Ok(Some([r, g, b]))
            }
            Some(_) => Err(invalid(&object.kind, 420)),
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
    fn line_type_record(
        &self,
        object: &Object,
        source: Ref,
        out: &mut Out,
        cx: &mut Cx,
    ) -> Lifted<()> {
        let kind = object.kind.as_slice();
        let name = self.string(object, 2)?.ok_or_else(|| invalid(kind, 2))?;
        let description = self.string(object, 3)?.unwrap_or_default();
        let mut pattern = Vec::new();
        let mut complex = false;
        for value in &object.values {
            match (value.code, &value.data) {
                (49, Data::Real(length)) if length.is_finite() => pattern.push(*length),
                (49, _) => return Err(invalid(kind, 49)),
                (74, Data::Int(flags)) => complex |= *flags != 0,
                (74, _) => return Err(invalid(kind, 74)),
                _ => {}
            }
        }
        if complex {
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
        };
        let id = self.add_line_type(kind, name, Some(definition), source, out)?;
        if let Some(handle) = object.handle() {
            out.line_type_handles.insert(handle, id);
        }
        Ok(())
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
    /// 印刷せず、ロック（70 の 4）なら編集できないとする。凍結と非表示の違いは移さない
    fn layer_record(&self, object: &Object, source: Ref, out: &mut Out) -> Lifted<()> {
        let kind = object.kind.as_slice();
        let name = self.string(object, 2)?.ok_or_else(|| invalid(kind, 2))?;
        let flags = self.int(object, 70, 0)?;
        let state = LayerState {
            visible: flags & 1 == 0 && self.int(object, 62, 7)? >= 0,
            printable: match object.first(290) {
                Some(Data::Bool(printable)) => *printable,
                Some(_) => return Err(invalid(kind, 290)),
                None => true,
            },
            editable: flags & 4 == 0,
        };
        // 色は 62 の絶対値か 420。線種は、DXF では名前、DWG ではレコードのハンドル
        let rgb = self.rgb(object)?;
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
            name,
            state: defined.map(|(state, _)| state),
            style: defined.map(|(_, style)| style),
            source,
        });
        Ok(id)
    }

    /// モデル空間とペーパー空間の SORTENTSTABLE。ほかのブロックの表は、そのブロックを移さないので読まない
    fn sort_keys(&self, db: &AcadDb, cx: &mut Cx) -> Result<[Option<SortKeys>; 2], Error> {
        let mut keys = [None, None];
        for object in db.objects() {
            if object.section != Section::Objects || object.kind != b"SORTENTSTABLE" {
                continue;
            }
            work(cx, 1)?;
            let source = source(object);
            match self.sortents(db, object) {
                Ok((space, sort)) => {
                    work(cx, sort.len() as u64)?;
                    if let Some(slot) = space.and_then(|index| keys.get_mut(index)) {
                        let (_, all) = slot.get_or_insert_with(|| (source, BTreeMap::new()));
                        all.extend(sort);
                    }
                }
                Err(diagnostic) => cx.report((*diagnostic).at(source)),
            }
        }
        Ok(keys)
    }

    /// SORTENTSTABLE の、指すブロックのレコード（0 はモデル空間、1 はペーパー空間、ほかは `None`）と、
    /// 図形のハンドル（331）から並べ替えのハンドル（5）への対応（DXF Reference「SORTENTSTABLE」）
    fn sortents(
        &self,
        db: &AcadDb,
        object: &Object,
    ) -> Lifted<(Option<usize>, BTreeMap<u64, u64>)> {
        let kind = object.kind.as_slice();
        let start = object
            .values
            .iter()
            .position(|value| {
                matches!((value.code, &value.data), (100, Data::Text(text)) if text == b"AcDbSortentsTable")
            })
            .ok_or_else(|| invalid(kind, 100))?;
        let mut rest = object.values.iter().skip(start + 1);
        let block = match rest.next() {
            Some(Value {
                code: 330,
                data: Data::Handle(handle),
            }) => db
                .get(*handle)
                .filter(|record| record.kind == b"BLOCK_RECORD")
                .ok_or_else(|| invalid(kind, 330))?,
            _ => return Err(invalid(kind, 330)),
        };
        let name = self.string(block, 2)?.unwrap_or_default();
        let space = ["*Model_Space", "*Paper_Space"]
            .iter()
            .position(|space| name.eq_ignore_ascii_case(space));
        let mut sort = BTreeMap::new();
        while let Some(value) = rest.next() {
            let (331, Data::Handle(entity)) = (value.code, &value.data) else {
                return Err(invalid(kind, value.code));
            };
            let Some(Value {
                code: 5,
                data: Data::Handle(key),
            }) = rest.next()
            else {
                return Err(invalid(kind, 5));
            };
            sort.insert(*entity, *key);
        }
        Ok((space, sort))
    }

    /// 有限の実数の値。なければ `default`、それもなければ読めないとする
    fn real(&self, object: &Object, code: i16, default: Option<f64>) -> Lifted<f64> {
        match object.first(code) {
            Some(Data::Real(value)) => Some(*value).filter(|v| v.is_finite()),
            Some(_) => None,
            None => default,
        }
        .ok_or_else(|| invalid(&object.kind, code))
    }

    /// 整数の値。なければ `default`
    fn int(&self, object: &Object, code: i16, default: i64) -> Lifted<i64> {
        match object.first(code) {
            Some(Data::Int(value)) => Ok(*value),
            Some(_) => Err(invalid(&object.kind, code)),
            None => Ok(default),
        }
    }

    /// 文字の値。2007 より前の版の ASCII でない文字は、コード・ページを読まないので受け付けない
    fn string(&self, object: &Object, code: i16) -> Lifted<Option<String>> {
        let kind = object.kind.as_slice();
        let value = match object.first(code) {
            Some(Data::Text(value)) => value,
            Some(_) => return Err(invalid(kind, code)),
            None => return Ok(None),
        };
        if self.unicode || value.is_ascii() {
            match core::str::from_utf8(value) {
                Ok(value) => Ok(Some(value.to_owned())),
                Err(_) => Err(invalid(kind, code)),
            }
        } else {
            Err(Box::new(
                Diagnostic::new(codes::DXF_CODE_PAGE_UNSUPPORTED).with("kind", text(kind)),
            ))
        }
    }
}

/// AutoCAD の色の番号。1〜255 のものだけを渡す
fn aci(index: i64) -> ColorCode {
    ColorCode {
        system: ColorSystem::Aci,
        value: index as u32,
    }
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

fn invalid(kind: &[u8], code: i16) -> Box<Diagnostic> {
    Box::new(
        Diagnostic::new(codes::DXF_LIFT_INVALID)
            .with("kind", text(kind))
            .with("code", i64::from(code)),
    )
}

fn non_planar(kind: &[u8]) -> Box<Diagnostic> {
    Box::new(Diagnostic::new(codes::DXF_NON_PLANAR_DROPPED).with("kind", text(kind)))
}
