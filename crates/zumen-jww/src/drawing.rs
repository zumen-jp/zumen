//! JWW から図面モデル（L2）への変換（決定 0022・0134）。
//!
//! 用紙・レイヤ・色・線種・線幅と、線・円・円弧・楕円・楕円弧・点・文字・ソリッドを移す。レイヤグループごとの縮尺は、
//! グループを実寸の空間にし、用紙への配置をビューにする。ブロック・寸法・画像は、まだ移さずに診断する。

use std::collections::BTreeMap;

use zumen_base::{Diagnostic, Kind, Ref, codes};
use zumen_drawing::{
    Affine, Boundary, Color, ColorCode, ColorSystem, Draft, Drawing2D, Edge, Element, Field,
    FieldValue, Fill, Hatch, HatchLoop, Height, HeightMeaning, Horizontal, Inherit, Islands, Layer,
    LayerId, LayerState, Length, LineType, LineTypeDefinition, LineTypeId, LineWeight, ModelLength,
    ModelPoint, ModelSpace, PaperLength, PatternBasis, Placement, Position, Resource, Shape, Sheet,
    SheetId, SheetItem, SpaceId, Style, Symbol, SymbolKey, Text, Underlay, UnderlayFrame,
    UnderlayKind, Unit, Vertex, Vertical, View,
};
use zumen_drawing::{
    Dimension, DimensionKind, DimensionText, Font, FontKind, Format, Label, Piece, Scaled, Span,
    SymbolDefinition,
};
use zumen_sdk::{Cx, Error, Lift};

use crate::parse::{Fail, Reader};
use crate::{JwwFile, Record, RecordKind};

mod apply;

/// 形式固有の情報の値の型の契約の版。どの `jww.*` の名前空間も同じ版にする（決定 0115・0156・0157）
const SCHEMA: u32 = 2;

const SHEET: SheetId = SheetId(0);

/// 寸法が SXF の欄を持つ最初の版
const SXF_SINCE: u32 = 420;

/// 図形の共通の欄に線幅が入る最初の版
const PEN_WIDTH_SINCE: u32 = 351;

/// 用紙の大きさのコードと、横長の幅と高さ（mm）。A0〜A4 と 2A〜5A
const PAPERS: [(u32, [f64; 2]); 9] = [
    (0, [1189.0, 841.0]),
    (1, [841.0, 594.0]),
    (2, [594.0, 420.0]),
    (3, [420.0, 297.0]),
    (4, [297.0, 210.0]),
    (8, [1682.0, 1189.0]),
    (9, [2378.0, 1682.0]),
    (10, [3364.0, 2378.0]),
    (11, [4756.0, 3364.0]),
];

/// 見出しから線種を読めないときの、線種 2〜9 の線と間（mm）
const DEFAULT_DASHES: [&[f64]; 8] = [
    &[1.0, 1.0],
    &[2.0, 1.0],
    &[4.0, 1.0],
    &[6.0, 1.0, 1.0, 1.0],
    &[10.0, 1.0, 1.0, 1.0],
    &[6.0, 1.0, 1.0, 1.0, 1.0, 1.0],
    &[10.0, 1.0, 1.0, 1.0, 1.0, 1.0],
    &[0.5, 0.5],
];

/// ランダム線（線種 11〜15）
const RANDOM_LINES: core::ops::RangeInclusive<u8> = 11..=15;

/// SXF の線種の最初の番号（30〜62）
const SXF_LINE_TYPE: u8 = 30;

/// 線種で記号を表す点
const POINT_SYMBOL: u8 = 100;

/// ソリッドの色を自分で持つ色の番号
const SOLID_OWN_COLOR: u16 = 10;

/// 円のソリッドを表す最初の線種
const CIRCLE_SOLID: u8 = 101;

/// 設定を書いた文字の位置（Jw_cad 8 から）
const SETTING_AT: [f64; 2] = [0.0, -1000.0];

/// ブロックの定義の空間の、最初の ID。レイヤグループの空間（0〜15）と重ならない
const DEFINITION_SPACES: u32 = 16;

/// ブロックの定義を指すキー。番号で指す
fn block_key(number: u32) -> SymbolKey {
    SymbolKey {
        namespace: "jww.block",
        name: number.to_string(),
    }
}

/// 1 周（ラジアン）
const TURN: f64 = 2.0 * core::f64::consts::PI;

impl Lift<Drawing2D> for JwwFile {
    fn lift(&self, cx: &mut Cx) -> Result<Drawing2D, Error> {
        let header = self
            .records()
            .first()
            .filter(|record| record.kind() == RecordKind::Header)
            .and_then(|record| Header::read(self, record.span().end).ok());
        if header.is_none() {
            cx.report(Diagnostic::new(codes::JWW_HEADER_INVALID).at(Ref::at_record(0)));
        }
        Lifter {
            file: self,
            header: header.unwrap_or_default(),
            line_types: BTreeMap::new(),
            groups: BTreeMap::new(),
            definitions: BTreeMap::new(),
            images: images(self),
            own: Vec::new(),
            drafts: Vec::new(),
            out: Drawing2D::default(),
        }
        .run(cx)
    }
}

/// レイヤグループ
#[derive(Clone, Debug, Default)]
struct Group {
    state: u32,
    /// 書き込むレイヤ
    write_layer: u32,
    scale: f64,
    protect: u32,
    layers: [u32; 16],
    /// レイヤの保護
    protects: [u32; 16],
}

/// 変換に使う見出しの欄
#[derive(Clone, Debug, Default)]
struct Header {
    version: u32,
    paper: u32,
    /// 書き込むレイヤグループ
    write_group: u32,
    groups: Vec<Group>,
    group_names: Vec<String>,
    /// 1 単位が 1/100 mm の線幅（負）か、プリンターのドット（ほか）
    max_draw_width: i32,
    layer_names: Vec<String>,
    /// 画面のペン 0〜9 の色
    pens: Vec<u32>,
    /// 印刷のペン 0〜9 の幅
    printer_widths: Vec<u32>,
    /// 線種 2〜9 と 16〜19 の、ビットの並び・単位・ピッチ・印刷のピッチ
    patterns: Vec<[u32; 4]>,
    /// 印刷の解像度（dpi）
    dpi: Option<f64>,
    /// SXF の拡張色 100〜356 の色と、印刷の幅
    sxf_pens: Vec<(u32, u32)>,
    /// SXF の線種 30〜62 の名前と、線と間
    sxf_line_types: Vec<(String, Option<Vec<f64>>)>,
}

impl Header {
    /// 見出しを読む。終わりは図形の一覧の件数の前
    fn read(file: &JwwFile, end: usize) -> Result<Header, Fail> {
        let src = file.bytes(0..end);
        let version = file.version();
        let since = |v: u32, n: usize| if version >= v { n } else { 0 };
        let mut r = Reader::new(src, 12);
        r.string()?;
        let paper = r.u32()?;
        let write_group = r.u32()?;
        let mut groups = Vec::with_capacity(16);
        for _ in 0..16 {
            let state = r.u32()?;
            let write_layer = r.u32()?;
            let scale = r.f64()?;
            let protect = r.u32()?;
            let mut layers = [0; 16];
            let mut protects = [0; 16];
            for (layer, protect) in layers.iter_mut().zip(&mut protects) {
                *layer = r.u32()?;
                *protect = r.u32()?;
            }
            groups.push(Group {
                state,
                write_layer,
                scale,
                protect,
                layers,
                protects,
            });
        }
        r.skip(80)?;
        let max_draw_width = r.u32()?.cast_signed();
        r.skip(72)?;
        let mut layer_names = Vec::with_capacity(256);
        for _ in 0..256 {
            let (unicode, bytes) = r.string()?;
            layer_names.push(decode(unicode, bytes));
        }
        let mut group_names = Vec::with_capacity(16);
        for _ in 0..16 {
            let (unicode, bytes) = r.string()?;
            group_names.push(decode(unicode, bytes));
        }
        let jumps = if version >= 300 { 8 * 28 } else { 4 * 24 };
        r.skip(28 + since(300, 16) + 4 + 24 + 24 + jumps + since(300, 56) + 80 + 8)?;
        let mut pens = Vec::with_capacity(10);
        for _ in 0..10 {
            pens.push(r.u32()?);
            r.skip(4)?;
        }
        let mut printer_widths = Vec::with_capacity(10);
        for _ in 0..10 {
            r.skip(4)?;
            printer_widths.push(r.u32()?);
            r.skip(8)?;
        }
        let mut patterns = Vec::with_capacity(12);
        for _ in 0..8 {
            patterns.push([r.u32()?, r.u32()?, r.u32()?, r.u32()?]);
        }
        r.skip(100)?;
        for _ in 0..4 {
            patterns.push([r.u32()?, r.u32()?, r.u32()?, r.u32()?]);
        }
        let mut flags = [0; 11];
        for flag in &mut flags {
            *flag = r.u32()?;
        }
        // 印刷の設定の 11 番目の 10 の位が、解像度を表す（推定）
        let dpi = match flags.get(10).map(|flag| flag / 10) {
            Some(1) if version >= SXF_SINCE => Some(300.0),
            Some(2) if version >= SXF_SINCE => Some(600.0),
            _ => None,
        };
        r.skip(since(223, 60) + since(225, 32) + since(230, 8))?;
        let mut sxf_pens = Vec::new();
        let mut sxf_line_types = Vec::new();
        if version >= SXF_SINCE {
            let mut colors = Vec::with_capacity(257);
            for _ in 0..257 {
                colors.push(r.u32()?);
                r.skip(4)?;
            }
            for color in colors {
                r.string()?;
                r.skip(4)?;
                let width = r.u32()?;
                r.skip(8)?;
                sxf_pens.push((color, width));
            }
            r.skip(33 * 16)?;
            for _ in 0..33 {
                let (unicode, bytes) = r.string()?;
                let segments = r.u32()?;
                let mut pitches = [0.0; 10];
                for pitch in &mut pitches {
                    *pitch = r.f64()?;
                }
                let pattern = usize::try_from(segments)
                    .ok()
                    .and_then(|n| pitches.get(..n))
                    .filter(|values| values.iter().all(|v| v.is_finite() && *v >= 0.0))
                    .map(dashes);
                sxf_line_types.push((decode(unicode, bytes), pattern));
            }
        }
        Ok(Header {
            version,
            paper,
            write_group,
            groups,
            group_names,
            max_draw_width,
            layer_names,
            pens,
            printer_widths,
            patterns,
            dpi,
            sxf_pens,
            sxf_line_types,
        })
    }

    /// レイヤグループの縮尺。読めないか正でなければ 1
    fn scale(&self, group: usize) -> f64 {
        self.groups
            .get(group)
            .map(|group| group.scale)
            .filter(|scale| scale.is_finite() && *scale > 0.0)
            .unwrap_or(1.0)
    }

    /// 色。1〜9 と 100〜356 は見出しの色を、ほかは番号だけを持つ
    fn color(&self, pen: u16) -> Color {
        let rgb = match pen {
            1..=9 => self.pens.get(usize::from(pen)).copied(),
            100..=356 => self
                .sxf_pens
                .get(usize::from(pen - 100))
                .map(|(color, _)| *color),
            _ => None,
        };
        Color {
            code: Some(ColorCode {
                system: ColorSystem::Jww,
                value: u32::from(pen),
            }),
            rgb: rgb.map(colorref),
        }
    }

    /// 線幅。図形の線幅、ペンの印刷の幅の順に使う。単位が分からなければ `None`
    fn weight(&self, pen_color: u16, pen_width: u16) -> Option<Inherit<LineWeight>> {
        let pen = match pen_color {
            1..=9 => self.printer_widths.get(usize::from(pen_color)).copied(),
            100..=356 => self
                .sxf_pens
                .get(usize::from(pen_color - 100))
                .map(|(_, width)| *width),
            _ => None,
        };
        self.width(u32::from(pen_width))
            .or_else(|| pen.and_then(|width| self.width(width)))
            .map(|mm| Inherit::Own(LineWeight::Width(PaperLength(mm))))
    }

    /// 見出しの線幅の単位を mm にする。単位が分からなければ `None`
    fn width(&self, value: u32) -> Option<f64> {
        if value == 0 {
            return None;
        }
        if self.max_draw_width < 0 {
            return Some(f64::from(value) / 100.0);
        }
        self.dpi.map(|dpi| f64::from(value) * 25.4 / dpi)
    }
}

/// 線と間の長さを、線を正・間を負にした列にする
fn dashes(values: &[f64]) -> Vec<f64> {
    values
        .iter()
        .enumerate()
        .map(|(i, v)| if i % 2 == 0 { *v } else { -*v })
        .collect()
}

/// 線種のビットの並びを、線と間の列（mm）にする。1 ビットは印刷のピッチの 1/32 mm（推定）
fn bit_pattern([pattern, unit, _, printer_pitch]: [u32; 4]) -> Option<Vec<f64>> {
    if !(1..=32).contains(&unit) || printer_pitch == 0 {
        return None;
    }
    let bit = |i: u32| (pattern >> (i % unit)) & 1 == 1;
    // 前のビットが 0 の、1 のビットから数える
    let start = (0..unit).find(|&i| bit(i) && !bit(i + unit - 1))?;
    let mut runs = Vec::new();
    let mut run = 0u32;
    let mut on = true;
    for i in start..start + unit {
        if bit(i) == on {
            run += 1;
        } else {
            runs.push(f64::from(run));
            run = 1;
            on = !on;
        }
    }
    runs.push(f64::from(run));
    if runs.len() < 2 {
        return None;
    }
    let size = f64::from(printer_pitch) / 32.0;
    Some(dashes(
        &runs.iter().map(|run| run * size).collect::<Vec<_>>(),
    ))
}

/// 同梱画像の名前と、レコードの位置
fn images(file: &JwwFile) -> BTreeMap<String, usize> {
    file.records()
        .iter()
        .enumerate()
        .filter(|(_, record)| record.kind() == RecordKind::Image)
        .filter_map(|(at, record)| {
            let mut r = Reader::new(file.bytes(0..record.span().end), record.span().start);
            let (unicode, name) = r.string().ok()?;
            Some((decode(unicode, name), at))
        })
        .collect()
}

/// 文字列を戻す。UTF-16LE でなければ Shift_JIS
fn decode(unicode: bool, bytes: &[u8]) -> String {
    if unicode {
        let units = bytes
            .as_chunks::<2>()
            .0
            .iter()
            .map(|pair| u16::from_le_bytes(*pair));
        char::decode_utf16(units)
            .map(|c| c.unwrap_or(char::REPLACEMENT_CHARACTER))
            .collect()
    } else {
        encoding_rs::SHIFT_JIS
            .decode_without_bom_handling(bytes)
            .0
            .into_owned()
    }
}

/// 図形の共通の欄
struct Base {
    /// 曲線属性の番号
    curve: u32,
    pen_style: u8,
    pen_color: u16,
    pen_width: u16,
    layer: usize,
    group: usize,
    /// 属性のフラグ
    flag: u16,
    /// 図形を置く空間と、座標に掛ける倍率
    at: At,
}

/// 図形を置く空間と、座標に掛ける倍率。レイヤグループの空間は縮尺の分母、ブロックの定義の空間は 1
#[derive(Clone, Copy, Debug)]
struct At {
    space: SpaceId,
    scale: f64,
}

fn point(at: At, x: f64, y: f64) -> ModelPoint {
    ModelPoint {
        space: at.space,
        x: x * at.scale,
        y: y * at.scale,
    }
}

fn length(at: At, value: f64) -> Length {
    Length::Model(ModelLength {
        space: at.space,
        value: value * at.scale,
    })
}

impl Base {
    fn read(r: &mut Reader, version: u32) -> Result<Base, Fail> {
        let curve = r.u32()?;
        let pen_style = r.u8()?;
        let pen_color = r.u16()?;
        let pen_width = if version >= PEN_WIDTH_SINCE {
            r.u16()?
        } else {
            0
        };
        let layer = usize::from(r.u16()? & 0xF);
        let group = usize::from(r.u16()? & 0xF);
        let flag = r.u16()?;
        Ok(Base {
            curve,
            pen_style,
            pen_color,
            pen_width,
            layer,
            group,
            flag,
            at: At {
                space: SpaceId(0),
                scale: 1.0,
            },
        })
    }

    fn layer(&self) -> LayerId {
        LayerId((self.group * 16 + self.layer + 1) as u32)
    }
}

struct Lifter<'f> {
    file: &'f JwwFile,
    header: Header,
    /// 使った線種の番号
    line_types: BTreeMap<u8, LineTypeId>,
    /// 要素を置いたレイヤグループと、その空間の要素
    groups: BTreeMap<usize, Vec<Element<ModelPoint>>>,
    /// ブロックの定義のレコードの位置から、記号の定義の位置へ
    definitions: BTreeMap<usize, usize>,
    /// 同梱画像の名前から、レコードの位置へ
    images: BTreeMap<String, usize>,
    /// 変換中の要素に付ける拡張の名前空間と欄
    own: Vec<(&'static str, Vec<Field>)>,
    /// 付く先を位置で指す拡張。要素がそろってから ID とハッシュにする（決定 0156）
    drafts: Vec<Draft>,
    out: Drawing2D,
}

impl Lifter<'_> {
    fn run(mut self, cx: &mut Cx) -> Result<Drawing2D, Error> {
        self.layers();
        self.edit_state();
        for (index, record) in self.file.records().iter().enumerate() {
            let source = Ref::at_record(index as u64);
            cx.check(Kind::Work, 1).map_err(|stop| {
                cx.report(stop.to_diagnostic().at(source.clone()));
                Error::Stopped(stop)
            })?;
            match record.kind() {
                RecordKind::Entity => self.entity(record, source, cx),
                RecordKind::Block if self.definition(index, record, source.clone()).is_err() => {
                    invalid(cx, self.file.kind_name(record), source);
                }
                _ => {}
            }
        }
        let drafts = core::mem::take(&mut self.drafts);
        let mut drawing = self.finish();
        drawing.settle_extensions(drafts, cx).map_err(|stop| {
            cx.report(stop.to_diagnostic());
            Error::Stopped(stop)
        })?;
        Ok(drawing)
    }

    /// 書き込むグループとレイヤ・保護・グループの名前を、図面の拡張にする。保護しないレイヤは拡張にしない
    fn edit_state(&mut self) {
        let header = &self.header;
        if header.groups.is_empty() {
            return;
        }
        let integer = |key, value: u32| Field {
            key,
            code: None,
            value: FieldValue::Integer(i64::from(value)),
        };
        let extension = |namespace, name, fields| Draft {
            namespace,
            name,
            version: Some(header.version.to_string()),
            schema: SCHEMA,
            at: Position::Document,
            fields,
            source: Ref::at_record(0),
        };
        let mut out = vec![extension(
            "jww.edit",
            String::new(),
            vec![integer("write_group", header.write_group)],
        )];
        for (at, group) in header.groups.iter().enumerate() {
            let name = header.group_names.get(at).cloned().unwrap_or_default();
            out.push(extension(
                "jww.group",
                format!("{at:X}"),
                vec![
                    Field {
                        key: "name",
                        code: None,
                        value: FieldValue::Text(name),
                    },
                    integer("write_layer", group.write_layer),
                    integer("protect", group.protect),
                ],
            ));
            for (layer, &protect) in group.protects.iter().enumerate() {
                if protect != 0 {
                    out.push(extension(
                        "jww.layer",
                        format!("{at:X}-{layer:X}"),
                        vec![integer("protect", protect)],
                    ));
                }
            }
        }
        self.drafts.extend(out);
    }

    /// 16 のレイヤグループの 16 のレイヤ。番号はグループ × 16 + レイヤ + 1
    fn layers(&mut self) {
        let header = &self.header;
        for group in 0..16 {
            for layer in 0..16 {
                let at = group * 16 + layer;
                let name = header
                    .layer_names
                    .get(at)
                    .filter(|name| !name.is_empty())
                    .cloned()
                    .unwrap_or_else(|| format!("{group:X}-{layer:X}"));
                let state = header.groups.get(group).map(|g| {
                    let own = g.layers.get(layer).copied().unwrap_or_default();
                    let visible = g.state != 0 && own != 0;
                    LayerState {
                        visible,
                        printable: visible,
                        editable: g.state >= 2 && own >= 2,
                        frozen: false,
                    }
                });
                self.out.layers.push(Layer {
                    id: LayerId(at as u32 + 1),
                    name,
                    state,
                    style: None,
                    follows_placement: false,
                    source: Ref::at_record(0),
                    description: None,
                });
            }
        }
    }

    /// ブロックの定義（決定 0135）。番号を名前空間 `jww.block` のキーにし、中身は定義ごとの空間に、用紙の mm のまま置く。
    /// 名前と、使われているかは、拡張に残す
    fn definition(&mut self, index: usize, record: &Record, source: Ref) -> Result<(), Fail> {
        let mut r = Reader::new(self.file.bytes(0..record.span().end), record.body().start);
        Base::read(&mut r, self.header.version)?;
        let number = r.u32()?;
        let referenced = r.u32()?;
        r.skip(4)?;
        let (unicode, name) = r.string()?;
        let name = decode(unicode, name);
        let space = SpaceId(DEFINITION_SPACES + self.out.symbols.len() as u32);
        self.definitions.insert(index, self.out.symbols.len());
        self.out.symbols.push(SymbolDefinition {
            key: block_key(number),
            space: ModelSpace {
                id: space,
                unit: Unit::Millimeter,
                elements: Vec::new(),
                order: Vec::new(),
            },
            base: Some(ModelPoint {
                space,
                x: 0.0,
                y: 0.0,
            }),
            attributes: Vec::new(),
            anonymous: false,
            external: None,
            source: source.clone(),
        });
        self.drafts.push(Draft {
            namespace: "jww.block",
            name: number.to_string(),
            version: Some(self.header.version.to_string()),
            schema: SCHEMA,
            at: Position::Document,
            fields: vec![
                Field {
                    key: "name",
                    code: None,
                    value: FieldValue::Text(name),
                },
                Field {
                    key: "referenced",
                    code: None,
                    value: FieldValue::Integer(i64::from(referenced)),
                },
            ],
            source,
        });
        Ok(())
    }

    fn entity(&mut self, record: &Record, source: Ref, cx: &mut Cx) {
        let name = self.file.kind_name(record);
        let mut r = Reader::new(self.file.bytes(0..record.span().end), record.body().start);
        let version = self.header.version;
        let Ok(mut base) = Base::read(&mut r, version) else {
            invalid(cx, name, source);
            return;
        };
        let definition = record
            .block()
            .and_then(|block| self.definitions.get(&block).copied());
        if record.block().is_some() && definition.is_none() {
            // 読めなかった定義の中身
            return;
        }
        base.at = match definition.and_then(|at| self.out.symbols.get(at)) {
            Some(symbol) => At {
                space: symbol.space.id,
                scale: 1.0,
            },
            None => At {
                space: SpaceId(base.group as u32),
                scale: self.header.scale(base.group),
            },
        };
        self.own.clear();
        let lifted = match name {
            b"CDataSen" => self.line(&mut r, &base),
            b"CDataEnko" => self.arc(&mut r, &base),
            b"CDataTen" => self.point(&mut r, &base),
            b"CDataMoji" => self.text(&mut r, &base, &source),
            b"CDataSolid" => self.solid(&mut r, &base),
            b"CDataBlock" => self.insert(&mut r, &base),
            b"CDataSunpou" => self.dimension(&mut r, &base),
            _ => {
                dropped(cx, name, source);
                return;
            }
        };
        let (shape, style) = match lifted {
            Ok(Some(lifted)) => lifted,
            Ok(None) => return,
            Err(_) => {
                invalid(cx, name, source);
                return;
            }
        };
        let element = Element {
            source: source.clone(),
            layer: base.layer(),
            style,
            shape,
        };
        let elements = match definition.and_then(|at| self.out.symbols.get_mut(at)) {
            Some(symbol) => {
                symbol.space.order.push(symbol.space.elements.len());
                &mut symbol.space.elements
            }
            None => self.groups.entry(base.group).or_default(),
        };
        if base.curve != 0 || base.flag != 0 {
            self.own.push((
                "jww.entity",
                vec![
                    Field {
                        key: "curve",
                        code: None,
                        value: FieldValue::Integer(i64::from(base.curve)),
                    },
                    Field {
                        key: "flag",
                        code: None,
                        value: FieldValue::Integer(i64::from(base.flag)),
                    },
                ],
            ));
        }
        for (namespace, fields) in self.own.drain(..) {
            self.drafts.push(Draft {
                namespace,
                name: String::new(),
                version: Some(version.to_string()),
                schema: SCHEMA,
                at: Position::Model {
                    space: base.at.space,
                    index: elements.len(),
                },
                fields,
                source: source.clone(),
            });
        }
        elements.push(element);
    }

    /// ブロックの参照。尺度にレイヤグループの縮尺を掛け、回転はラジアンから度にする
    fn insert(
        &mut self,
        r: &mut Reader,
        base: &Base,
    ) -> Result<Option<(Shape<ModelPoint>, Style)>, Fail> {
        let [x, y, sx, sy, rotation] = [r.f64()?, r.f64()?, r.f64()?, r.f64()?, r.f64()?];
        let number = r.u32()?;
        let shape = Shape::Symbol(Symbol {
            key: block_key(number),
            position: point(base.at, x, y),
            scale: [sx * base.at.scale, sy * base.at.scale],
            rotation: rotation.to_degrees(),
            grid: None,
            attributes: Vec::new(),
            clip: None,
        });
        // 中身の色・線種・線幅は、定義の図形が持つ
        let style = Style {
            color: Inherit::Block,
            line_type: Inherit::Block,
            line_type_scale: 1.0,
            line_weight: None,
            transparency: Inherit::Layer,
        };
        Ok(Some((shape, style)))
    }

    /// 寸法（決定 0135）。寸法線の向きに測る長さとし、測る点は、版 420 からの SXF の形があれば基準点、なければ寸法線の端
    /// とする。寸法値の文字は、そのまま上書きの文字にする。寸法線・補助線・端の点は `jww.dimension` の拡張に残す
    fn dimension(
        &mut self,
        r: &mut Reader,
        base: &Base,
    ) -> Result<Option<(Shape<ModelPoint>, Style)>, Fail> {
        let version = self.header.version;
        let at = base.at;
        let xy = |p: ModelPoint| FieldValue::Point([p.x, p.y, 0.0]);
        let line_base = Base::read(r, version)?;
        let [x1, y1, x2, y2] = [r.f64()?, r.f64()?, r.f64()?, r.f64()?];
        let (start, end) = (point(at, x1, y1), point(at, x2, y2));
        let mut fields = vec![
            Field {
                key: "line_start",
                code: None,
                value: xy(start),
            },
            Field {
                key: "line_end",
                code: None,
                value: xy(end),
            },
        ];
        Base::read(r, version)?;
        let [tx1, ty1, tx2, ty2] = [r.f64()?, r.f64()?, r.f64()?, r.f64()?];
        r.skip(4)?;
        let [size_x, size_y, spacing, angle] = [r.f64()?, r.f64()?, r.f64()?, r.f64()?];
        let (unicode, font) = r.string()?;
        let font = decode(unicode, font);
        let (unicode, content) = r.string()?;
        let content = decode(unicode, content);
        fields.push(Field {
            key: "text_end",
            code: None,
            value: xy(point(at, tx2, ty2)),
        });
        fields.push(Field {
            key: "text_spacing",
            code: None,
            value: FieldValue::Real(spacing),
        });
        let (mut first, mut second) = (start, end);
        if version >= SXF_SINCE {
            let mode = r.u16()?;
            fields.push(Field {
                key: "sxf_mode",
                code: None,
                value: FieldValue::Integer(i64::from(mode)),
            });
            for (start_key, end_key) in [
                ("extension1_start", "extension1_end"),
                ("extension2_start", "extension2_end"),
            ] {
                Base::read(r, version)?;
                let [x1, y1, x2, y2] = [r.f64()?, r.f64()?, r.f64()?, r.f64()?];
                fields.push(Field {
                    key: start_key,
                    code: None,
                    value: xy(point(at, x1, y1)),
                });
                fields.push(Field {
                    key: end_key,
                    code: None,
                    value: xy(point(at, x2, y2)),
                });
            }
            let mut points = Vec::with_capacity(4);
            for key in ["end1", "end2", "reference1", "reference2"] {
                let point_base = Base::read(r, version)?;
                let p = point(at, r.f64()?, r.f64()?);
                r.skip(4)?;
                if point_base.pen_style == POINT_SYMBOL {
                    r.skip(20)?;
                }
                fields.push(Field {
                    key,
                    code: None,
                    value: xy(p),
                });
                points.push(p);
            }
            // SXF の形を持つ寸法は、基準点を測る点とする（推定）
            if mode != 0
                && let [_, _, reference1, reference2] = points[..]
            {
                (first, second) = (reference1, reference2);
            }
        }
        let rotation = libm::atan2(y2 - y1, x2 - x1).to_degrees();
        let format = Format {
            height: Some(Scaled::Value(Length::Paper(PaperLength(size_y)))),
            width_factor: (size_y > 0.0).then_some(Scaled::Value(size_x / size_y)),
            font: (!font.is_empty()).then_some(Font {
                name: font,
                kind: FontKind::Named {
                    bold: false,
                    italic: false,
                    charset: None,
                    pitch: None,
                },
            }),
            ..Format::default()
        };
        let shape = Shape::Dimension(Dimension {
            kind: DimensionKind::Linear {
                first,
                second,
                line: start,
                rotation,
                oblique: 0.0,
            },
            measurement: None,
            label: Label::Override {
                spans: vec![Span {
                    piece: Piece::Text(content.clone()),
                    format,
                }],
                content,
            },
            text: DimensionText {
                position: Some(point(at, tx1, ty1)),
                moved: false,
                rotation: Some(angle),
                attachment: Some((Horizontal::Left, Vertical::Bottom)),
            },
            style: String::new(),
            block: None,
        });
        self.own.push(("jww.dimension", fields));
        Ok(Some((shape, self.line_style(&line_base))))
    }

    /// 線の色・線種・線幅
    fn line_style(&mut self, base: &Base) -> Style {
        Style {
            color: Inherit::Own(self.header.color(base.pen_color)),
            line_type: Inherit::Own(self.line_type(base.pen_style)),
            line_type_scale: 1.0,
            line_weight: self.header.weight(base.pen_color, base.pen_width),
            transparency: Inherit::Layer,
        }
    }

    /// 線種を持たない図形（点・文字・ソリッド）の色
    fn fill_style(&self, color: Color) -> Style {
        Style {
            color: Inherit::Own(color),
            line_type: Inherit::Layer,
            line_type_scale: 1.0,
            line_weight: None,
            transparency: Inherit::Layer,
        }
    }

    /// 線種。番号を名前にし、線と間は見出しから読む。0 は 1 と同じ実線とする
    fn line_type(&mut self, code: u8) -> LineTypeId {
        let code = code.max(1);
        if let Some(id) = self.line_types.get(&code) {
            return *id;
        }
        let id = LineTypeId(u32::from(code));
        let header = &self.header;
        let (name, pattern) = match code {
            1 => (None, Some(Vec::new())),
            2..=9 => {
                let at = usize::from(code - 2);
                let read = header.patterns.get(at).copied().and_then(bit_pattern);
                (
                    None,
                    read.or_else(|| DEFAULT_DASHES.get(at).map(|d| dashes(d))),
                )
            }
            16..=19 => (
                None,
                header
                    .patterns
                    .get(usize::from(code - 16) + 8)
                    .copied()
                    .and_then(bit_pattern),
            ),
            code if code >= SXF_LINE_TYPE => {
                match header.sxf_line_types.get(usize::from(code - SXF_LINE_TYPE)) {
                    Some((name, pattern)) => (
                        Some(name.clone()).filter(|name| !name.is_empty()),
                        pattern.clone(),
                    ),
                    None => (None, None),
                }
            }
            // ランダム線の振れ幅は、線と間の列にできない
            _ => (None, None),
        };
        let description = if RANDOM_LINES.contains(&code) {
            "random"
        } else {
            ""
        };
        self.out.line_types.push(LineType {
            id,
            name: name.unwrap_or_else(|| code.to_string()),
            definition: pattern.map(|pattern| LineTypeDefinition {
                description: description.to_owned(),
                pattern,
                basis: PatternBasis::Paper,
                marks: Vec::new(),
            }),
            source: Ref::at_record(0),
        });
        self.line_types.insert(code, id);
        id
    }

    fn line(
        &mut self,
        r: &mut Reader,
        base: &Base,
    ) -> Result<Option<(Shape<ModelPoint>, Style)>, Fail> {
        let [x1, y1, x2, y2] = [r.f64()?, r.f64()?, r.f64()?, r.f64()?];
        let shape = Shape::Line {
            start: point(base.at, x1, y1),
            end: point(base.at, x2, y2),
        };
        Ok(Some((shape, self.line_style(base))))
    }

    /// 円・円弧・楕円・楕円弧。角度はラジアンで、開き角が負なら時計回り
    fn arc(
        &mut self,
        r: &mut Reader,
        base: &Base,
    ) -> Result<Option<(Shape<ModelPoint>, Style)>, Fail> {
        let [cx, cy, radius, start, sweep, tilt, flatness] = [
            r.f64()?,
            r.f64()?,
            r.f64()?,
            r.f64()?,
            r.f64()?,
            r.f64()?,
            r.f64()?,
        ];
        let full = r.u32()? != 0;
        let edge = self.ellipse(
            base.at,
            [cx, cy],
            radius,
            flatness,
            tilt,
            start,
            sweep,
            full,
        );
        let shape = match edge {
            Edge::Arc { center, radius, .. } if full => Shape::Circle { center, radius },
            edge => Shape::Curve(edge),
        };
        Ok(Some((shape, self.line_style(base))))
    }

    /// 円弧か楕円弧の辺。扁平率は短軸と半径の比で、1 より大きければ傾きの直交の向きを長軸にする
    #[expect(clippy::too_many_arguments)]
    fn ellipse(
        &self,
        at: At,
        [cx, cy]: [f64; 2],
        radius: f64,
        flatness: f64,
        tilt: f64,
        start: f64,
        sweep: f64,
        full: bool,
    ) -> Edge<ModelPoint> {
        let center = point(at, cx, cy);
        let radius = radius.abs();
        let flatness = if flatness.is_finite() && flatness > 0.0 {
            flatness
        } else {
            1.0
        };
        let (start, sweep) = if full { (0.0, TURN) } else { (start, sweep) };
        let counterclockwise = sweep >= 0.0;
        if (flatness - 1.0).abs() <= 1e-9 {
            return Edge::Arc {
                center,
                radius: length(at, radius),
                start: (tilt + start).to_degrees(),
                end: (tilt + start + sweep).to_degrees(),
                counterclockwise,
            };
        }
        // 楕円弧の角度は、媒介変数の角度とする（推定）
        let (axis, major, ratio, offset) = if flatness < 1.0 {
            (tilt, radius, flatness, 0.0)
        } else {
            (
                tilt + TURN / 4.0,
                radius * flatness,
                1.0 / flatness,
                TURN / 4.0,
            )
        };
        Edge::Elliptic {
            center,
            major: point(
                at,
                cx + major * libm::cos(axis),
                cy + major * libm::sin(axis),
            ),
            ratio,
            start: (start - offset).to_degrees(),
            end: (start + sweep - offset).to_degrees(),
            counterclockwise,
        }
    }

    /// 点。仮点は描かない。記号の点は番号を、ほかは `point` を、名前空間 `jww.point` のキーにする
    fn point(
        &mut self,
        r: &mut Reader,
        base: &Base,
    ) -> Result<Option<(Shape<ModelPoint>, Style)>, Fail> {
        let [x, y] = [r.f64()?, r.f64()?];
        let temporary = r.u32()? != 0;
        let (name, rotation, scale) = if base.pen_style == POINT_SYMBOL {
            let code = r.u32()?;
            // 記号の角度は度とする（推定）
            (code.to_string(), r.f64()?, r.f64()?)
        } else {
            ("point".to_owned(), 0.0, 1.0)
        };
        if temporary {
            return Ok(None);
        }
        let shape = Shape::Symbol(Symbol {
            key: SymbolKey {
                namespace: "jww.point",
                name,
            },
            position: point(base.at, x, y),
            scale: [scale; 2],
            rotation,
            grid: None,
            attributes: Vec::new(),
            clip: None,
        });
        Ok(Some((
            shape,
            self.fill_style(self.header.color(base.pen_color)),
        )))
    }

    /// 文字。始点を左下にし、高さは縦の大きさ（用紙の mm）とする。設定を書いた文字は、図面の設定にする
    fn text(
        &mut self,
        r: &mut Reader,
        base: &Base,
        source: &Ref,
    ) -> Result<Option<(Shape<ModelPoint>, Style)>, Fail> {
        let [x1, y1] = [r.f64()?, r.f64()?];
        r.skip(16)?;
        let kind = r.u32()?;
        let [size_x, size_y] = [r.f64()?, r.f64()?];
        let spacing = r.f64()?;
        let angle = r.f64()?;
        let (unicode, font) = r.string()?;
        let font = decode(unicode, font);
        let (unicode, content) = r.string()?;
        let content = decode(unicode, content);
        if [x1, y1] == SETTING_AT
            && let Some((key, value)) = setting(&content)
        {
            self.drafts.push(Draft {
                namespace: "jww.setting",
                name: key.to_owned(),
                version: Some(self.header.version.to_string()),
                schema: SCHEMA,
                at: Position::Document,
                fields: vec![Field {
                    key: "value",
                    code: None,
                    value: FieldValue::Text(value.to_owned()),
                }],
                source: source.clone(),
            });
            return Ok(None);
        }
        if let Some(image) = content.strip_prefix("^@BM") {
            return self.image(image, base, [x1, y1], angle, source).map(Some);
        }
        if content.starts_with("^@") {
            // 画像のほかの制御文字は、描かずに元のまま持つ
            self.drafts.push(Draft {
                namespace: "jww.control",
                name: content,
                version: Some(self.header.version.to_string()),
                schema: SCHEMA,
                at: Position::Document,
                fields: vec![Field {
                    key: "position",
                    code: None,
                    value: FieldValue::Point([x1, y1, 0.0]),
                }],
                source: source.clone(),
            });
            return Ok(None);
        }
        if kind != 0 || spacing != 0.0 {
            self.own.push((
                "jww.moji",
                vec![
                    Field {
                        key: "kind",
                        code: None,
                        value: FieldValue::Integer(i64::from(kind)),
                    },
                    Field {
                        key: "spacing",
                        code: None,
                        value: FieldValue::Real(spacing),
                    },
                ],
            ));
        }
        let shape = Shape::Text(Text {
            content,
            height: Height {
                value: Length::Paper(PaperLength(size_y)),
                meaning: HeightMeaning::Declared("jww.moji.size_y"),
            },
            placement: Placement::Anchor {
                point: point(base.at, x1, y1),
                horizontal: Horizontal::Left,
                vertical: Vertical::Bottom,
            },
            rotation: angle,
            // 全角の字の幅を横の大きさとする（推定）
            width_factor: if size_y > 0.0 { size_x / size_y } else { 1.0 },
            oblique: 0.0,
            backward: false,
            upside_down: false,
            style: font,
        });
        Ok(Some((
            shape,
            self.fill_style(self.header.color(base.pen_color)),
        )))
    }

    /// `^@BM` の文字の画像。`ファイル,幅,高さ` の幅と高さは用紙の mm で、文字の始点を左下の角にし、文字の角度で
    /// 回す（推定）。同梱画像を指す `%temp%` の名前は、同梱画像のレコードを定義にする
    fn image(
        &mut self,
        image: &str,
        base: &Base,
        [x, y]: [f64; 2],
        angle: f64,
        source: &Ref,
    ) -> Result<(Shape<ModelPoint>, Style), Fail> {
        let mut values = image.splitn(4, ',');
        let path = values.next().unwrap_or_default();
        let size = |value: Option<&str>| {
            value
                .and_then(|value| value.trim().parse::<f64>().ok())
                .filter(|value| value.is_finite() && *value > 0.0)
        };
        let (Some(width), Some(height)) = (size(values.next()), size(values.next())) else {
            return Err(Fail::Invalid);
        };
        if path.is_empty() {
            return Err(Fail::Invalid);
        }
        if let Some(extra) = values.next() {
            self.own.push((
                "jww.image",
                vec![Field {
                    key: "extra",
                    code: None,
                    value: FieldValue::Text(extra.to_owned()),
                }],
            ));
        }
        let definition = path
            .strip_prefix("%temp%")
            .and_then(|name| {
                self.images
                    .get(name)
                    .or_else(|| self.images.get(&format!("{name}.gz")))
            })
            .map_or_else(|| source.clone(), |&at| Ref::at_record(at as u64));
        let (sin, cos) = (libm::sin(angle.to_radians()), libm::cos(angle.to_radians()));
        let shape = Shape::Underlay(Underlay {
            kind: UnderlayKind::Image,
            resource: Resource {
                path: path.to_owned(),
                definition,
            },
            page: None,
            frame: UnderlayFrame::Corners {
                origin: point(base.at, x, y),
                right: point(base.at, x + width * cos, y + width * sin),
                top: point(base.at, x - height * sin, y + height * cos),
            },
            clip: None,
            visible: true,
        });
        Ok((shape, self.fill_style(self.header.color(base.pen_color))))
    }

    /// ソリッド。4 点は書いた順に結ぶ。線種が 101 からは円のソリッドとする
    fn solid(
        &mut self,
        r: &mut Reader,
        base: &Base,
    ) -> Result<Option<(Shape<ModelPoint>, Style)>, Fail> {
        let mut values = [0.0; 8];
        for value in &mut values {
            *value = r.f64()?;
        }
        let color = if base.pen_color == SOLID_OWN_COLOR {
            Color {
                code: None,
                rgb: Some(colorref(r.u32()?)),
            }
        } else {
            self.header.color(base.pen_color)
        };
        let at = base.at;
        let boundary = if base.pen_style >= CIRCLE_SOLID {
            let [cx, cy, radius, flatness, tilt, start, sweep, mode] = values;
            let full = mode == 100.0 || (sweep.abs() - TURN).abs() <= 1e-9;
            let arc = self.ellipse(at, [cx, cy], radius, flatness, tilt, start, sweep, full);
            let mut edges = Vec::new();
            if !full {
                let [first, last] = ends(&arc).ok_or(Fail::Invalid)?;
                let center = point(at, cx, cy);
                // 弓形（-1 と 5）は弦で、ほかは扇形として中心を通って閉じる（推定）
                if mode == -1.0 || mode == 5.0 {
                    edges.push(arc);
                    edges.push(Edge::Line {
                        start: last,
                        end: first,
                    });
                } else {
                    edges.push(Edge::Line {
                        start: center,
                        end: first,
                    });
                    edges.push(arc);
                    edges.push(Edge::Line {
                        start: last,
                        end: center,
                    });
                }
            } else {
                edges.push(arc);
            }
            Boundary::Edges(edges)
        } else {
            let vertices = values
                .as_chunks::<2>()
                .0
                .iter()
                .map(|&[x, y]| Vertex {
                    point: point(at, x, y),
                    bulge: 0.0,
                })
                .collect();
            Boundary::Polyline {
                vertices,
                closed: true,
            }
        };
        let shape = Shape::Hatch(Hatch {
            loops: vec![HatchLoop {
                boundary,
                external: true,
                outermost: true,
                derived: false,
                text_box: false,
                sources: Vec::new(),
            }],
            islands: Islands::Nested,
            fill: Fill::Solid,
            associative: false,
            seeds: Vec::new(),
        });
        Ok(Some((shape, self.fill_style(color))))
    }

    /// 用紙と、レイヤグループごとの空間とビュー
    fn finish(mut self) -> Drawing2D {
        let size = PAPERS
            .iter()
            .find(|(code, _)| *code == self.header.paper)
            .map(|(_, size)| *size);
        let [ox, oy] = size.map_or([0.0, 0.0], |[w, h]| [w / 2.0, h / 2.0]);
        let mut sheet = Sheet {
            id: SHEET,
            name: None,
            size: size.map(|[w, h]| [PaperLength(w), PaperLength(h)]),
            elements: Vec::new(),
            views: Vec::new(),
            order: Vec::new(),
            source: Ref::default(),
        };
        for (group, elements) in std::mem::take(&mut self.groups) {
            let id = SpaceId(group as u32);
            let k = 1.0 / self.header.scale(group);
            sheet.order.push(SheetItem::View(sheet.views.len()));
            sheet.views.push(View {
                space: id,
                sheet: SHEET,
                transform: Affine {
                    m: [k, 0.0, ox, 0.0, k, oy],
                },
                clip: Vec::new(),
                hidden_layers: Vec::new(),
                source: Ref::at_record(0),
            });
            self.out.spaces.push(ModelSpace {
                id,
                unit: Unit::Millimeter,
                order: (0..elements.len()).collect(),
                elements,
            });
        }
        self.out.sheets.push(sheet);
        self.out
    }
}

/// 円弧と楕円弧の始まりと終わりの点
fn ends(edge: &Edge<ModelPoint>) -> Option<[ModelPoint; 2]> {
    match *edge {
        Edge::Arc {
            center,
            radius,
            start,
            end,
            ..
        } => {
            let r = match radius {
                Length::Model(length) => length.value,
                Length::Paper(length) => length.0,
            };
            let at = |deg: f64| ModelPoint {
                space: center.space,
                x: center.x + r * libm::cos(deg.to_radians()),
                y: center.y + r * libm::sin(deg.to_radians()),
            };
            Some([at(start), at(end)])
        }
        Edge::Elliptic {
            center,
            major,
            ratio,
            start,
            end,
            ..
        } => {
            let [ax, ay] = [major.x - center.x, major.y - center.y];
            let [bx, by] = [-ay * ratio, ax * ratio];
            let at = |deg: f64| {
                let (s, c) = (libm::sin(deg.to_radians()), libm::cos(deg.to_radians()));
                ModelPoint {
                    space: center.space,
                    x: center.x + ax * c + bx * s,
                    y: center.y + ay * c + by * s,
                }
            };
            Some([at(start), at(end)])
        }
        Edge::Line { start, end } => Some([start, end]),
        Edge::Spline(_) | Edge::Clothoid { .. } => None,
    }
}

/// Windows の COLORREF（0x00BBGGRR）を RGB にする
fn colorref(value: u32) -> [u8; 3] {
    let [r, g, b, _] = value.to_le_bytes();
    [r, g, b]
}

/// `名前 = 値` の設定。名前は英字で始まり、英数字と `_` が続く
fn setting(content: &str) -> Option<(&str, &str)> {
    let (key, value) = content.split_once(" = ")?;
    let mut chars = key.chars();
    let valid = chars.next().is_some_and(|c| c.is_ascii_alphabetic())
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_');
    valid.then_some((key, value))
}

fn dropped(cx: &mut Cx, kind: &[u8], source: Ref) {
    cx.report(
        Diagnostic::new(codes::JWW_LIFT_DROPPED)
            .with("kind", String::from_utf8_lossy(kind).into_owned())
            .at(source),
    );
}

fn invalid(cx: &mut Cx, kind: &[u8], source: Ref) {
    cx.report(
        Diagnostic::new(codes::JWW_LIFT_INVALID)
            .with("kind", String::from_utf8_lossy(kind).into_owned())
            .at(source),
    );
}
