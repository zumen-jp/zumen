//! SFC から図面モデル（L2）への変換（決定 0022・0121）。
//!
//! 用紙・レイヤ・色・線種・線幅の表と、線分・折線・円・円弧・楕円・楕円弧・スプライン・クロソイド・文字・点マーカー・
//! 表題欄・画像を移す。部分図は実寸の空間にし、用紙への配置をビューにする。作図グループは、置いた所にそのまま展開する。ほかのフィーチャは、まだ移さずに診断する。

use std::collections::BTreeMap;

use zumen_base::{Diagnostic, Kind, Ref, codes};
use zumen_drawing::{
    Affine, Basis, Color, ColorCode, ColorSystem, Drawing2D, Edge, Element, Extension, Field,
    FieldValue, Flow, Font, FontKind, Format, FormattedText, Height, HeightMeaning, Horizontal,
    Inherit, Layer, LayerId, LayerState, Length, LineSpacing, LineType, LineTypeDefinition,
    LineTypeId, LineWeight, ModelLength, ModelPoint, ModelSpace, PaperLength, PaperPoint,
    PatternBasis, Piece, Polyline, Resource, Rotation, Shape, Sheet, SheetId, SheetItem, SpaceId,
    Span, Spline, Style, Symbol, SymbolKey, Target, TitleBlock, TitleItem, Underlay, UnderlayFrame,
    UnderlayKind, Unit, Vertex, Vertical, View,
};
use zumen_sdk::{Cx, Error, Lift};

use crate::{Mode, Quote, SxfFile, Value};

const SHEET: SheetId = SheetId(0);

/// 既定義線種のコード 1〜15 の名前（SXF の共通既定義要素の表）
const LINE_TYPES: [&str; 15] = [
    "continuous",
    "dashed",
    "dashed spaced",
    "long dashed dotted",
    "long dashed double-dotted",
    "long dashed triplicate-dotted",
    "dotted",
    "chain",
    "chain double dash",
    "dashed dotted",
    "double-dashed dotted",
    "dashed double-dotted",
    "double-dashed double-dotted",
    "dashed triplicate-dotted",
    "double-dashed triplicate-dotted",
];

/// 既定義線幅のコード 1〜9 の太さ（mm）
const WIDTHS: [f64; 9] = [0.13, 0.18, 0.25, 0.35, 0.5, 0.7, 1.0, 1.4, 2.0];

/// 既定義色のコードの数。利用者定義色は、この次から数える
const PREDEFINED_COLORS: i64 = 16;
/// 利用者定義線種の最初のコード
const USER_LINE_TYPE: i64 = 16;
/// 利用者定義線幅の最初のコード
const USER_WIDTH: i64 = 11;

/// JIS の A 列の用紙の縦長の幅と高さ（mm）。用紙サイズ種別 0〜4
const A_SIZES: [[f64; 2]; 5] = [
    [841.0, 1189.0],
    [594.0, 841.0],
    [420.0, 594.0],
    [297.0, 420.0],
    [210.0, 297.0],
];

/// 自由サイズの用紙サイズ種別
const FREE_SIZE: i64 = 9;

/// 図面表題欄の項目の、キーと項目名。引数の順（推定。docs/formats/sxf.md）
const TITLE_ITEMS: [(&str, &str); 12] = [
    ("business_name", "事業名"),
    ("construction_name", "工事名"),
    ("contract_type", "契約区分"),
    ("drawing_name", "図面名"),
    ("drawing_number", "図面番号"),
    ("drawing_type", "図面種別"),
    ("scale", "尺度"),
    ("year", "作成年"),
    ("month", "作成月"),
    ("day", "作成日"),
    ("contractor", "受注会社名"),
    ("client", "発注事業者名"),
];

impl Lift<Drawing2D> for SxfFile {
    /// SFC を変換する。P21 の変換はまだないので、空の図面を返して診断する
    fn lift(&self, cx: &mut Cx) -> Result<Drawing2D, Error> {
        if self.mode() == Mode::P21 {
            cx.report(Diagnostic::new(codes::SXF_P21_UNSUPPORTED));
            return Ok(Drawing2D::default());
        }
        Lifter::new(self).run(cx)
    }
}

/// フィーチャのレコード
#[derive(Clone, Copy)]
struct Feature<'f> {
    /// レコードの番号
    record: usize,
    /// `_feature` を除いた名前
    name: &'f [u8],
    args: &'f [Value],
}

impl Feature<'_> {
    fn is(&self, name: &[u8]) -> bool {
        self.name.eq_ignore_ascii_case(name)
    }

    fn source(&self) -> Ref {
        Ref::at_record(self.record as u64)
    }

    /// 診断に書く種類。フィーチャの名前
    fn kind(&self) -> String {
        let mut kind = String::from_utf8_lossy(self.name).to_ascii_lowercase();
        kind.push_str("_feature");
        kind
    }
}

/// 複合図形の定義（sfig_org）
struct Definition {
    /// 1・2 は部分図（数学座標系・測地座標系）、3 は作図グループ、4 は作図部品
    flag: i64,
    /// 前の定義の後に書いた要素のフィーチャの添字
    members: Vec<usize>,
    /// 部分図なら、その空間
    space: Option<SpaceId>,
    /// 定義のフィーチャ
    source: Ref,
}

/// 展開している画像の作図グループ。範囲を示す折れ線を、画像の下敷きにする
#[derive(Clone)]
struct Raster {
    /// 範囲を示す折れ線のフィーチャの添字
    outline: usize,
    /// 画像のファイルの名前
    path: String,
    /// 作図グループの定義
    definition: Ref,
}

/// 置く先の空間。点と長さを作る
trait Place {
    /// 点
    type P;
    fn point(&self, x: f64, y: f64) -> Self::P;
    fn length(&self, value: f64) -> Length;
    fn element(&self, element: Element<Self::P>, out: &mut Out);
    /// 次に足す要素を指す、拡張の対象と前提
    fn target(&self, element: &Element<Self::P>, out: &Out) -> (Target, Basis);
    /// 用紙なら、部分図を置くビューを足す
    fn view(&self, view: View, out: &mut Out) -> bool;
}

struct OnSheet;

impl Place for OnSheet {
    type P = PaperPoint;

    fn point(&self, x: f64, y: f64) -> PaperPoint {
        PaperPoint { sheet: SHEET, x, y }
    }

    fn length(&self, value: f64) -> Length {
        Length::Paper(PaperLength(value))
    }

    fn element(&self, element: Element<PaperPoint>, out: &mut Out) {
        out.order.push(SheetItem::Element(out.paper.len()));
        out.paper.push(element);
    }

    fn target(&self, element: &Element<PaperPoint>, out: &Out) -> (Target, Basis) {
        let target = Target::Paper {
            sheet: SHEET,
            index: out.paper.len(),
        };
        (target, Basis::Paper(element.clone()))
    }

    fn view(&self, view: View, out: &mut Out) -> bool {
        out.order.push(SheetItem::View(out.views.len()));
        out.views.push(view);
        true
    }
}

struct InSpace(SpaceId);

impl Place for InSpace {
    type P = ModelPoint;

    fn point(&self, x: f64, y: f64) -> ModelPoint {
        ModelPoint {
            space: self.0,
            x,
            y,
        }
    }

    fn length(&self, value: f64) -> Length {
        Length::Model(ModelLength {
            space: self.0,
            value,
        })
    }

    fn element(&self, element: Element<ModelPoint>, out: &mut Out) {
        out.model.push(element);
    }

    fn target(&self, element: &Element<ModelPoint>, out: &Out) -> (Target, Basis) {
        let target = Target::Model {
            space: self.0,
            index: out.model.len(),
        };
        (target, Basis::Model(element.clone()))
    }

    fn view(&self, _: View, _: &mut Out) -> bool {
        false
    }
}

#[derive(Default)]
struct Out {
    paper: Vec<Element<PaperPoint>>,
    views: Vec<View>,
    order: Vec<SheetItem>,
    model: Vec<Element<ModelPoint>>,
    extensions: Vec<Extension>,
}

struct Lifter<'f> {
    file: &'f SxfFile,
    features: Vec<Feature<'f>>,
    layers: Vec<Layer>,
    /// 定義のあるレイヤの数。コードは 1 から
    defined_layers: usize,
    line_types: Vec<LineType>,
    /// 利用者定義色の RGB。コードは 17 から
    colors: Vec<Option<[u8; 3]>>,
    /// 利用者定義線幅（mm）。コードは 11 から
    widths: Vec<Option<f64>>,
    /// 文字フォントの名前。コードは 1 から
    fonts: Vec<String>,
    /// 展開している画像の作図グループ
    raster: Option<Raster>,
    definitions: BTreeMap<String, Definition>,
}

impl<'f> Lifter<'f> {
    fn new(file: &'f SxfFile) -> Self {
        let features = file
            .records()
            .iter()
            .enumerate()
            .filter(|(_, record)| record.marker().is_some())
            .filter_map(|(record, entry)| {
                let part = entry.parts().first()?;
                let name = file.bytes(part.name());
                let name = name
                    .len()
                    .checked_sub(b"_feature".len())
                    .and_then(|end| {
                        let (head, tail) = name.split_at(end);
                        tail.eq_ignore_ascii_case(b"_feature").then_some(head)
                    })
                    .unwrap_or(name);
                Some(Feature {
                    record,
                    name,
                    args: part.args(),
                })
            })
            .collect();
        Self {
            file,
            features,
            layers: Vec::new(),
            defined_layers: 0,
            line_types: Vec::new(),
            colors: Vec::new(),
            widths: Vec::new(),
            fonts: Vec::new(),
            raster: None,
            definitions: BTreeMap::new(),
        }
    }

    fn run(mut self, cx: &mut Cx) -> Result<Drawing2D, Error> {
        let mut extensions = Vec::new();
        let mut title_blocks = Vec::new();
        let mut size = None;
        let mut pending = Vec::new();
        let mut sheet_members = Vec::new();
        let mut sheet_seen = false;
        let mut order = Vec::new();
        for index in 0..self.features.len() {
            let Some(&feature) = self.features.get(index) else {
                continue;
            };
            work(cx, 1, &feature.source())?;
            let read = if feature.is(b"layer") {
                self.layer(feature)
            } else if feature.is(b"user_defined_colour") {
                self.user_colour(feature)
            } else if feature.is(b"user_defined_font") {
                self.user_font(feature)
            } else if feature.is(b"width") {
                self.width(feature)
            } else if feature.is(b"text_font") {
                self.args(feature).text(0).map(|name| self.fonts.push(name))
            } else if feature.is(b"pre_defined_colour") || feature.is(b"pre_defined_font") {
                // 既定義のものは、コードで決まる
                Some(())
            } else if feature.is(b"drawing_sheet") {
                sheet_seen = true;
                sheet_members.append(&mut pending);
                self.sheet(feature).map(|(sheet, extension)| {
                    size = sheet;
                    extensions.push(extension);
                })
            } else if feature.is(b"sfig_org") {
                let members = core::mem::take(&mut pending);
                self.definition(feature, members, &mut order)
            } else if feature.is(b"composite_curve_org") {
                // 複合曲線の要素は、ハッチングの境界と一緒に移す
                pending.clear();
                dropped(cx, feature);
                Some(())
            } else if feature.is(b"drawing_attribute") {
                self.title_block(feature)
                    .map(|title_block| title_blocks.push(title_block))
            } else {
                pending.push(index);
                Some(())
            };
            if read.is_none() {
                invalid(cx, feature);
            }
        }
        if !pending.is_empty() {
            if sheet_seen {
                // 仕様では用紙のフィーチャが最後だが、後の要素も落とさない（推定。docs/formats/sxf.md）
                cx.report(
                    Diagnostic::new(codes::SXF_MEMBER_ASSUMED).at(self.features_source(&pending)),
                );
            }
            sheet_members.append(&mut pending);
        }

        let mut out = Out::default();
        let mut spaces = Vec::new();
        for name in &order {
            let Some(definition) = self.definitions.get(name) else {
                continue;
            };
            let Some(space) = definition.space else {
                continue;
            };
            let members = definition.members.clone();
            let mut stack = vec![name.clone()];
            self.members(&members, &InSpace(space), &mut out, &mut stack, cx)?;
            spaces.push(ModelSpace {
                id: space,
                unit: Unit::Millimeter,
                order: (0..out.model.len()).collect(),
                elements: core::mem::take(&mut out.model),
            });
        }
        self.members(&sheet_members, &OnSheet, &mut out, &mut Vec::new(), cx)?;
        extensions.append(&mut out.extensions);
        Ok(Drawing2D {
            spaces,
            sheets: vec![Sheet {
                id: SHEET,
                size,
                elements: out.paper,
                views: out.views,
                order: out.order,
            }],
            layers: self.layers,
            line_types: self.line_types,
            extensions,
            title_blocks,
            ..Drawing2D::default()
        })
    }

    fn features_source(&self, indices: &[usize]) -> Ref {
        indices
            .first()
            .and_then(|index| self.features.get(*index))
            .map(Feature::source)
            .unwrap_or_default()
    }

    fn args(&self, feature: Feature<'f>) -> Args<'f> {
        Args {
            file: self.file,
            values: feature.args,
        }
    }

    fn layer(&mut self, feature: Feature<'f>) -> Option<()> {
        let args = self.args(feature);
        let name = args.text(0)?;
        let visible = match args.int(1) {
            Some(flag) => flag != 0,
            None if feature.args.len() < 2 => true,
            None => return None,
        };
        self.defined_layers += 1;
        self.layers.push(Layer {
            id: LayerId(u32::try_from(self.defined_layers).ok()?),
            name,
            // SXF のレイヤは表示の可否だけを持つ。印刷は表示に合わせ、編集はできるとする（推定）
            state: Some(LayerState {
                visible,
                printable: visible,
                editable: true,
            }),
            style: None,
            source: feature.source(),
        });
        Some(())
    }

    fn user_colour(&mut self, feature: Feature<'f>) -> Option<()> {
        let args = self.args(feature);
        let channel = |at| args.int(at).and_then(|value| u8::try_from(value).ok());
        let rgb = (|| Some([channel(0)?, channel(1)?, channel(2)?]))();
        // 読めなくてもコードは数える。後の色のコードがずれないようにする
        self.colors.push(rgb);
        rgb.map(|_| ())
    }

    fn user_font(&mut self, feature: Feature<'f>) -> Option<()> {
        let args = self.args(feature);
        let code = USER_LINE_TYPE + 1 + self.user_line_types() as i64;
        let pattern = args.nums(2).filter(|pitch| {
            !pitch.is_empty() && pitch.len() % 2 == 0 && pitch.iter().all(|v| *v > 0.0)
        });
        let definition = pattern.map(|pitch| LineTypeDefinition {
            description: String::new(),
            // 線と間を交互に書く
            pattern: pitch
                .iter()
                .enumerate()
                .map(|(at, v)| if at % 2 == 0 { *v } else { -*v })
                .collect(),
            basis: PatternBasis::Paper,
        });
        let read = definition.is_some();
        self.line_types.push(LineType {
            id: LineTypeId(u32::try_from(code).ok()?),
            name: args.text(0).unwrap_or_default(),
            definition,
            source: feature.source(),
        });
        read.then_some(())
    }

    fn user_line_types(&self) -> usize {
        self.line_types
            .iter()
            .filter(|line_type| i64::from(line_type.id.0) > USER_LINE_TYPE)
            .count()
    }

    fn width(&mut self, feature: Feature<'f>) -> Option<()> {
        let width = self.args(feature).num(0).filter(|v| *v > 0.0);
        self.widths.push(width);
        width.map(|_| ())
    }

    /// 用紙の大きさと、種別を持つ形式固有の情報
    /// 図面表題欄。表示する要素との対応は持たない。足りない項目は空にする
    fn title_block(&self, feature: Feature<'f>) -> Option<TitleBlock> {
        let args = self.args(feature);
        let items = TITLE_ITEMS
            .iter()
            .enumerate()
            .map(|(at, (key, name))| {
                let value = if at < feature.args.len() {
                    args.text(at)?.trim().to_owned()
                } else {
                    String::new()
                };
                Some(TitleItem {
                    key: Some(key),
                    name: (*name).to_owned(),
                    value,
                    display: Vec::new(),
                })
            })
            .collect::<Option<_>>()?;
        Some(TitleBlock {
            items,
            source: feature.source(),
        })
    }

    fn sheet(&self, feature: Feature<'f>) -> Option<(Option<[PaperLength; 2]>, Extension)> {
        let args = self.args(feature);
        let kind = args.int(1)?;
        let orientation = args.int(2).unwrap_or(1);
        let size = match kind {
            FREE_SIZE => Some([args.num(3)?, args.num(4)?]).filter(|s| s.iter().all(|v| *v > 0.0)),
            _ => usize::try_from(kind)
                .ok()
                .and_then(|at| A_SIZES.get(at))
                .map(|&[w, h]| if orientation == 1 { [h, w] } else { [w, h] }),
        };
        let size = size?;
        let mut fields = vec![
            Field {
                key: "size",
                value: FieldValue::Integer(kind),
            },
            Field {
                key: "orientation",
                value: FieldValue::Integer(orientation),
            },
        ];
        if kind == FREE_SIZE {
            fields.extend([
                Field {
                    key: "width",
                    value: FieldValue::Real(size[0]),
                },
                Field {
                    key: "height",
                    value: FieldValue::Real(size[1]),
                },
            ]);
        }
        Some((
            Some(size.map(PaperLength)),
            Extension {
                namespace: "sxf.sheet",
                name: args.text(0).unwrap_or_default(),
                version: None,
                schema: 1,
                target: Target::Document,
                basis: Basis::Document,
                fields,
                source: feature.source(),
            },
        ))
    }

    fn definition(
        &mut self,
        feature: Feature<'f>,
        members: Vec<usize>,
        order: &mut Vec<String>,
    ) -> Option<()> {
        let args = self.args(feature);
        let name = args.text(0)?;
        let flag = args.int(1).filter(|flag| (1..=4).contains(flag))?;
        if self.definitions.contains_key(&name) {
            // 同じ名前の 2 つ目の定義は使わない
            return None;
        }
        let space = matches!(flag, 1 | 2)
            .then(|| u32::try_from(self.spaces()).ok().map(SpaceId))
            .flatten();
        self.definitions.insert(
            name.clone(),
            Definition {
                flag,
                members,
                space,
                source: feature.source(),
            },
        );
        order.push(name);
        Some(())
    }

    fn spaces(&self) -> usize {
        self.definitions
            .values()
            .filter(|definition| definition.space.is_some())
            .count()
    }

    /// 要素のフィーチャを、置く先に移す
    fn members<C: Place>(
        &mut self,
        members: &[usize],
        at: &C,
        out: &mut Out,
        stack: &mut Vec<String>,
        cx: &mut Cx,
    ) -> Result<(), Error> {
        for index in members {
            let Some(&feature) = self.features.get(*index) else {
                continue;
            };
            work(cx, 1, &feature.source())?;
            if feature.is(b"sfig_locate") {
                self.locate(feature, at, out, stack, cx)?;
                continue;
            }
            if feature.is(b"text_string") {
                if self.text(feature, at, out, cx).is_none() {
                    invalid(cx, feature);
                }
                continue;
            }
            if feature.is(b"point_marker") {
                if self.marker(feature, at, out, cx).is_none() {
                    invalid(cx, feature);
                }
                continue;
            }
            let outline = self
                .raster
                .as_ref()
                .filter(|r| r.outline == *index)
                .cloned();
            let shape = match outline {
                Some(raster) => self.underlay(feature, raster, at),
                None => self.shape(feature, at),
            };
            match shape {
                Shaped::Element(shape) => {
                    let element = self.element(feature, *shape, cx);
                    at.element(element, out);
                }
                Shaped::Invalid => invalid(cx, feature),
                Shaped::NotYet => dropped(cx, feature),
            }
        }
        Ok(())
    }

    fn locate<C: Place>(
        &mut self,
        feature: Feature<'f>,
        at: &C,
        out: &mut Out,
        stack: &mut Vec<String>,
        cx: &mut Cx,
    ) -> Result<(), Error> {
        let args = self.args(feature);
        let Some(name) = args.text(1) else {
            invalid(cx, feature);
            return Ok(());
        };
        let Some(definition) = self.definitions.get(&name) else {
            cx.report(
                Diagnostic::new(codes::SXF_SFIG_MISSING)
                    .with("name", name)
                    .at(feature.source()),
            );
            return Ok(());
        };
        match (definition.flag, definition.space) {
            // 作図グループは、配置の値によらず、置いた所にそのまま展開する（S-F 3-5 の補足）
            (3, _) => {
                if stack.contains(&name) {
                    cx.report(
                        Diagnostic::new(codes::SXF_SFIG_CYCLE_INVALID)
                            .with("name", name)
                            .at(feature.source()),
                    );
                    return Ok(());
                }
                let members = definition.members.clone();
                let raster = self.raster(&name, definition, cx);
                cx.enter().map_err(|stop| stopped(cx, stop, feature))?;
                stack.push(name);
                let outer = core::mem::replace(&mut self.raster, raster);
                let result = self.members(&members, at, out, stack, cx);
                self.raster = outer;
                stack.pop();
                cx.leave();
                result
            }
            (flag, Some(space)) => {
                let Some(transform) = (|| {
                    placement(
                        [args.num(2)?, args.num(3)?],
                        args.num(4)?,
                        [args.num(5)?, args.num(6)?],
                        flag == 2,
                    )
                })() else {
                    invalid(cx, feature);
                    return Ok(());
                };
                let view = View {
                    space,
                    sheet: SHEET,
                    transform,
                    clip: Vec::new(),
                    hidden_layers: Vec::new(),
                    source: feature.source(),
                };
                if !at.view(view, out) {
                    // 部分図の中に置いた部分図は、まだ移さない
                    dropped(cx, feature);
                }
                Ok(())
            }
            // 作図部品は、記号と一緒に移す
            _ => {
                dropped(cx, feature);
                Ok(())
            }
        }
    }

    /// 画像の作図グループなら、その画像。名前の `$$RASTER$$` か、属性の `$$ATRU$$番号$$名称$$画像$$` の後が
    /// ファイルの名前で、グループの中の最初の 4 点以上の折れ線が範囲を示す（推定。docs/formats/sxf.md）
    fn raster(&self, name: &str, definition: &Definition, cx: &mut Cx) -> Option<Raster> {
        let path = raster_path(name)?;
        let outline = definition.members.iter().copied().find(|index| {
            self.features.get(*index).is_some_and(|feature| {
                let args = self.args(*feature);
                feature.is(b"polyline")
                    && args.nums(5).is_some_and(|xs| xs.len() >= 4)
                    && args.nums(6).is_some_and(|ys| ys.len() >= 4)
            })
        });
        let Some(outline) = outline else {
            cx.report(
                Diagnostic::new(codes::SXF_RASTER_FRAME_MISSING)
                    .with("name", name.to_owned())
                    .at(definition.source.clone()),
            );
            return None;
        };
        Some(Raster {
            outline,
            path,
            definition: definition.source.clone(),
        })
    }

    /// 範囲を示す折れ線の 1・2・4 番目の点を角にした画像。2 番目の点が 1 番目から時計回りの側（上）にあれば、
    /// 4 番目の点を右下の角とする
    fn underlay<C: Place>(&self, feature: Feature<'f>, raster: Raster, at: &C) -> Shaped<C::P> {
        let args = self.args(feature);
        let corners = (|| {
            let xs = args.nums(5)?;
            let ys = args.nums(6)?;
            let corner = |i: usize| Some([*xs.get(i)?, *ys.get(i)?]);
            Some([corner(0)?, corner(1)?, corner(3)?])
        })();
        let Some([o, p, q]) = corners else {
            return Shaped::Invalid;
        };
        let cross = (p[0] - o[0]) * (q[1] - o[1]) - (p[1] - o[1]) * (q[0] - o[0]);
        if cross == 0.0 {
            return Shaped::Invalid;
        }
        let (right, top) = if cross < 0.0 { (q, p) } else { (p, q) };
        Shaped::Element(Box::new(Shape::Underlay(Underlay {
            kind: UnderlayKind::Image,
            resource: Resource {
                path: raster.path,
                definition: raster.definition,
            },
            page: None,
            frame: UnderlayFrame::Corners {
                origin: at.point(o[0], o[1]),
                right: at.point(right[0], right[1]),
                top: at.point(top[0], top[1]),
            },
            clip: None,
            visible: true,
        })))
    }

    fn shape<C: Place>(&self, feature: Feature<'f>, at: &C) -> Shaped<C::P> {
        let args = self.args(feature);
        let shape = if feature.is(b"line") {
            (|| {
                Some(Shape::Line {
                    start: at.point(args.num(4)?, args.num(5)?),
                    end: at.point(args.num(6)?, args.num(7)?),
                })
            })()
        } else if feature.is(b"polyline") {
            (|| {
                let xs = args.nums(5)?;
                let ys = args.nums(6)?;
                if xs.len() != ys.len() || xs.len() < 2 {
                    return None;
                }
                Some(Shape::Polyline(Polyline {
                    vertices: xs
                        .iter()
                        .zip(&ys)
                        .map(|(x, y)| Vertex {
                            point: at.point(*x, *y),
                            bulge: 0.0,
                        })
                        .collect(),
                    closed: false,
                    width: None,
                    vertex_widths: Vec::new(),
                }))
            })()
        } else if feature.is(b"circle") {
            (|| {
                Some(Shape::Circle {
                    center: at.point(args.num(4)?, args.num(5)?),
                    radius: at.length(args.num(6).filter(|r| *r > 0.0)?),
                })
            })()
        } else if feature.is(b"arc") {
            (|| {
                Some(Shape::Curve(Edge::Arc {
                    center: at.point(args.num(4)?, args.num(5)?),
                    radius: at.length(args.num(6).filter(|r| *r > 0.0)?),
                    start: args.num(8)?,
                    end: args.num(9)?,
                    // 向きのフラグは 0 が反時計回り
                    counterclockwise: args.int(7)? == 0,
                }))
            })()
        } else if feature.is(b"ellipse") {
            (|| {
                let center = [args.num(4)?, args.num(5)?];
                let radii = [args.num(6)?, args.num(7)?];
                ellipse(at, center, radii, args.num(8)?, [0.0, 360.0], true)
            })()
        } else if feature.is(b"ellipse_arc") {
            (|| {
                let center = [args.num(4)?, args.num(5)?];
                let radii = [args.num(6)?, args.num(7)?];
                let ccw = args.int(8)? == 0;
                ellipse(
                    at,
                    center,
                    radii,
                    args.num(9)?,
                    [args.num(10)?, args.num(11)?],
                    ccw,
                )
            })()
        } else if feature.is(b"spline") {
            (|| {
                let xs = args.nums(6)?;
                let ys = args.nums(7)?;
                Some(Shape::Curve(Edge::Spline(bezier(
                    &xs,
                    &ys,
                    // 開閉のフラグは 0 が閉じている（推定）
                    args.int(4)? == 0,
                    at,
                )?)))
            })()
        } else if feature.is(b"clothoid") {
            (|| {
                let start = args.num(9)?;
                let end = args.num(10)?;
                if start == end {
                    return None;
                }
                Some(Shape::Curve(Edge::Clothoid {
                    origin: at.point(args.num(4)?, args.num(5)?),
                    parameter: at.length(args.num(6).filter(|a| *a > 0.0)?),
                    // 向きのフラグは 0 が反時計回り
                    counterclockwise: args.int(7)? == 0,
                    angle: args.num(8)?,
                    start: at.length(start),
                    end: at.length(end),
                }))
            })()
        } else {
            return Shaped::NotYet;
        };
        shape.map_or(Shaped::Invalid, |shape| Shaped::Element(Box::new(shape)))
    }

    /// 点マーカー。引数は、レイヤと色のコード、点、マーカーのコード、回転、尺度。既定義のマーカーは中身を
    /// 持たないので、名前空間 `sxf.marker` のコードのキーで指す
    fn marker<C: Place>(
        &mut self,
        feature: Feature<'f>,
        at: &C,
        out: &mut Out,
        cx: &mut Cx,
    ) -> Option<()> {
        let args = self.args(feature);
        let code = args.int(4)?;
        let scale = args.num(6)?;
        let shape = Shape::Symbol(Symbol {
            key: SymbolKey {
                namespace: "sxf.marker",
                name: code.to_string(),
            },
            position: at.point(args.num(2)?, args.num(3)?),
            scale: [scale; 2],
            rotation: args.num(5)?,
            grid: None,
            attributes: Vec::new(),
        });
        let source = feature.source();
        let element = Element {
            layer: self.layer_id(args.int(0), &source, cx),
            style: Style {
                color: Inherit::Own(self.color(args.int(1), &source, cx)),
                line_type: Inherit::Layer,
                line_type_scale: 1.0,
                line_weight: None,
            },
            source,
            shape,
        };
        at.element(element, out);
        Some(())
    }

    /// 文字。引数は、レイヤ・色・フォントのコード、文字列、基点、文字範囲の高さと幅、字の間隔、回転、斜体、
    /// 9 点の配置、書く向き。範囲の幅と字の間隔は図面モデルの欄にないので、`sxf.text` の拡張に残す（決定 0122）
    fn text<C: Place>(
        &mut self,
        feature: Feature<'f>,
        at: &C,
        out: &mut Out,
        cx: &mut Cx,
    ) -> Option<()> {
        let args = self.args(feature);
        let content = args.text(3)?;
        let height = args.num(6).filter(|h| *h > 0.0)?;
        let width = args.num(7).filter(|w| *w >= 0.0)?;
        let spacing = args.num(8)?;
        let slant = args.num(10)?;
        // 1 が左下で、右へ、上へ数える
        let anchor = args.int(11).filter(|a| (1..=9).contains(a))? - 1;
        let horizontal = [Horizontal::Left, Horizontal::Center, Horizontal::Right]
            .get(usize::try_from(anchor % 3).ok()?)
            .copied()?;
        let vertical = [Vertical::Bottom, Vertical::Middle, Vertical::Top]
            .get(usize::try_from(anchor / 3).ok()?)
            .copied()?;
        // 書く向きは、1 が横書き、2 が縦書き。ないものは横書きとする
        let flow = match args.int(12) {
            None if feature.args.len() < 13 => Flow::LeftToRight,
            Some(1) => Flow::LeftToRight,
            Some(2) => Flow::TopToBottom,
            _ => return None,
        };
        let source = feature.source();
        let font = args.int(2).unwrap_or_default();
        let name = usize::try_from(font)
            .ok()
            .and_then(|code| code.checked_sub(1))
            .and_then(|at| self.fonts.get(at));
        if name.is_none() {
            missing(cx, "font", font, &source);
        }
        let format = Format {
            oblique: (slant != 0.0).then_some(slant),
            font: name.map(|name| Font {
                name: name.clone(),
                kind: FontKind::Named {
                    bold: false,
                    italic: false,
                    charset: None,
                    pitch: None,
                },
            }),
            ..Format::default()
        };
        let shape = Shape::Formatted(FormattedText {
            spans: vec![Span {
                piece: Piece::Text(content.clone()),
                format,
            }],
            content,
            height: Height {
                value: at.length(height),
                meaning: HeightMeaning::Frame,
            },
            point: at.point(args.num(4)?, args.num(5)?),
            horizontal,
            vertical,
            rotation: Rotation::Angle(args.num(9)?),
            width: None,
            frame_height: Some(at.length(height)),
            flow,
            line_spacing: LineSpacing {
                factor: 1.0,
                exact: false,
            },
            background: None,
            style: String::new(),
        });
        let element = Element {
            layer: self.layer_id(args.int(0), &source, cx),
            style: Style {
                color: Inherit::Own(self.color(args.int(1), &source, cx)),
                line_type: Inherit::Layer,
                line_type_scale: 1.0,
                line_weight: None,
            },
            source: source.clone(),
            shape,
        };
        let (target, basis) = at.target(&element, out);
        out.extensions.push(Extension {
            namespace: "sxf.text",
            name: String::new(),
            version: None,
            schema: 1,
            target,
            basis,
            fields: vec![
                Field {
                    key: "width",
                    value: FieldValue::Real(width),
                },
                Field {
                    key: "spacing",
                    value: FieldValue::Real(spacing),
                },
            ],
            source,
        });
        at.element(element, out);
        Some(())
    }

    /// レイヤ・色・線種・線幅のコード（引数の 0〜3）で、要素を作る
    fn element<P>(&mut self, feature: Feature<'f>, shape: Shape<P>, cx: &mut Cx) -> Element<P> {
        let args = self.args(feature);
        let source = feature.source();
        let layer = self.layer_id(args.int(0), &source, cx);
        let color = self.color(args.int(1), &source, cx);
        let line_type = self.line_type(args.int(2), &source, cx);
        let line_weight = self.line_weight(args.int(3), &source, cx);
        Element {
            source,
            layer,
            style: Style {
                color: Inherit::Own(color),
                line_type: Inherit::Own(line_type),
                line_type_scale: 1.0,
                line_weight: line_weight.map(|width| Inherit::Own(LineWeight::Width(width))),
            },
            shape,
        }
    }

    fn layer_id(&mut self, code: Option<i64>, source: &Ref, cx: &mut Cx) -> LayerId {
        let code = code.unwrap_or_default();
        let id = LayerId(u32::try_from(code).unwrap_or_default());
        let defined = code >= 1 && usize::try_from(code).is_ok_and(|c| c <= self.defined_layers);
        if !defined {
            missing(cx, "layer", code, source);
            if self.layer_of(id).is_none() {
                // 名前で指すだけのレイヤと同じに、定義のないレイヤを足す
                self.layers.push(Layer {
                    id,
                    name: code.to_string(),
                    state: None,
                    style: None,
                    source: Ref::default(),
                });
            }
        }
        id
    }

    fn layer_of(&self, id: LayerId) -> Option<&Layer> {
        self.layers.iter().find(|layer| layer.id == id)
    }

    fn color(&self, code: Option<i64>, source: &Ref, cx: &mut Cx) -> Color {
        let code = code.unwrap_or_default();
        let rgb = code
            .checked_sub(PREDEFINED_COLORS + 1)
            .and_then(|at| usize::try_from(at).ok())
            .and_then(|at| self.colors.get(at).copied());
        let defined = (1..=PREDEFINED_COLORS).contains(&code) || rgb.is_some();
        if !defined {
            missing(cx, "colour", code, source);
        }
        Color {
            code: Some(ColorCode {
                system: ColorSystem::Sxf,
                value: u32::try_from(code).unwrap_or_default(),
            }),
            // 既定義色の RGB は表から引かない（図面モデルは、ファイルが持つ RGB だけを持つ）
            rgb: rgb.flatten(),
        }
    }

    fn line_type(&mut self, code: Option<i64>, source: &Ref, cx: &mut Cx) -> LineTypeId {
        let code = code.unwrap_or_default();
        let id = LineTypeId(u32::try_from(code).unwrap_or_default());
        if self.line_types.iter().any(|line_type| line_type.id == id) {
            return id;
        }
        let name = usize::try_from(code - 1)
            .ok()
            .and_then(|at| LINE_TYPES.get(at));
        let name = match name {
            Some(name) => (*name).to_string(),
            None => {
                missing(cx, "line type", code, source);
                code.to_string()
            }
        };
        // 既定義線種は名前で決まり、ファイルは線と間の長さを持たない
        self.line_types.push(LineType {
            id,
            name,
            definition: None,
            source: Ref::default(),
        });
        id
    }

    fn line_weight(&self, code: Option<i64>, source: &Ref, cx: &mut Cx) -> Option<PaperLength> {
        let code = code.unwrap_or_default();
        let predefined = usize::try_from(code - 1)
            .ok()
            .and_then(|at| WIDTHS.get(at).copied());
        let user = code
            .checked_sub(USER_WIDTH)
            .and_then(|at| usize::try_from(at).ok())
            .and_then(|at| self.widths.get(at).copied())
            .flatten();
        let width = predefined.or(user);
        if width.is_none() {
            missing(cx, "width", code, source);
        }
        width.map(PaperLength)
    }
}

enum Shaped<P> {
    Element(Box<Shape<P>>),
    Invalid,
    NotYet,
}

/// 楕円か楕円弧。半径の大きい方の軸を長軸にし、角度は媒介変数の角度とする（推定。docs/formats/sxf.md）
fn ellipse<C: Place>(
    at: &C,
    [cx, cy]: [f64; 2],
    [rx, ry]: [f64; 2],
    rotation: f64,
    [start, end]: [f64; 2],
    counterclockwise: bool,
) -> Option<Shape<C::P>> {
    if !(rx > 0.0 && ry > 0.0) {
        return None;
    }
    let (major, ratio, shift) = if rx >= ry {
        (rx, ry / rx, 0.0)
    } else {
        (ry, rx / ry, 90.0)
    };
    let angle = (rotation + shift).to_radians();
    Some(Shape::Curve(Edge::Elliptic {
        center: at.point(cx, cy),
        major: at.point(cx + major * libm::cos(angle), cy + major * libm::sin(angle)),
        ratio,
        start: start - shift,
        end: end - shift,
        counterclockwise,
    }))
}

/// 部分図の配置。部分図の点を、尺度・回転・移動の順に用紙へ写す。測地座標系は X と Y を入れ替えてから写す
fn placement([x, y]: [f64; 2], angle: f64, [sx, sy]: [f64; 2], geodetic: bool) -> Option<Affine> {
    if sx == 0.0 || sy == 0.0 {
        return None;
    }
    let radians = angle.to_radians();
    let (cos, sin) = (libm::cos(radians), libm::sin(radians));
    let m = if geodetic {
        [-sy * sin, sx * cos, x, sy * cos, sx * sin, y]
    } else {
        [sx * cos, -sy * sin, x, sx * sin, sy * cos, y]
    };
    m.iter().all(|v| v.is_finite()).then_some(Affine { m })
}

/// フィーチャの引数を読む
struct Args<'f> {
    file: &'f SxfFile,
    values: &'f [Value],
}

impl Args<'_> {
    /// 文字列。`\'…\'` の `\\` を戻し、改行を除いて、Shift_JIS として読む
    fn text(&self, at: usize) -> Option<String> {
        let (raw, quote) = match self.values.get(at)? {
            Value::Text { raw, quote } => (self.file.bytes(raw.clone()), *quote),
            Value::Token(raw) => (self.file.bytes(raw.clone()), Quote::Plain),
            _ => return None,
        };
        let mut bytes = Vec::with_capacity(raw.len());
        let mut rest = raw.iter().copied().peekable();
        while let Some(byte) = rest.next() {
            if matches!(byte, b'\r' | b'\n') {
                continue;
            }
            if matches!(byte, 0x81..=0x9f | 0xe0..=0xfc) {
                bytes.push(byte);
                bytes.extend(rest.next());
                continue;
            }
            if quote == Quote::Backslash && byte == b'\\' && rest.peek() == Some(&b'\\') {
                rest.next();
            }
            bytes.push(byte);
        }
        let (text, _) = encoding_rs::SHIFT_JIS.decode_without_bom_handling(&bytes);
        Some(text.into_owned())
    }

    fn num(&self, at: usize) -> Option<f64> {
        number(self.text(at)?.trim())
    }

    fn int(&self, at: usize) -> Option<i64> {
        let value = self.num(at)?;
        (value.fract() == 0.0 && value.abs() < 9.0e15).then_some(value as i64)
    }

    /// 数の並び。`(…)` の並びか、`'(1,2)'` のように囲んだ文字列
    fn nums(&self, at: usize) -> Option<Vec<f64>> {
        match self.values.get(at)? {
            Value::List(items) => items
                .iter()
                .map(|item| {
                    let one = Args {
                        file: self.file,
                        values: core::slice::from_ref(item),
                    };
                    one.num(0)
                })
                .collect(),
            _ => {
                let text = self.text(at)?;
                let inner = text.trim().strip_prefix('(')?.strip_suffix(')')?.trim();
                if inner.is_empty() {
                    return Some(Vec::new());
                }
                inner.split(',').map(|item| number(item.trim())).collect()
            }
        }
    }
}

/// 3 次のベジェ曲線をつないだもの（制御点が 3n + 1 個）を、内側の節点を 3 重にした 3 次の B スプラインにする。
/// 形は変わらない
fn bezier<C: Place>(xs: &[f64], ys: &[f64], closed: bool, at: &C) -> Option<Spline<C::P>> {
    let count = xs.len();
    if count != ys.len() || count < 4 || !(count - 1).is_multiple_of(3) {
        return None;
    }
    let pieces = (count - 1) / 3;
    let mut knots = vec![0.0; 4];
    let mut knot = 0.0;
    for _ in 1..pieces {
        knot += 1.0;
        knots.extend([knot; 3]);
    }
    knots.extend([knot + 1.0; 4]);
    Some(Spline {
        degree: 3,
        knots,
        control: xs.iter().zip(ys).map(|(x, y)| at.point(*x, *y)).collect(),
        weights: Vec::new(),
        fit: Vec::new(),
        start_tangent: None,
        end_tangent: None,
        closed,
        periodic: false,
    })
}

/// 作図グループの名前が示す画像のファイルの名前
fn raster_path(name: &str) -> Option<String> {
    let parts: Vec<&str> = name.split("$$").collect();
    let path = match parts.iter().position(|part| *part == "ATRU") {
        // $$ATRU$$番号$$名称$$属性名$$値
        Some(at) => (parts.get(at + 3) == Some(&"画像")).then(|| parts.get(at + 4..))??,
        None => {
            let at = parts.iter().position(|part| *part == "RASTER")?;
            parts.get(at + 1..)?
        }
    };
    let path = path.join("$$");
    (!path.is_empty()).then_some(path)
}

fn number(text: &str) -> Option<f64> {
    text.parse::<f64>().ok().filter(|value| value.is_finite())
}

fn work(cx: &mut Cx, amount: u64, place: &Ref) -> Result<(), Error> {
    cx.check(Kind::Work, amount).map_err(|stop| {
        cx.report(stop.to_diagnostic().at(place.clone()));
        Error::Stopped(stop)
    })
}

fn stopped(cx: &mut Cx, stop: zumen_sdk::Stop, feature: Feature) -> Error {
    cx.report(stop.to_diagnostic().at(feature.source()));
    Error::Stopped(stop)
}

fn dropped(cx: &mut Cx, feature: Feature) {
    cx.report(
        Diagnostic::new(codes::SXF_LIFT_DROPPED)
            .with("kind", feature.kind())
            .at(feature.source()),
    );
}

fn invalid(cx: &mut Cx, feature: Feature) {
    cx.report(
        Diagnostic::new(codes::SXF_LIFT_INVALID)
            .with("kind", feature.kind())
            .at(feature.source()),
    );
}

fn missing(cx: &mut Cx, kind: &'static str, code: i64, source: &Ref) {
    cx.report(
        Diagnostic::new(codes::SXF_CODE_MISSING)
            .with("kind", kind)
            .with("code", code)
            .at(source.clone()),
    );
}
