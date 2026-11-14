//! 図形のレコードを、列に置く値へ持ち上げる。要素になる条件は、`zumen_acad::single` の共通関数をそのまま呼んで
//! 一括変換の読み方（`crates/zumen-acad/src/drawing`）と 1 つの実装に揃える。
//! 開くときも、変更のあとで読み直すときも、ここを通る。

use zumen_acad::single::{
    self, DxfFields, FieldSource as _, PolylineSink, RawLineType, RawTextPlace,
};
pub use zumen_acad::single::{Follow, Followed, RawTag};
use zumen_drawing::{
    Attribute, Color, Edge, Fit, Grid, Height, HeightMeaning, Horizontal, Inherit, LayerId, Length,
    ModelLength, ModelPoint, Placement, Polyline, Shape, SpaceId, Style, Symbol, SymbolKey, Text,
    Vertex, Vertical,
};

/// この試作が要素として持つ図形の種類
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[repr(u8)]
pub enum Kind {
    /// LINE
    Line = 0,
    /// CIRCLE
    Circle = 1,
    /// ARC
    Arc = 2,
    /// LWPOLYLINE
    Polyline = 3,
    /// TEXT
    Text = 4,
    /// INSERT（ブロックの挿入）。付いた ATTRIB は、この行の属性として持つ
    Insert = 5,
}

impl Kind {
    /// すべての種類
    pub const ALL: [Kind; 6] = [
        Kind::Line,
        Kind::Circle,
        Kind::Arc,
        Kind::Polyline,
        Kind::Text,
        Kind::Insert,
    ];

    /// レコードの種類（グループ・コード 0 の値。前後の空白を除いたもの）から
    pub fn of(name: &[u8]) -> Option<Kind> {
        Some(match name {
            b"LINE" => Kind::Line,
            b"CIRCLE" => Kind::Circle,
            b"ARC" => Kind::Arc,
            b"LWPOLYLINE" => Kind::Polyline,
            b"TEXT" => Kind::Text,
            b"INSERT" => Kind::Insert,
            _ => return None,
        })
    }

    /// DXF のグループ・コード 0 の種類名
    pub fn dxf_name(self) -> &'static [u8] {
        match self {
            Kind::Line => b"LINE",
            Kind::Circle => b"CIRCLE",
            Kind::Arc => b"ARC",
            Kind::Polyline => b"LWPOLYLINE",
            Kind::Text => b"TEXT",
            Kind::Insert => b"INSERT",
        }
    }
}

/// 折れ線の閉じる指定（70 の 1）
pub const POLY_CLOSED: u8 = 1;
/// 折れ線の線種の生成（70 の 128）
pub const POLY_GENERATION: u8 = 2;
/// 折れ線が頂点ごとの幅を持つ
pub const POLY_WIDTHS: u8 = 4;

/// 文字の置き方
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Place {
    /// 1 点に揃える
    Anchor {
        /// 揃える点
        point: [f64; 2],
        /// 横の揃え
        horizontal: Horizontal,
        /// 縦の揃え
        vertical: Vertical,
    },
    /// 2 点の間に収める
    Between {
        /// 1 点め
        first: [f64; 2],
        /// 2 点め
        second: [f64; 2],
        /// 収め方
        fit: Fit,
    },
}

/// 文字の左右を反転（71 の 2）
pub const TEXT_BACKWARD: u8 = 1;
/// 文字の上下を反転（71 の 4）
pub const TEXT_UPSIDE_DOWN: u8 = 2;

/// 文字の行
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TextRow {
    /// 置き方
    pub place: Place,
    /// 高さ（40）
    pub height: f64,
    /// 回転（50、度）
    pub rotation: f64,
    /// 幅の係数（41）
    pub width_factor: f64,
    /// 傾き（51、度）
    pub oblique: f64,
    /// `TEXT_*`
    pub flags: u8,
    /// 中身（1）。置き場の中の範囲。符号化のまま
    pub content: (u32, u32),
    /// 文字スタイルの名前（7）。置き場の中の範囲。符号化のまま
    pub style: Option<(u32, u32)>,
}

/// 定義が見つからない記号の、定義の番号
pub const NO_DEFINITION: u32 = u32::MAX;

/// 記号（INSERT）の行
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct InsertRow {
    /// 定義の名前（2）。置き場の中の範囲。符号化のまま
    pub name: (u32, u32),
    /// 定義の番号。文書に置くときに名前から引く。なければ `NO_DEFINITION`
    pub definition: u32,
    /// 置く点（10・20）
    pub position: [f64; 2],
    /// 尺度（41・42）
    pub scale: [f64; 2],
    /// 回転（50、度）
    pub rotation: f64,
    /// 列と行の数（70・71）。どちらも 1 なら、格子に並べない
    pub grid: [u32; 2],
    /// 列と行の間隔（44・45）。格子に並べないときは 0
    pub spacing: [f64; 2],
    /// 属性。`Pools::attribs` の中の最初の番号と数
    pub attribs: (u32, u32),
    /// 付いた ATTRIB のレコードの数。読めなかったものも数える
    pub attached: u32,
    /// 66 が 0 でない整数。今の実装は、この記号を除く変更を受け付けない
    pub followed: bool,
}

/// 記号の属性（ATTRIB）
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AttribRow {
    /// レコードの番号
    pub record: u32,
    /// 文字
    pub text: TextRow,
    /// タグ（2）。置き場の中の範囲。符号化のまま
    pub tag: (u32, u32),
    /// 表示しない（70 の 1）
    pub hidden: bool,
    /// 画層の名前（8）。置き場の中の範囲。符号化のまま
    pub layer_name: (u32, u32),
    /// 画層の番号。文書に置くときに名前から引く
    pub layer: u32,
    /// 色の指定（`color`）
    pub color: u32,
    /// 文字の値の種類（`class`）
    pub class: u8,
}

/// レコードの種類とタグから、親に付けるために見る値を読む。INSERT・POLYLINE・ATTRIB・VERTEX・SEQEND で
/// なければ `None`
pub fn followed(name: &[u8], tags: &[RawTag<'_>]) -> Option<Followed> {
    single::followed_tags(name, tags)
}

/// 持ち上げた形。折れ線の頂点と文字の中身は、渡した置き場に足す
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Geom {
    /// 始点と終点
    Line([f64; 4]),
    /// 中心と半径
    Circle([f64; 3]),
    /// 中心・半径・始まりと終わりの角（度、反時計回り）
    Arc([f64; 5]),
    /// 折れ線
    Polyline {
        /// 置き場の中の最初の頂点
        first: u32,
        /// 頂点の数
        count: u32,
        /// `POLY_*`
        flags: u8,
        /// 一定の幅（43）。0 は、なし
        width: f64,
    },
    /// 文字
    Text(TextRow),
    /// 記号
    Insert(InsertRow),
}

/// 折れ線の頂点と、文字の中身の置き場
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Pools {
    /// 記号の属性。記号ごとに、続けて並ぶ
    pub attribs: Vec<AttribRow>,
    /// 頂点（x、y、膨らみ）
    pub vertices: Vec<[f64; 3]>,
    /// 頂点ごとの幅（始まり、終わり）。幅を持つ折れ線の頂点だけが、`vertices` と同じ数だけ並ぶ
    pub widths: Vec<[f64; 2]>,
    /// 幅を持つ折れ線ごとの、`widths` の中の最初の番号。`Geom::Polyline` の `first` から引く
    pub width_first: Vec<(u32, u32)>,
    /// 文字の中身と文字スタイルの名前
    pub text: Vec<u8>,
}

impl Pools {
    /// 折れ線 `first` の頂点ごとの幅
    pub fn widths_of(&self, first: u32, count: u32) -> &[[f64; 2]] {
        let Ok(at) = self
            .width_first
            .binary_search_by_key(&first, |&(key, _)| key)
        else {
            return &[];
        };
        let start = self.width_first.get(at).map_or(0, |&(_, start)| start) as usize;
        self.widths
            .get(start..start + count as usize)
            .unwrap_or_default()
    }

    /// 文字の置き場の中の範囲
    pub fn text_of(&self, (start, end): (u32, u32)) -> &[u8] {
        self.text
            .get(start as usize..end as usize)
            .unwrap_or_default()
    }

    /// 折れ線 `first` の頂点ごとの幅を、書き換えるために借りる
    pub fn widths_mut(&mut self, first: u32, count: u32) -> &mut [[f64; 2]] {
        let Ok(at) = self
            .width_first
            .binary_search_by_key(&first, |&(key, _)| key)
        else {
            return &mut [];
        };
        let start = self.width_first.get(at).map_or(0, |&(_, start)| start) as usize;
        self.widths
            .get_mut(start..start + count as usize)
            .unwrap_or_default()
    }

    /// 文字を足して、範囲を返す
    pub fn push_text(&mut self, bytes: &[u8]) -> Option<(u32, u32)> {
        let start = u32::try_from(self.text.len()).ok()?;
        self.text.extend_from_slice(bytes);
        Some((start, u32::try_from(self.text.len()).ok()?))
    }

    /// 記号の属性
    pub fn attribs_of(&self, row: &InsertRow) -> &[AttribRow] {
        let (first, count) = row.attribs;
        self.attribs
            .get(first as usize..first as usize + count as usize)
            .unwrap_or_default()
    }

    /// 足したものを除くための、今の長さ
    pub fn marks(&self) -> [usize; 5] {
        [
            self.vertices.len(),
            self.widths.len(),
            self.width_first.len(),
            self.text.len(),
            self.attribs.len(),
        ]
    }

    /// `marks` のときの長さに戻す
    pub fn truncate(&mut self, marks: [usize; 5]) {
        self.vertices.truncate(marks[0]);
        self.widths.truncate(marks[1]);
        self.width_first.truncate(marks[2]);
        self.text.truncate(marks[3]);
        self.attribs.truncate(marks[4]);
    }

    /// 空にする
    pub fn clear(&mut self) {
        self.truncate([0; 5]);
    }
}

/// 色の指定。上位のバイトが種類
pub mod color {
    /// 色番号（下位 8 ビット）
    pub const INDEX: u32 = 0;
    /// RGB（下位 24 ビット）
    pub const RGB: u32 = 1 << 24;
    /// 画層に従う
    pub const BY_LAYER: u32 = 2 << 24;
    /// ブロックに従う
    pub const BY_BLOCK: u32 = 3 << 24;
}

/// 文字の値の種類。文字コードが決まってから、要素にするかを決める
pub mod class {
    /// ASCII だけ
    pub const ASCII: u8 = 0;
    /// ASCII でない文字を含む、正しい UTF-8
    pub const UTF8: u8 = 1;
    /// UTF-8 として読めない
    pub const OTHER: u8 = 2;
}

/// 持ち上げた行
#[derive(Clone, Debug, PartialEq)]
pub struct Lifted<'a> {
    /// ハンドル（5 の、16 進として読める最初の値）
    pub handle: Option<u64>,
    /// 形
    pub geom: Geom,
    /// 画層の名前（8）。符号化のまま
    pub layer: &'a [u8],
    /// 色の指定（`color`）
    pub color: u32,
    /// 文字の値の種類（`class`）
    pub class: u8,
}

/// 持ち上げの結果
#[derive(Clone, Debug, PartialEq)]
pub enum Lift<'a> {
    /// モデル空間の要素になる
    Element(Lifted<'a>),
    /// ペーパー空間（67 = 1）。この試作は読まない
    Paper,
    /// 要素にならない（今の実装でも要素にならない）
    Not,
}

/// 文字の値の種類
fn class_of(value: &[u8]) -> u8 {
    if value.is_ascii() {
        class::ASCII
    } else if core::str::from_utf8(value).is_ok() {
        class::UTF8
    } else {
        class::OTHER
    }
}

/// 図形のレコードを持ち上げる。`tags` は、グループ・コード 0 の次からのタグ。
/// 折れ線の頂点は `pools` に足す（要素にならなければ、足さない）
pub fn lift<'a>(kind: Kind, tags: &[RawTag<'a>], pools: &mut Pools) -> Lift<'a> {
    let fields = DxfFields::new(tags);
    match fields.int(67, 0) {
        Ok(1) => return Lift::Paper,
        Ok(_) => {}
        Err(_) => return Lift::Not,
    }
    let marks = pools.marks();
    match lifted(kind, tags, &fields, pools) {
        Some(lifted) => Lift::Element(lifted),
        None => {
            pools.truncate(marks);
            Lift::Not
        }
    }
}

fn lifted<'a>(
    kind: Kind,
    tags: &[RawTag<'a>],
    fields: &DxfFields<'a>,
    pools: &mut Pools,
) -> Option<Lifted<'a>> {
    single::check_upright(fields).ok()?;
    let mut class = class::ASCII;
    let geom = match kind {
        Kind::Line => Geom::Line(single::line_ends(fields).ok()?),
        Kind::Circle => Geom::Circle(single::circle_geom(fields).ok()?),
        Kind::Arc => Geom::Arc(single::arc_geom(fields).ok()?),
        Kind::Polyline => polyline(tags, fields, pools)?,
        Kind::Text => {
            let (row, text_class) = text(fields, pools, 73)?;
            class = class.max(text_class);
            Geom::Text(row)
        }
        Kind::Insert => {
            let (row, name_class) = insert(fields, pools)?;
            class = class.max(name_class);
            Geom::Insert(row)
        }
    };
    let (layer, color, style_class) = styled(fields)?;
    Some(Lifted {
        handle: fields.handle,
        geom,
        layer,
        color,
        class: class.max(style_class),
    })
}

/// 画層・色・線種・線種の倍率・線幅・透過性を確かめて、画層の名前・色の指定・文字の値の種類を返す
fn styled<'a>(fields: &DxfFields<'a>) -> Option<(&'a [u8], u32, u8)> {
    let es = single::entity_style(fields).ok()?;
    let color = match es.color {
        Inherit::Own(Color {
            rgb: Some([r, g, b]),
            ..
        }) => color::RGB | u32::from_be_bytes([0, r, g, b]),
        Inherit::Own(Color {
            code: Some(code),
            rgb: None,
        }) => color::INDEX | code.value,
        Inherit::Own(Color {
            code: None,
            rgb: None,
        }) => return None,
        Inherit::Block => color::BY_BLOCK,
        Inherit::Layer => color::BY_LAYER,
    };
    let mut class = match es.line_type {
        RawLineType::Named(bytes) => class_of(bytes),
        RawLineType::Layer | RawLineType::Block => fields.raw(6).map_or(class::ASCII, class_of),
    };
    let layer = fields.raw_text(8).ok()??;
    class = class.max(class_of(layer));
    Some((layer, color, class))
}

/// 記号（INSERT）。定義がなくても要素になる
fn insert(fields: &DxfFields<'_>, pools: &mut Pools) -> Option<(InsertRow, u8)> {
    let raw = single::insert_geom(fields).ok()?;
    let class = class_of(raw.name);
    Some((
        InsertRow {
            name: pools.push_text(raw.name)?,
            definition: NO_DEFINITION,
            position: raw.position,
            scale: raw.scale,
            rotation: raw.rotation,
            grid: raw.grid,
            spacing: raw.spacing,
            attribs: (0, 0),
            attached: 0,
            followed: raw.followed,
        },
        class,
    ))
}

/// ATTRIB を、記号の属性として持ち上げる。空間（67）は見ない。書式つきの中身（101）を持つ属性は読まない
/// （今の実装は、その属性を持つ記号を動かす変更を受け付けない）。読めなければ、置き場に何も残さない
pub fn attrib(tags: &[RawTag<'_>], record: u32, pools: &mut Pools) -> Option<AttribRow> {
    let fields = DxfFields::new(tags);
    let marks = pools.marks();
    let row = attribute(&fields, record, pools);
    if row.is_none() {
        pools.truncate(marks);
    }
    row
}

fn attribute(fields: &DxfFields<'_>, record: u32, pools: &mut Pools) -> Option<AttribRow> {
    single::check_upright(fields).ok()?;
    if fields.has_code(101) {
        return None;
    }
    let (text, text_class) = text(fields, pools, 74)?;
    let tag = fields.raw_text(2).ok()??;
    let hidden = fields.int(70, 0).ok()? & 1 != 0;
    let (layer, color, style_class) = styled(fields)?;
    Some(AttribRow {
        record,
        text,
        tag: pools.push_text(tag)?,
        hidden,
        layer_name: pools.push_text(layer)?,
        layer: 0,
        color,
        class: text_class.max(class_of(tag)).max(style_class),
    })
}

struct PoolPolySink<'p> {
    pools: &'p mut Pools,
    first: usize,
    widths: Vec<[Option<f64>; 2]>,
    any_width: bool,
}

impl PolylineSink for PoolPolySink<'_> {
    fn len(&self) -> usize {
        self.pools.vertices.len() - self.first
    }

    fn push_vertex(&mut self, x: f64, y: f64) {
        self.pools.vertices.push([x, y, 0.0]);
        if self.any_width {
            self.widths.push([None; 2]);
        }
    }

    fn set_width(&mut self, is_end: bool, width: f64) {
        let count = self.len();
        if !self.any_width {
            self.any_width = true;
            self.widths.resize(count, [None; 2]);
        }
        if let Some(slot) = self.widths.last_mut() {
            slot[usize::from(is_end)] = Some(width);
        }
    }

    fn set_bulge(&mut self, bulge: f64) {
        if let Some(vertex) = self.pools.vertices.last_mut() {
            vertex[2] = bulge;
        }
    }
}

fn polyline(tags: &[RawTag<'_>], fields: &DxfFields<'_>, pools: &mut Pools) -> Option<Geom> {
    let first = pools.vertices.len();
    let mut sink = PoolPolySink {
        pools,
        first,
        widths: Vec::new(),
        any_width: false,
    };
    let meta = single::lwpolyline_scan(fields, tags, &mut sink).ok()?;
    let count = sink.len();
    let any_width = sink.any_width;
    let widths = sink.widths;
    let mut flags = 0;
    if meta.closed {
        flags |= POLY_CLOSED;
    }
    if meta.line_type_generation {
        flags |= POLY_GENERATION;
    }
    if any_width {
        flags |= POLY_WIDTHS;
        pools.width_first.push((
            u32::try_from(first).ok()?,
            u32::try_from(pools.widths.len()).ok()?,
        ));
        pools.widths.extend(
            widths
                .iter()
                .map(|[start, end]| [start.unwrap_or(0.0), end.unwrap_or(0.0)]),
        );
    }
    Some(Geom::Polyline {
        first: u32::try_from(first).ok()?,
        count: u32::try_from(count).ok()?,
        flags,
        width: meta.width.unwrap_or(0.0),
    })
}

/// 文字。縦の揃えのグループ・コードは、TEXT では 73、ATTRIB では 74
fn text(fields: &DxfFields<'_>, pools: &mut Pools, vertical: i16) -> Option<(TextRow, u8)> {
    let raw = single::text_geom(fields, vertical).ok()?;
    let place = match raw.place {
        RawTextPlace::Anchor {
            point,
            horizontal,
            vertical,
        } => Place::Anchor {
            point,
            horizontal,
            vertical,
        },
        RawTextPlace::Between { first, second, fit } => Place::Between { first, second, fit },
    };
    let class = class_of(raw.content).max(raw.style.map_or(class::ASCII, class_of));
    let mut bits = 0;
    if raw.backward {
        bits |= TEXT_BACKWARD;
    }
    if raw.upside_down {
        bits |= TEXT_UPSIDE_DOWN;
    }
    let content = pools.push_text(raw.content)?;
    let style = match raw.style {
        Some(name) => Some(pools.push_text(name)?),
        None => None,
    };
    Some((
        TextRow {
            place,
            height: raw.height,
            rotation: raw.rotation,
            width_factor: raw.width_factor,
            oblique: raw.oblique,
            flags: bits,
            content,
            style,
        },
        class,
    ))
}

const MODEL: SpaceId = SpaceId(0);

fn point([x, y]: [f64; 2]) -> ModelPoint {
    ModelPoint { space: MODEL, x, y }
}

fn length(value: f64) -> Length {
    Length::Model(ModelLength {
        space: MODEL,
        value,
    })
}

/// 列の値を、図面モデルの形にする。文字は UTF-8 として読めること（読めなければ `None`）
pub fn shape(geom: &Geom, pools: &Pools) -> Option<Shape<ModelPoint>> {
    Some(match geom {
        Geom::Line([x0, y0, x1, y1]) => Shape::Line {
            start: point([*x0, *y0]),
            end: point([*x1, *y1]),
        },
        Geom::Circle([x, y, radius]) => Shape::Circle {
            center: point([*x, *y]),
            radius: length(*radius),
        },
        Geom::Arc([x, y, radius, start, end]) => Shape::Curve(Edge::Arc {
            center: point([*x, *y]),
            radius: length(*radius),
            start: *start,
            end: *end,
            counterclockwise: true,
        }),
        Geom::Polyline {
            first,
            count,
            flags,
            width,
        } => {
            let range = *first as usize..(*first as usize + *count as usize);
            let vertices = pools
                .vertices
                .get(range)?
                .iter()
                .map(|[x, y, bulge]| Vertex {
                    point: point([*x, *y]),
                    bulge: *bulge,
                })
                .collect();
            let vertex_widths = if flags & POLY_WIDTHS != 0 {
                pools
                    .widths_of(*first, *count)
                    .iter()
                    .map(|[start, end]| [length(*start), length(*end)])
                    .collect()
            } else {
                Vec::new()
            };
            Shape::Polyline(Polyline {
                vertices,
                closed: flags & POLY_CLOSED != 0,
                width: (*width != 0.0).then(|| length(*width)),
                vertex_widths,
                line_type_generation: flags & POLY_GENERATION != 0,
            })
        }
        Geom::Text(row) => Shape::Text(lettering(row, pools, "dxf.TEXT.40")?),
        Geom::Insert(row) => Shape::Symbol(Symbol {
            key: SymbolKey {
                namespace: BLOCK.into(),
                name: core::str::from_utf8(pools.text_of(row.name))
                    .ok()?
                    .to_owned(),
            },
            position: point(row.position),
            scale: row.scale,
            rotation: row.rotation,
            grid: (row.grid != [1, 1]).then(|| Grid {
                columns: row.grid[0],
                rows: row.grid[1],
                column_spacing: length(row.spacing[0]),
                row_spacing: length(row.spacing[1]),
            }),
            attributes: pools
                .attribs_of(row)
                .iter()
                .map(|attrib| {
                    Some(Attribute {
                        tag: core::str::from_utf8(pools.text_of(attrib.tag))
                            .ok()?
                            .to_owned(),
                        text: lettering(&attrib.text, pools, "dxf.ATTRIB.40")?,
                        formatted: None,
                        hidden: attrib.hidden,
                        layer: LayerId(attrib.layer),
                        // 属性のスタイルは、動かす変更では使わない
                        style: Style {
                            color: Inherit::Layer,
                            line_type: Inherit::Layer,
                            line_type_scale: 1.0,
                            line_weight: None,
                            transparency: Inherit::Layer,
                        },
                        source: Default::default(),
                    })
                })
                .collect::<Option<Vec<_>>>()?,
            clip: None,
        }),
    })
}

/// 記号の定義の名前空間。今の実装の `BLOCK`（`crates/zumen-acad/src/drawing.rs`）と同じ
pub const BLOCK: &str = "dxf.block";

/// 文字の行を、図面モデルの文字にする。`meaning` は、高さの意味
fn lettering(row: &TextRow, pools: &Pools, meaning: &'static str) -> Option<Text<ModelPoint>> {
    Some(Text {
        content: core::str::from_utf8(pools.text_of(row.content))
            .ok()?
            .to_owned(),
        height: Height {
            value: length(row.height),
            meaning: HeightMeaning::Declared(meaning.into()),
        },
        placement: match row.place {
            Place::Anchor {
                point: at,
                horizontal,
                vertical,
            } => Placement::Anchor {
                point: point(at),
                horizontal,
                vertical,
            },
            Place::Between { first, second, fit } => Placement::Between {
                first: point(first),
                second: point(second),
                fit,
            },
        },
        rotation: row.rotation,
        width_factor: row.width_factor,
        oblique: row.oblique,
        backward: row.flags & TEXT_BACKWARD != 0,
        upside_down: row.flags & TEXT_UPSIDE_DOWN != 0,
        style: match row.style {
            Some(range) => core::str::from_utf8(pools.text_of(range)).ok()?.to_owned(),
            None => "STANDARD".to_owned(),
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tags(pairs: &[(i16, &'static str)]) -> Vec<RawTag<'static>> {
        pairs
            .iter()
            .map(|&(code, value)| RawTag {
                code,
                value: value.as_bytes(),
            })
            .collect()
    }

    fn element(
        kind: Kind,
        pairs: &[(i16, &'static str)],
        pools: &mut Pools,
    ) -> Option<Lifted<'static>> {
        match lift(kind, &tags(pairs), pools) {
            Lift::Element(lifted) => Some(lifted),
            _ => None,
        }
    }

    #[test]
    fn a_line_needs_both_ends_and_a_layer() {
        let mut pools = Pools::default();
        let line = [
            (5, "2A"),
            (8, "0"),
            (10, "1.0"),
            (20, "2"),
            (30, "0.0"),
            (11, "3"),
            (21, "4"),
        ];
        let lifted = element(Kind::Line, &line, &mut pools).expect("線");
        assert_eq!(lifted.handle, Some(0x2A));
        assert_eq!(lifted.geom, Geom::Line([1.0, 2.0, 3.0, 4.0]));
        assert_eq!(lifted.color, color::BY_LAYER);
        // 画層がない・Z が 0 でない・押し出しの向きが既定でないものは、要素にしない
        assert!(element(Kind::Line, &line[..1], &mut pools).is_none());
        let mut high = line.to_vec();
        high[4] = (30, "1.0");
        assert!(element(Kind::Line, &high, &mut pools).is_none());
        let mut tilted = line.to_vec();
        tilted.push((230, "-1.0"));
        assert!(element(Kind::Line, &tilted, &mut pools).is_none());
        let mut paper = line.to_vec();
        paper.push((67, "1"));
        assert_eq!(lift(Kind::Line, &tags(&paper), &mut pools), Lift::Paper);
    }

    #[test]
    fn a_polyline_counts_its_vertices() {
        let mut pools = Pools::default();
        let square = [
            (8, "0"),
            (90, "3"),
            (70, "129"),
            (43, "0.0"),
            (10, "0"),
            (20, "0"),
            (10, "2"),
            (20, "0"),
            (42, "0.5"),
            (10, "2"),
            (20, "2"),
        ];
        let lifted = element(Kind::Polyline, &square, &mut pools).expect("折れ線");
        assert_eq!(
            lifted.geom,
            Geom::Polyline {
                first: 0,
                count: 3,
                flags: POLY_CLOSED | POLY_GENERATION,
                width: 0.0
            }
        );
        assert_eq!(
            pools.vertices,
            [[0.0, 0.0, 0.0], [2.0, 0.0, 0.5], [2.0, 2.0, 0.0]]
        );
        // 頂点の数（90）が合わなければ要素にせず、置き場にも残さない
        let mut short = square.to_vec();
        short[1] = (90, "4");
        assert!(element(Kind::Polyline, &short, &mut pools).is_none());
        assert_eq!(pools.vertices.len(), 3);
        // 幅を持つ頂点があれば、ほかの頂点の幅は 0
        let mut wide = square.to_vec();
        wide.insert(8, (40, "0.25"));
        let lifted = element(Kind::Polyline, &wide, &mut pools).expect("幅つき");
        let Geom::Polyline {
            first,
            count,
            flags,
            ..
        } = lifted.geom
        else {
            panic!("折れ線");
        };
        assert_ne!(flags & POLY_WIDTHS, 0);
        assert_eq!(
            pools.widths_of(first, count),
            [[0.0, 0.0], [0.25, 0.0], [0.0, 0.0]]
        );
    }

    #[test]
    fn text_alignment_picks_the_point() {
        let mut pools = Pools::default();
        let base = [(8, "0"), (10, "1"), (20, "2"), (40, "0.5"), (1, "abc")];
        let lifted = element(Kind::Text, &base, &mut pools).expect("文字");
        let Geom::Text(row) = &lifted.geom else {
            panic!("文字");
        };
        assert_eq!(pools.text_of(row.content), b"abc");
        assert_eq!(row.style, None);
        assert_eq!(
            row.place,
            Place::Anchor {
                point: [1.0, 2.0],
                horizontal: Horizontal::Left,
                vertical: Vertical::Baseline
            }
        );
        let mut centered = base.to_vec();
        centered.extend([(72, "1"), (11, "5"), (21, "6")]);
        let lifted = element(Kind::Text, &centered, &mut pools).expect("中央");
        let Geom::Text(row) = &lifted.geom else {
            panic!("文字");
        };
        assert_eq!(
            row.place,
            Place::Anchor {
                point: [5.0, 6.0],
                horizontal: Horizontal::Center,
                vertical: Vertical::Baseline
            }
        );
        // 揃えがあって 2 点めがなければ、要素にしない
        let mut missing = base.to_vec();
        missing.push((72, "1"));
        assert!(element(Kind::Text, &missing, &mut pools).is_none());
        let mut japanese = base.to_vec();
        japanese[4] = (1, "図面");
        assert_eq!(
            element(Kind::Text, &japanese, &mut pools)
                .expect("日本語")
                .class,
            class::UTF8
        );
    }

    #[test]
    fn colors_follow_62_and_420() {
        let mut pools = Pools::default();
        let base = [(8, "0"), (10, "0"), (20, "0"), (40, "1")];
        let with = |extra: &[(i16, &'static str)], pools: &mut Pools| {
            let mut all = base.to_vec();
            all.extend_from_slice(extra);
            element(Kind::Circle, &all, pools).map(|lifted| lifted.color)
        };
        assert_eq!(with(&[(62, "1")], &mut pools), Some(color::INDEX | 1));
        assert_eq!(with(&[(62, "0")], &mut pools), Some(color::BY_BLOCK));
        assert_eq!(with(&[(420, "255")], &mut pools), Some(color::RGB | 255));
        assert_eq!(with(&[(62, "257")], &mut pools), None);
        assert_eq!(with(&[(62, "-1")], &mut pools), None);
        assert_eq!(with(&[(370, "212")], &mut pools), None);
        assert_eq!(with(&[(370, "-3")], &mut pools), Some(color::BY_LAYER));
        assert_eq!(with(&[(48, "0.0")], &mut pools), None);
        assert_eq!(with(&[(440, "50331648")], &mut pools), None);
    }
}
