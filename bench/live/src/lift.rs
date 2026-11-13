//! 図形のレコードを、列に置く値へ持ち上げる。要素になる条件は、今の実装の読み方
//! （`crates/zumen-acad/src/drawing.rs`・`drawing/geometry.rs`・`drawing/style.rs`）と同じにする。
//! 開くときも、変更のあとで読み直すときも、ここを通る。

use zumen_drawing::{
    Attribute, Edge, Fit, Grid, Height, HeightMeaning, Horizontal, Inherit, LayerId, Length,
    ModelLength, ModelPoint, Placement, Polyline, Shape, SpaceId, Style, Symbol, SymbolKey, Text,
    Vertex, Vertical,
};

use crate::value::{Val, typed};

/// レコードの、グループ・コード 0 の次からのタグ
#[derive(Clone, Copy, Debug)]
pub struct RawTag<'a> {
    /// グループ・コード
    pub code: i16,
    /// 値の行（改行を除く）
    pub value: &'a [u8],
}

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

/// 続くレコードを親に付けるときの、レコードの役
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Follow {
    /// INSERT。`opens` は、66 が 1（続く ATTRIB を、SEQEND まで自分に付ける）
    Insert {
        /// 続くレコードを受ける
        opens: bool,
    },
    /// POLYLINE。`opens` は、66 が 0 でない（続く VERTEX を、SEQEND まで自分に付ける）
    Polyline {
        /// 続くレコードを受ける
        opens: bool,
    },
    /// ATTRIB
    Attrib,
    /// VERTEX
    Vertex,
    /// SEQEND
    Seqend,
}

/// 続くレコードを親に付けるために見る値。今の実装の `attached`（`crates/zumen-acad/src/drawing.rs`）が
/// 見るものと同じ
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Followed {
    /// レコードの役
    pub kind: Follow,
    /// ハンドル（5 の、16 進として読める最初の値）
    pub handle: Option<u64>,
    /// 所有者（102 の組の外の最初の 330。ハンドルとして読めなければ、ない）
    pub owner: Option<u64>,
}

/// レコードの種類とタグから、親に付けるために見る値を読む。INSERT・POLYLINE・ATTRIB・VERTEX・SEQEND で
/// なければ `None`
pub fn followed(name: &[u8], tags: &[RawTag<'_>]) -> Option<Followed> {
    let first = |code: i16| {
        tags.iter()
            .find(|tag| tag.code == code)
            .map(|tag| typed(code, tag.value))
    };
    let kind = match name {
        b"INSERT" => Follow::Insert {
            opens: first(66) == Some(Val::Int(1)),
        },
        b"POLYLINE" => Follow::Polyline {
            opens: first(66) != Some(Val::Int(0)),
        },
        b"ATTRIB" => Follow::Attrib,
        b"VERTEX" => Follow::Vertex,
        b"SEQEND" => Follow::Seqend,
        _ => return None,
    };
    let handle =
        tags.iter()
            .filter(|tag| tag.code == 5)
            .find_map(|tag| match typed(5, tag.value) {
                Val::Handle(handle) => Some(handle),
                _ => None,
            });
    let mut owner = None;
    let mut depth = 0usize;
    for tag in tags {
        match tag.code {
            102 if tag.value.starts_with(b"{") => depth += 1,
            102 if tag.value.starts_with(b"}") => depth = depth.saturating_sub(1),
            330 if depth == 0 => {
                if let Val::Handle(handle) = typed(330, tag.value) {
                    owner = Some(handle);
                }
                break;
            }
            _ => {}
        }
    }
    Some(Followed {
        kind,
        handle,
        owner,
    })
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

/// グループ・コードの置き場の番号。持ち上げで読むコードだけが置き場を持つ
const fn slot(code: i16) -> usize {
    match code {
        1 => 1,
        6 => 2,
        7 => 3,
        8 => 4,
        10 => 5,
        11 => 6,
        20 => 7,
        21 => 8,
        30 => 9,
        31 => 10,
        38 => 11,
        40 => 12,
        41 => 13,
        43 => 14,
        48 => 15,
        50 => 16,
        51 => 17,
        62 => 18,
        67 => 19,
        70 => 20,
        71 => 21,
        72 => 22,
        73 => 23,
        90 => 24,
        210 => 25,
        220 => 26,
        230 => 27,
        370 => 28,
        420 => 29,
        440 => 30,
        2 => 31,
        42 => 32,
        44 => 33,
        45 => 34,
        66 => 35,
        74 => 36,
        101 => 37,
        _ => 0,
    }
}

const SLOTS: usize = 38;

/// 0〜511 のグループ・コードから置き場の番号を引く表
const SLOT_TABLE: [u8; 512] = {
    let mut table = [0u8; 512];
    let mut code = 0;
    while code < 512 {
        table[code] = slot(code as i16) as u8;
        code += 1;
    }
    table
};

/// グループ・コードごとの最初の値
struct Fields<'a> {
    first: [Option<&'a [u8]>; SLOTS],
    /// 5 の、16 進として読める最初の値
    handle: Option<u64>,
}

impl<'a> Fields<'a> {
    fn new(tags: &[RawTag<'a>]) -> Self {
        let mut first: [Option<&'a [u8]>; SLOTS] = [None; SLOTS];
        let mut handle = None;
        for tag in tags {
            if tag.code == 5 && handle.is_none() {
                if let Val::Handle(value) = typed(tag.code, tag.value) {
                    handle = Some(value);
                }
                continue;
            }
            let at = usize::try_from(tag.code)
                .ok()
                .and_then(|code| SLOT_TABLE.get(code))
                .map_or(0, |&at| at as usize);
            if at != 0 {
                let place = &mut first[at];
                if place.is_none() {
                    *place = Some(tag.value);
                }
            }
        }
        Fields { first, handle }
    }

    /// グループ・コードの最初の値を、型どおりに読んだもの
    fn first(&self, code: i16) -> Option<Val<'a>> {
        self.first[slot(code)].map(|raw| typed(code, raw))
    }

    /// 元のままの最初の値
    fn raw(&self, code: i16) -> Option<&'a [u8]> {
        self.first[slot(code)]
    }

    /// 有限の実数の値。なければ `default`、それもなければ読めないとする
    fn real(&self, code: i16, default: Option<f64>) -> Option<f64> {
        match self.first(code) {
            Some(Val::Real(value)) => Some(value),
            Some(_) => None,
            None => default,
        }
    }

    /// 整数の値。なければ `default`
    fn int(&self, code: i16, default: i64) -> Option<i64> {
        match self.first(code) {
            Some(Val::Int(value)) => Some(value),
            Some(_) => None,
            None => Some(default),
        }
    }
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
    let fields = Fields::new(tags);
    match fields.int(67, 0) {
        Some(1) => return Lift::Paper,
        Some(_) => {}
        None => return Lift::Not,
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
    fields: &Fields<'a>,
    pools: &mut Pools,
) -> Option<Lifted<'a>> {
    // 押し出しの向きが既定（0,0,1）の図形だけを読む
    if !upright(fields)? {
        return None;
    }
    let mut class = class::ASCII;
    let geom = match kind {
        Kind::Line => {
            let mut ends = [0.0; 4];
            for (index, x) in [10i16, 11].into_iter().enumerate() {
                let z = fields.real(x + 20, Some(0.0))?;
                ends[index * 2] = fields.real(x, None)?;
                ends[index * 2 + 1] = fields.real(x + 10, None)?;
                if z != 0.0 {
                    return None;
                }
            }
            Geom::Line(ends)
        }
        Kind::Circle => {
            let [x, y] = planar_point(fields, 10)?;
            let radius = fields.real(40, None)?;
            if radius <= 0.0 {
                return None;
            }
            Geom::Circle([x, y, radius])
        }
        Kind::Arc => {
            let [x, y] = planar_point(fields, 10)?;
            let radius = fields.real(40, None)?;
            if radius <= 0.0 {
                return None;
            }
            let start = fields.real(50, None)?;
            let end = fields.real(51, None)?;
            Geom::Arc([x, y, radius, start, end])
        }
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

/// 押し出しの向き（210・220・230）が既定（0,0,1）か
fn upright(fields: &Fields<'_>) -> Option<bool> {
    let normal = [
        fields.real(210, Some(0.0))?,
        fields.real(220, Some(0.0))?,
        fields.real(230, Some(1.0))?,
    ];
    Some(normal == [0.0, 0.0, 1.0])
}

/// 画層・色・線種・線種の倍率・線幅・透過性を確かめて、画層の名前・色の指定・文字の値の種類を返す
fn styled<'a>(fields: &Fields<'a>) -> Option<(&'a [u8], u32, u8)> {
    let mut class = class::ASCII;
    // 色（62・420）
    let rgb = match fields.first(420) {
        None => None,
        Some(Val::Int(value @ 0..=0xFF_FFFF)) => Some(value as u32),
        Some(_) => return None,
    };
    let color = match (fields.int(62, 256)?, rgb) {
        // 色番号と RGB の両方があれば、表示には RGB を使う
        (_, Some(rgb)) => color::RGB | rgb,
        (index @ 1..=255, None) => color::INDEX | index as u32,
        (0, None) => color::BY_BLOCK,
        (256, None) => color::BY_LAYER,
        _ => return None,
    };
    // 線種（6）は名前を読めればよい。線種の倍率（48）・線幅（370）・透過性（440）は、値の範囲だけを見る
    if let Some(name) = fields.raw(6) {
        class = class.max(class_of(name));
    }
    if fields.real(48, Some(1.0))? <= 0.0 {
        return None;
    }
    match fields.first(370) {
        None | Some(Val::Int(-3..=211)) => {}
        Some(_) => return None,
    }
    match fields.first(440) {
        None => {}
        Some(Val::Int(value)) => {
            let value = u32::try_from(value).ok()?;
            if value >> 24 > 2 {
                return None;
            }
        }
        Some(_) => return None,
    }
    // 画層（8）は必ず要る
    let layer = fields.raw(8)?;
    class = class.max(class_of(layer));
    Some((layer, color, class))
}

/// 記号（INSERT）。定義がなくても要素になる。格子に並べないとき（列も行も 1）は、間隔を読まない
fn insert(fields: &Fields<'_>, pools: &mut Pools) -> Option<(InsertRow, u8)> {
    let name = fields.raw(2)?;
    let scale = [fields.real(41, Some(1.0))?, fields.real(42, Some(1.0))?];
    if scale.contains(&0.0) {
        return None;
    }
    let count = |code: i16| {
        u32::try_from(fields.int(code, 1)?)
            .ok()
            .filter(|count| *count >= 1)
    };
    let grid = [count(70)?, count(71)?];
    let spacing = if grid == [1, 1] {
        [0.0, 0.0]
    } else {
        [fields.real(44, Some(0.0))?, fields.real(45, Some(0.0))?]
    };
    let position = planar_point(fields, 10)?;
    let rotation = fields.real(50, Some(0.0))?;
    let followed = matches!(fields.first(66), Some(Val::Int(value)) if value != 0);
    let class = class_of(name);
    Some((
        InsertRow {
            name: pools.push_text(name)?,
            definition: NO_DEFINITION,
            position,
            scale,
            rotation,
            grid,
            spacing,
            attribs: (0, 0),
            attached: 0,
            followed,
        },
        class,
    ))
}

/// ATTRIB を、記号の属性として持ち上げる。空間（67）は見ない。書式つきの中身（101）を持つ属性は読まない
/// （今の実装は、その属性を持つ記号を動かす変更を受け付けない）。読めなければ、置き場に何も残さない
pub fn attrib(tags: &[RawTag<'_>], record: u32, pools: &mut Pools) -> Option<AttribRow> {
    let fields = Fields::new(tags);
    let marks = pools.marks();
    let row = attribute(&fields, record, pools);
    if row.is_none() {
        pools.truncate(marks);
    }
    row
}

fn attribute(fields: &Fields<'_>, record: u32, pools: &mut Pools) -> Option<AttribRow> {
    if !upright(fields)? || fields.raw(101).is_some() {
        return None;
    }
    let (text, text_class) = text(fields, pools, 74)?;
    let tag = fields.raw(2)?;
    let hidden = fields.int(70, 0)? & 1 != 0;
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

/// Z が 0 の点
fn planar_point(fields: &Fields<'_>, x: i16) -> Option<[f64; 2]> {
    if fields.real(x + 20, Some(0.0))? != 0.0 {
        return None;
    }
    Some([fields.real(x, None)?, fields.real(x + 10, None)?])
}

fn polyline(tags: &[RawTag<'_>], fields: &Fields<'_>, pools: &mut Pools) -> Option<Geom> {
    if fields.real(38, Some(0.0))? != 0.0 {
        return None;
    }
    let first = pools.vertices.len();
    // 頂点ごとの幅。幅を持つ頂点が出るまでは置かない
    let mut widths: Vec<[Option<f64>; 2]> = Vec::new();
    let mut any_width = false;
    let mut index = 0;
    while let Some(tag) = tags.get(index) {
        index += 1;
        match tag.code {
            10 => {
                let Val::Real(x) = typed(10, tag.value) else {
                    return None;
                };
                // 20 は、すぐ後ろの値であること
                let y = match tags.get(index) {
                    Some(next) if next.code == 20 => match typed(20, next.value) {
                        Val::Real(y) => y,
                        _ => return None,
                    },
                    _ => return None,
                };
                index += 1;
                pools.vertices.push([x, y, 0.0]);
                if any_width {
                    widths.push([None; 2]);
                }
            }
            40..=42 => {
                let Val::Real(value) = typed(tag.code, tag.value) else {
                    return None;
                };
                let count = pools.vertices.len() - first;
                if count == 0 {
                    return None;
                }
                if tag.code == 42 {
                    pools.vertices.last_mut()?[2] = value;
                } else {
                    if !any_width {
                        any_width = true;
                        widths.resize(count, [None; 2]);
                    }
                    widths.last_mut()?[usize::from(tag.code == 41)] = Some(value);
                }
            }
            _ => {}
        }
    }
    let count = pools.vertices.len() - first;
    if usize::try_from(fields.int(90, -1)?).ok() != Some(count) {
        return None;
    }
    let width = match fields.real(43, Some(0.0))? {
        0.0 => 0.0,
        width if width > 0.0 => width,
        _ => return None,
    };
    let bits = fields.int(70, 0)?;
    let mut flags = 0;
    if bits & 1 != 0 {
        flags |= POLY_CLOSED;
    }
    if bits & 128 != 0 {
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
        width,
    })
}

/// 文字。縦の揃えのグループ・コードは、TEXT では 73、ATTRIB では 74
fn text(fields: &Fields<'_>, pools: &mut Pools, vertical: i16) -> Option<(TextRow, u8)> {
    let at = |x: i16| planar_point(fields, x);
    let first = at(10)?;
    let horizontal = fields.int(72, 0)?;
    let vertical = fields.int(vertical, 0)?;
    let place = match (horizontal, vertical) {
        (0, 0) => Place::Anchor {
            point: first,
            horizontal: Horizontal::Left,
            vertical: Vertical::Baseline,
        },
        (3, _) | (5, _) => Place::Between {
            first,
            second: at(11)?,
            fit: if horizontal == 3 {
                Fit::Aligned
            } else {
                Fit::Fit
            },
        },
        _ => Place::Anchor {
            point: at(11)?,
            horizontal: match horizontal {
                0 => Horizontal::Left,
                1 => Horizontal::Center,
                2 => Horizontal::Right,
                4 => Horizontal::Middle,
                _ => return None,
            },
            vertical: match vertical {
                0 => Vertical::Baseline,
                1 => Vertical::Bottom,
                2 => Vertical::Middle,
                3 => Vertical::Top,
                _ => return None,
            },
        },
    };
    let flags = fields.int(71, 0)?;
    let content = fields.raw(1)?;
    let height = fields.real(40, None)?;
    let rotation = fields.real(50, Some(0.0))?;
    let width_factor = fields.real(41, Some(1.0))?;
    let oblique = fields.real(51, Some(0.0))?;
    let style = fields.raw(7);
    let class = class_of(content).max(style.map_or(class::ASCII, class_of));
    let mut bits = 0;
    if flags & 2 != 0 {
        bits |= TEXT_BACKWARD;
    }
    if flags & 4 != 0 {
        bits |= TEXT_UPSIDE_DOWN;
    }
    let content = pools.push_text(content)?;
    let style = match style {
        Some(name) => Some(pools.push_text(name)?),
        None => None,
    };
    Some((
        TextRow {
            place,
            height,
            rotation,
            width_factor,
            oblique,
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
