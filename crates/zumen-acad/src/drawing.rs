//! 共有のオブジェクトの表から図面モデル（L2）への変換（決定 0022・0023・0105・0106・0107・0108）。いまは ENTITIES の
//! LINE・TEXT・DIMENSION・LEADER・INSERT（と、その ATTRIB）・HATCH・VIEWPORT と、画層・線種・寸法スタイルの表と、
//! 図形の色・線種・線幅と、空間の描く順だけを移す。
//! DXF も DWG も、表を作ってから同じ変換を通す。
//!
//! モデル空間の図形は実寸の空間に、ペーパー空間（67 = 1）の図形とビューポートは 1 枚の用紙に置く。
//! 実寸の単位は見出しの `$INSUNITS` で決め、分からなければ不明とする。用紙の点は `$PLIMMIN` を原点にした mm で、
//! 単位が分からなければ mm に直せないので、ペーパー空間の図形を移さない。
//! 画層は表の順に置き、表にない名前を図形が指せば、定義のない画層として足す。描く順は、モデル空間と
//! ペーパー空間のブロックのレコードを指す SORTENTSTABLE で決め、なければ入力の順とする。
//! 移さない図形と値は、元のレコードを指す診断で返す。診断の code は、表がグループ・コードで値を名付けるので、
//! DWG でも `dxf.` で始まるものを使う。何をどう読んだかの推定は `docs/formats/dxf.md` に記録した。

use std::collections::{BTreeMap, BTreeSet};

use crate::db::{AcadDb, Data, Object, Origin, Section, Value};
use crate::{DwgFile, DxfFile};
use zumen_base::{Cx, Diagnostic, Kind, Ref, codes};
use zumen_drawing::{
    Affine, Arrowhead, Attribute, Axis, Boundary, Color, ColorCode, ColorSystem, Dimension,
    DimensionKind, DimensionStyle, DimensionText, Drawing2D, Edge, Element, Fill, Fit, Gradient,
    Grid, Hatch, HatchLoop, Height, HeightMeaning, Horizontal, Inherit, Islands, Label, Layer,
    LayerId, LayerState, LayerStyle, Leader, Length, LineType, LineTypeDefinition, LineTypeId,
    LineWeight, Measured, ModelLength, ModelPoint, ModelSpace, PaperLength, PaperPoint, Pattern,
    PatternBasis, PatternLine, PatternOrigin, Placement, Shape, Sheet, SheetId, SheetItem, SpaceId,
    Style, Symbol, SymbolKey, Text, Unit, Vertex, Vertical, View,
};
use zumen_sdk::{Error, Lift};

/// UTF-8 で書く最初の版（2007）
const UTF8: &[u8] = b"AC1021";

/// 移さない理由。診断は大きいので箱に入れる
type Lifted<T> = Result<T, Box<Diagnostic>>;

const MODEL: SpaceId = SpaceId(0);
const SHEET: SheetId = SheetId(0);

/// ブロックの定義を指す記号のキーの名前空間
const BLOCK: &str = "dxf.block";

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
    for kind in [&b"LTYPE"[..], b"LAYER", b"DIMSTYLE"] {
        for object in db.objects() {
            if object.section != Section::Tables || object.kind != kind {
                continue;
            }
            work(cx, 1)?;
            let source = source(object);
            let lifted = match kind {
                b"LTYPE" => lifter.line_type_record(object, source.clone(), &mut out, cx),
                b"LAYER" => lifter.layer_record(object, source.clone(), &mut out),
                _ => lifter.dimension_style_record(db, object, source.clone(), &mut out),
            };
            if let Err(diagnostic) = lifted {
                cx.report((*diagnostic).at(source));
            }
        }
    }
    let entities: Vec<&Object> = db
        .objects()
        .iter()
        .filter(|object| object.section == Section::Entities)
        .collect();
    let (attributes, owned) = attached(&entities);
    for (index, object) in entities.iter().enumerate() {
        work(cx, 1)?;
        if owned.contains(&index) {
            continue;
        }
        let source = source(object);
        let attributes: Vec<&Object> = attributes
            .get(&index)
            .map_or(&[][..], Vec::as_slice)
            .iter()
            .filter_map(|at| entities.get(*at).copied())
            .collect();
        if let Err(diagnostic) =
            lifter.entity(db, object, source.clone(), &attributes, &mut out, cx)
        {
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
        dimension_styles: out.dimension_styles,
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

/// INSERT の属性（ATTRIB）と終わりの印（SEQEND）。所有者（330）が INSERT を指すもの、所有者がないか図形でない
/// ものは、属性が続く（66 = 1）INSERT の後に並ぶものを、その INSERT のものとする（DXF Reference「INSERT」）。
/// INSERT の添字から ATTRIB の添字への対応と、INSERT のものとした添字を返す
fn attached(entities: &[&Object]) -> (BTreeMap<usize, Vec<usize>>, BTreeSet<usize>) {
    let by_handle: BTreeMap<u64, usize> = entities
        .iter()
        .enumerate()
        .filter_map(|(index, object)| Some((object.handle()?, index)))
        .collect();
    let mut attributes: BTreeMap<usize, Vec<usize>> = BTreeMap::new();
    let mut owned = BTreeSet::new();
    let mut open = None;
    for (index, object) in entities.iter().enumerate() {
        match object.kind.as_slice() {
            b"INSERT" => {
                open = matches!(object.first(66), Some(Data::Int(1) | Data::Bool(true)))
                    .then_some(index);
            }
            kind @ (b"ATTRIB" | b"SEQEND") => {
                let parent = object
                    .owner()
                    .and_then(|owner| by_handle.get(&owner).copied())
                    .or(open)
                    .filter(|parent| entities.get(*parent).is_some_and(|o| o.kind == b"INSERT"));
                if let Some(parent) = parent {
                    owned.insert(index);
                    if kind == b"ATTRIB" {
                        attributes.entry(parent).or_default().push(index);
                    }
                }
                if kind == b"SEQEND" {
                    open = None;
                }
            }
            _ => open = None,
        }
    }
    (attributes, owned)
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
    dimension_styles: Vec<DimensionStyle>,
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
    fn entity(
        &self,
        db: &AcadDb,
        object: &Object,
        source: Ref,
        attributes: &[&Object],
        out: &mut Out,
        cx: &mut Cx,
    ) -> Lifted<()> {
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
            b"LINE" | b"TEXT" | b"DIMENSION" | b"LEADER" | b"INSERT" | b"HATCH" if paper => {
                let mm = self.paper_mm(kind)?;
                let [ox, oy] = self.origin;
                let point = |x: f64, y: f64| PaperPoint {
                    sheet: SHEET,
                    x: (x - ox) * mm,
                    y: (y - oy) * mm,
                };
                let length = |value: f64| Length::Paper(PaperLength(value * mm));
                let element =
                    self.element(db, object, source, attributes, point, length, out, cx)?;
                out.sheet
                    .push((SheetItem::Element(out.paper.len()), object.handle()));
                out.paper.push(element);
            }
            b"LINE" | b"TEXT" | b"DIMENSION" | b"LEADER" | b"INSERT" | b"HATCH" => {
                let point = |x: f64, y: f64| ModelPoint { space: MODEL, x, y };
                let length = |value: f64| {
                    Length::Model(ModelLength {
                        space: MODEL,
                        value,
                    })
                };
                let element =
                    self.element(db, object, source, attributes, point, length, out, cx)?;
                out.model_handles.push(object.handle());
                out.model.push(element);
            }
            _ => {
                return Err(Box::new(
                    Diagnostic::new(codes::DXF_LIFT_DROPPED).with("kind", text(kind)),
                ));
            }
        }
        Ok(())
    }

    /// 図形を要素にする。INSERT の属性のうち移せないものは、属性の出所を指す診断を返し、INSERT は移す
    #[allow(clippy::too_many_arguments)]
    fn element<P: Copy>(
        &self,
        db: &AcadDb,
        object: &Object,
        source: Ref,
        attributes: &[&Object],
        point: impl Fn(f64, f64) -> P,
        length: impl Fn(f64) -> Length,
        out: &mut Out,
        cx: &mut Cx,
    ) -> Lifted<Element<P>> {
        let mut shape = self.shape(db, object, &point, &length)?;
        let style = self.style(object, out)?;
        let layer = self.layer(object, out)?;
        if let Shape::Symbol(symbol) = &mut shape {
            for attribute in attributes {
                match self.attribute(attribute, &point, &length, out) {
                    Ok(lifted) => symbol.attributes.push(lifted),
                    Err(diagnostic) => {
                        cx.report((*diagnostic).at(crate::drawing::source(attribute)))
                    }
                }
            }
        }
        Ok(Element {
            source,
            layer,
            style,
            shape,
        })
    }

    fn shape<P: Copy>(
        &self,
        db: &AcadDb,
        object: &Object,
        point: &impl Fn(f64, f64) -> P,
        length: &impl Fn(f64) -> Length,
    ) -> Lifted<Shape<P>> {
        let kind = object.kind.as_slice();
        self.planar(object)?;
        match kind {
            b"DIMENSION" => Ok(Shape::Dimension(self.dimension(object, point, length)?)),
            b"LEADER" => Ok(Shape::Leader(self.leader(db, object, point)?)),
            b"INSERT" => Ok(Shape::Symbol(self.symbol(object, point, length)?)),
            b"HATCH" => Ok(Shape::Hatch(self.hatch(db, object, point, length)?)),
            b"LINE" => self.line(object, point),
            // TEXT（DXF Reference「TEXT」）
            _ => Ok(Shape::Text(self.text(
                object,
                point,
                length,
                73,
                "dxf.TEXT.40",
            )?)),
        }
    }

    /// LINE（DXF Reference「LINE」）。Z のないものは 0 とみなす（推定。docs/formats/dxf.md）
    fn line<P>(&self, object: &Object, point: &impl Fn(f64, f64) -> P) -> Lifted<Shape<P>> {
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
            return Err(non_planar(&object.kind));
        }
        Ok(Shape::Line {
            start: point(start[0], start[1]),
            end: point(end[0], end[1]),
        })
    }

    /// TEXT と ATTRIB の文字。縦の位置合わせは、TEXT では 73、ATTRIB では 74 にある。`key` は高さの欄
    fn text<P: Copy>(
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

    /// INSERT（DXF Reference「INSERT」）。尺度（41・42）が 0 なら読めないとする。Z の尺度（43）は平面の形を
    /// 変えないので移さない。列と行（70・71）が 1 より多ければ、格子に並べる（MINSERT）。属性は後で足す
    fn symbol<P: Copy>(
        &self,
        object: &Object,
        point: &impl Fn(f64, f64) -> P,
        length: &impl Fn(f64) -> Length,
    ) -> Lifted<Symbol<P>> {
        let kind = object.kind.as_slice();
        let name = self.string(object, 2)?.ok_or_else(|| invalid(kind, 2))?;
        let mut scale = [1.0; 2];
        for (code, slot) in [41, 42].into_iter().zip(&mut scale) {
            *slot = self.real(object, code, Some(1.0))?;
            if *slot == 0.0 {
                return Err(invalid(kind, code));
            }
        }
        let count = |code: i16| -> Lifted<u32> {
            u32::try_from(self.int(object, code, 1)?)
                .ok()
                .filter(|count| *count > 0)
                .ok_or_else(|| invalid(kind, code))
        };
        let (columns, rows) = (count(70)?, count(71)?);
        let grid = if (columns, rows) == (1, 1) {
            None
        } else {
            Some(Grid {
                columns,
                rows,
                column_spacing: length(self.real(object, 44, Some(0.0))?),
                row_spacing: length(self.real(object, 45, Some(0.0))?),
            })
        };
        Ok(Symbol {
            key: SymbolKey {
                namespace: BLOCK,
                name,
            },
            position: self.planar_point(object, 10, point)?,
            scale,
            rotation: self.real(object, 50, Some(0.0))?,
            grid,
            attributes: Vec::new(),
        })
    }

    /// ATTRIB（DXF Reference「ATTRIB」）。タグ（2）と、TEXT と同じ欄の値を移し、70 の 1 なら表示しないとする。
    /// 複数行の属性（埋め込みの MTEXT を持つ。101）は、MTEXT をまだ移さないので移さない
    fn attribute<P: Copy>(
        &self,
        object: &Object,
        point: &impl Fn(f64, f64) -> P,
        length: &impl Fn(f64) -> Length,
        out: &mut Out,
    ) -> Lifted<Attribute<P>> {
        let kind = object.kind.as_slice();
        self.planar(object)?;
        if object.first(101).is_some() {
            return Err(invalid(kind, 101));
        }
        let text = self.text(object, point, length, 74, "dxf.ATTRIB.40")?;
        let tag = self.string(object, 2)?.ok_or_else(|| invalid(kind, 2))?;
        let hidden = self.int(object, 70, 0)? & 1 != 0;
        let style = self.style(object, out)?;
        let layer = self.layer(object, out)?;
        Ok(Attribute {
            tag,
            text,
            hidden,
            layer,
            style,
            source: source(object),
        })
    }

    /// HATCH（DXF Reference「HATCH」「Boundary Path Data」「Pattern Data」）。境界の輪の数（91）からは、値を
    /// 並びの順に読む。標高（30）が 0 でなければ平面に移さない。SPLINE の辺を持つものは、まだ移さない。
    /// 境界を作った図形（330）は、輪ごとの数（97）と合わなければ読めないとする。DWG の読み手は 330 を置かない
    /// ので、DWG では空にする。密度の画素の大きさ（47）は表示のための値なので移さない。グラデーション（450 が
    /// 1）は、色の RGB（421）だけを移し、色の番号（63）は移さない（推定。docs/formats/dxf.md）
    fn hatch<P: Copy>(
        &self,
        db: &AcadDb,
        object: &Object,
        point: &impl Fn(f64, f64) -> P,
        length: &impl Fn(f64) -> Length,
    ) -> Lifted<Hatch<P>> {
        let kind = object.kind.as_slice();
        if self.real(object, 30, Some(0.0))? != 0.0 {
            return Err(non_planar(kind));
        }
        let solid = match self.int(object, 70, 0)? {
            0 => false,
            1 => true,
            _ => return Err(invalid(kind, 70)),
        };
        let associative = self.int(object, 71, 0)? != 0;
        let start = object
            .values
            .iter()
            .position(|value| value.code == 91)
            .ok_or_else(|| invalid(kind, 91))?;
        let mut values = Values {
            kind,
            values: object.values.get(start..).unwrap_or_default(),
        };
        let mut loops = Vec::new();
        for _ in 0..values.count(91)? {
            let flags = values.int(92)?;
            let boundary = if flags & 2 != 0 {
                let bulges = values.int(72)? != 0;
                let closed = values.int(73)? != 0;
                let mut vertices = Vec::new();
                for _ in 0..values.count(93)? {
                    let at = point(values.real(10)?, values.real(20)?);
                    let bulge = if bulges && values.next() == Some(42) {
                        values.real(42)?
                    } else {
                        0.0
                    };
                    vertices.push(Vertex { point: at, bulge });
                }
                Boundary::Polyline { vertices, closed }
            } else {
                let mut edges = Vec::new();
                for _ in 0..values.count(93)? {
                    edges.push(self.hatch_edge(&mut values, point, length)?);
                }
                Boundary::Edges(edges)
            };
            let count = values.count(97)?;
            let mut sources = Vec::new();
            while sources.len() < count && values.next() == Some(330) {
                let Data::Handle(handle) = values.take(330)? else {
                    return Err(invalid(kind, 330));
                };
                let found = db.get(*handle).ok_or_else(|| invalid(kind, 330))?;
                sources.push(source(found));
            }
            if sources.len() != count && !matches!(object.origin, Origin::Dwg { .. }) {
                return Err(invalid(kind, 97));
            }
            loops.push(HatchLoop {
                boundary,
                external: flags & 1 != 0,
                outermost: flags & 16 != 0,
                derived: flags & 4 != 0,
                text_box: flags & 8 != 0,
                sources,
            });
        }
        let islands = match values.int(75)? {
            0 => Islands::Nested,
            1 => Islands::Outer,
            2 => Islands::Ignore,
            _ => return Err(invalid(kind, 75)),
        };
        let origin = match values.int(76)? {
            0 => PatternOrigin::UserDefined,
            1 => PatternOrigin::Predefined,
            2 => PatternOrigin::Custom,
            _ => return Err(invalid(kind, 76)),
        };
        // 塗りでも、パターンの欄を書く形式がある。塗りなら読むだけにする
        let pattern = if values.next() == Some(52) {
            let angle = values.real(52)?;
            let scale = values.real(41)?;
            let double = values.int(77)? != 0;
            let mut lines = Vec::new();
            for _ in 0..values.count(78)? {
                let angle = values.real(53)?;
                let base = [values.real(43)?, values.real(44)?];
                let offset = [values.real(45)?, values.real(46)?];
                let mut dashes = Vec::new();
                for _ in 0..values.count(79)? {
                    dashes.push(values.real(49)?);
                }
                lines.push(PatternLine {
                    angle,
                    base,
                    offset,
                    dashes,
                });
            }
            Some(Pattern {
                name: self.string(object, 2)?.unwrap_or_default(),
                origin,
                angle,
                scale,
                double,
                lines,
            })
        } else {
            None
        };
        if values.next() == Some(47) {
            values.real(47)?;
        }
        let mut seeds = Vec::new();
        if values.next() == Some(98) {
            for _ in 0..values.count(98)? {
                seeds.push(point(values.real(10)?, values.real(20)?));
            }
        }
        let fill = match (self.int(object, 450, 0)?, solid, pattern) {
            (0, true, _) => Fill::Solid,
            (0, false, Some(pattern)) => Fill::Pattern(pattern),
            (0, false, None) => return Err(invalid(kind, 52)),
            (1, _, _) => Fill::Gradient(self.gradient(object)?),
            _ => return Err(invalid(kind, 450)),
        };
        Ok(Hatch {
            loops,
            islands,
            fill,
            associative,
            seeds,
        })
    }

    /// HATCH の境界の辺。種類（72）が 1 なら線分、2 なら円弧、3 なら楕円弧（長軸の端は中心からの向き）、4 なら
    /// SPLINE
    fn hatch_edge<P: Copy>(
        &self,
        values: &mut Values,
        point: &impl Fn(f64, f64) -> P,
        length: &impl Fn(f64) -> Length,
    ) -> Lifted<Edge<P>> {
        let kind = values.kind;
        let edge = match values.int(72)? {
            1 => Edge::Line {
                start: point(values.real(10)?, values.real(20)?),
                end: point(values.real(11)?, values.real(21)?),
            },
            2 => {
                let center = point(values.real(10)?, values.real(20)?);
                let radius = values.real(40)?;
                if radius <= 0.0 {
                    return Err(invalid(kind, 40));
                }
                Edge::Arc {
                    center,
                    radius: length(radius),
                    start: values.real(50)?,
                    end: values.real(51)?,
                    counterclockwise: values.int(73)? != 0,
                }
            }
            3 => {
                let [x, y] = [values.real(10)?, values.real(20)?];
                let [dx, dy] = [values.real(11)?, values.real(21)?];
                let ratio = values.real(40)?;
                if !(ratio > 0.0 && ratio <= 1.0) {
                    return Err(invalid(kind, 40));
                }
                Edge::Elliptic {
                    center: point(x, y),
                    major: point(x + dx, y + dy),
                    ratio,
                    start: values.real(50)?,
                    end: values.real(51)?,
                    counterclockwise: values.int(73)? != 0,
                }
            }
            4 => {
                return Err(Box::new(Diagnostic::new(
                    codes::DXF_HATCH_SPLINE_UNSUPPORTED,
                )));
            }
            _ => return Err(invalid(kind, 72)),
        };
        Ok(edge)
    }

    /// HATCH のグラデーション（450〜470）。回転（460）はラジアンなので度に直す。1 色（452 が 1）なら、色の
    /// 濃さ（462）を持つ
    fn gradient(&self, object: &Object) -> Lifted<Gradient> {
        let kind = object.kind.as_slice();
        let start = object
            .values
            .iter()
            .position(|value| value.code == 450)
            .unwrap_or_default();
        let mut colors = Vec::new();
        for value in object.values.iter().skip(start) {
            match (value.code, &value.data) {
                (421, Data::Int(rgb @ 0..=0xFF_FFFF)) => {
                    let [_, r, g, b] = (*rgb as u32).to_be_bytes();
                    colors.push([r, g, b]);
                }
                (421, _) => return Err(invalid(kind, 421)),
                _ => {}
            }
        }
        let tint = match self.int(object, 452, 0)? {
            0 => None,
            _ => Some(self.real(object, 462, None)?),
        };
        Ok(Gradient {
            name: self.string(object, 470)?.unwrap_or_default(),
            angle: self.real(object, 460, Some(0.0))?.to_degrees(),
            shift: self.real(object, 461, Some(0.0))?,
            colors,
            tint,
        })
    }

    /// DIMENSION（DXF Reference「DIMENSION」「Common Dimension Group Codes」）。70 の下の 5 ビットが種類で、
    /// 64 は座標寸法の X、128 は利用者が動かした文字。利用者の文字（1）は、ないか空か `<>` なら測定値を、
    /// 空白 1 つなら書かないことを表す。複製の挿入点（12）・行間（41・72）・水平の向き（51）は移さない
    /// （推定。docs/formats/dxf.md）
    fn dimension<P: Copy>(
        &self,
        object: &Object,
        point: &impl Fn(f64, f64) -> P,
        length: &impl Fn(f64) -> Length,
    ) -> Lifted<Dimension<P>> {
        let kind = object.kind.as_slice();
        let at = |x: i16| self.planar_point(object, x, point);
        let real = |code: i16, default: Option<f64>| self.real(object, code, default);
        let flags = self.int(object, 70, 0)?;
        let dimension = match flags & 0b1_1111 {
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
        };
        let label = match self.string(object, 1)?.as_deref() {
            None | Some("") | Some("<>") => Label::Measurement,
            Some(" ") => Label::Hidden,
            Some(text) => Label::Override(text.to_owned()),
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
                    key: "dxf.DIMENSION.42",
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

    /// LEADER（DXF Reference「LEADER」）。頂点（10）は数（76）と合わなければ読めないとする。矢印（71）がなければ
    /// 寸法スタイルに従うとして `None` にする。注記（340）は、指すオブジェクトの出所にする。鉤の線（74・75）・
    /// 注記の種類と大きさ（73・40・41）・ずれ（212・213）は移さない（推定。docs/formats/dxf.md）
    fn leader<P: Copy>(
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
    fn planar_point<P>(
        &self,
        object: &Object,
        x: i16,
        point: &impl Fn(f64, f64) -> P,
    ) -> Lifted<P> {
        if self.real(object, x + 20, Some(0.0))? != 0.0 {
            return Err(non_planar(&object.kind));
        }
        Ok(point(
            self.real(object, x, None)?,
            self.real(object, x + 10, None)?,
        ))
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

    /// 寸法スタイルの表のレコード（DXF Reference「DIMSTYLE」）。端部記号は、ブロックのレコードのハンドル
    /// （DIMBLK の 342、DIMBLK1・DIMBLK2 の 343・344、DIMLDRBLK の 341）を名前にし、0 かなければ既定の矢印と
    /// する。両端を別にする（DIMSAH の 173）のでなければ、両端とも 342 にする。値がなければ DXF の既定の値
    /// （DIMSCALE の 40 は 1、DIMASZ の 41 は 0.18、DIMTSZ の 142 は 0）とする（推定。docs/formats/dxf.md）
    fn dimension_style_record(
        &self,
        db: &AcadDb,
        object: &Object,
        source: Ref,
        out: &mut Out,
    ) -> Lifted<()> {
        let kind = object.kind.as_slice();
        let name = self.string(object, 2)?.ok_or_else(|| invalid(kind, 2))?;
        let arrow = |code: i16| -> Lifted<Arrowhead> {
            match object.first(code) {
                None | Some(Data::Handle(0)) => Ok(Arrowhead::Default),
                Some(Data::Handle(handle)) => {
                    let record = db
                        .get(*handle)
                        .filter(|record| record.kind == b"BLOCK_RECORD")
                        .ok_or_else(|| invalid(kind, code))?;
                    let name = self.string(record, 2)?.ok_or_else(|| invalid(kind, code))?;
                    Ok(Arrowhead::Symbol(SymbolKey {
                        namespace: BLOCK,
                        name,
                    }))
                }
                Some(_) => Err(invalid(kind, code)),
            }
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

/// 図形の値の並びを、先頭から順に読む
struct Values<'a> {
    /// 図形の種類
    kind: &'a [u8],
    /// まだ読んでいない値
    values: &'a [Value],
}

impl<'a> Values<'a> {
    /// 次の値のグループ・コード
    fn next(&self) -> Option<i16> {
        self.values.first().map(|value| value.code)
    }

    /// 次の値。グループ・コードが `code` でなければ読めないとする
    fn take(&mut self, code: i16) -> Lifted<&'a Data> {
        match self.values.split_first() {
            Some((value, rest)) if value.code == code => {
                self.values = rest;
                Ok(&value.data)
            }
            _ => Err(invalid(self.kind, code)),
        }
    }

    /// 次の有限の実数
    fn real(&mut self, code: i16) -> Lifted<f64> {
        match self.take(code)? {
            Data::Real(value) if value.is_finite() => Ok(*value),
            _ => Err(invalid(self.kind, code)),
        }
    }

    /// 次の整数
    fn int(&mut self, code: i16) -> Lifted<i64> {
        match self.take(code)? {
            Data::Int(value) => Ok(*value),
            _ => Err(invalid(self.kind, code)),
        }
    }

    /// 次の 0 以上の数
    fn count(&mut self, code: i16) -> Lifted<usize> {
        usize::try_from(self.int(code)?).map_err(|_| invalid(self.kind, code))
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
