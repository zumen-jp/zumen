//! 常駐する文書。元のバイト列を並列に読んで列に置き、変更・取り消し・保存・範囲の選択に答える。
//!
//! 元のバイト列は変えずに持ち、変えたレコードだけを、タグの並びとして別に持つ（`overlay`）。
//! 列（行・形・表示・索引）は、いつもタグの並びから読み直した値と同じになるように保つ。

use std::collections::{BTreeMap, BTreeSet};
use std::ops::Range;
use std::sync::Arc;
use std::thread;

pub use zumen_acad::Charset;
use zumen_acad::single::{self, SingleTables};
use zumen_acad::{DataType, Value};

use crate::display::{Display, RowBuffers};
use crate::grid::Grid;
use crate::lift::{
    self, AttribRow, Follow, Followed, Geom, InsertRow, Kind, Lift, NO_DEFINITION, POLY_CLOSED,
    Place, Pools, RawTag, TextRow, class,
};
use crate::scan::{self, Next, Span};
use crate::value::{Val, typed, typed_of, value};

/// 開けない理由
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OpenError {
    /// バイナリの DXF
    Binary,
    /// 4 GiB 以上
    TooLarge,
    /// 値の行がないか、グループ・コードを読めない
    Invalid,
}

/// バイナリの DXF の先頭
const BINARY_SENTINEL: &[u8] = b"AutoCAD Binary DXF\r\n\x1a\0";

/// セクション
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Section {
    Header,
    Tables,
    Blocks,
    Entities,
    Objects,
    Other,
}

/// 名前の表を作るレコードの種類。足す変更と属性を変える変更は、名前をこの表の綴りで書く
/// （今の実装の `Born::names`）
pub const TABLE_KINDS: [&[u8]; 5] = [b"LAYER", b"LTYPE", b"STYLE", b"BLOCK_RECORD", b"DIMSTYLE"];

/// `TABLE_KINDS` の中の番号
pub mod table {
    pub const LAYER: usize = 0;
    pub const LTYPE: usize = 1;
    pub const STYLE: usize = 2;
    pub const BLOCK_RECORD: usize = 3;
}

/// 読むときに覚えておくレコード
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Mark {
    Section(Section),
    EndSection,
    /// 名前の表を作るレコード（`TABLE_KINDS` の中の番号）
    Table(u8),
    /// BLOCK
    Block,
    /// ENDBLK
    EndBlock,
    /// ATTDEF
    Attdef,
    /// 6 種類の図形のうち、ペーパー空間のもの
    Paper,
    /// 6 種類の図形のうち、要素にならなかったもの
    Dropped,
}

/// 区間を読み終えた様子
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum End {
    /// 区間の終わりまで読んだ
    Through,
    /// EOF のレコードを読んだ。値は、その後ろの位置
    Eof(u32),
    /// 読めないタグがあった
    Invalid,
}

/// 要素の行
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Row {
    /// ハンドル。なければ 0
    pub handle: u64,
    /// レコードの番号
    pub record: u32,
    /// 種類ごとの表の中の番号
    pub geom: u32,
    /// 画層の番号
    pub layer: u32,
    /// 色の指定（`lift::color`）
    pub color: u32,
    /// 種類
    pub kind: Kind,
    /// 文字の値の種類（`lift::class`）
    pub class: u8,
    /// 生きているか。除いた要素の行は、取り消しのために残す
    pub alive: bool,
}

/// 折れ線の行
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Poly {
    /// 最初の頂点
    pub first: u32,
    /// 頂点の数
    pub count: u32,
    /// `lift::POLY_*`
    pub flags: u8,
    /// 一定の幅。0 は、なし
    pub width: f64,
}

/// 種類ごとの形の表
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Shapes {
    /// 線
    pub lines: Vec<[f64; 4]>,
    /// 円
    pub circles: Vec<[f64; 3]>,
    /// 円弧
    pub arcs: Vec<[f64; 5]>,
    /// 折れ線
    pub polys: Vec<Poly>,
    /// 文字
    pub texts: Vec<TextRow>,
    /// 記号（ブロックの挿入）
    pub inserts: Vec<InsertRow>,
    /// 折れ線の頂点・文字の中身・記号の属性
    pub pools: Pools,
}

impl Shapes {
    /// 形を足して、表の中の番号を返す
    pub(crate) fn push(&mut self, geom: Geom) -> u32 {
        match geom {
            Geom::Line(line) => {
                self.lines.push(line);
                self.lines.len() as u32 - 1
            }
            Geom::Circle(circle) => {
                self.circles.push(circle);
                self.circles.len() as u32 - 1
            }
            Geom::Arc(arc) => {
                self.arcs.push(arc);
                self.arcs.len() as u32 - 1
            }
            Geom::Polyline {
                first,
                count,
                flags,
                width,
            } => {
                self.polys.push(Poly {
                    first,
                    count,
                    flags,
                    width,
                });
                self.polys.len() as u32 - 1
            }
            Geom::Text(text) => {
                self.texts.push(text);
                self.texts.len() as u32 - 1
            }
            Geom::Insert(insert) => {
                self.inserts.push(insert);
                self.inserts.len() as u32 - 1
            }
        }
    }

    /// 種類ごとの表の、終わりの形を除く。足した行を取り消すときに使う（頂点と文字の置き場は詰めない）
    pub(crate) fn pop(&mut self, kind: Kind) {
        match kind {
            Kind::Line => drop(self.lines.pop()),
            Kind::Circle => drop(self.circles.pop()),
            Kind::Arc => drop(self.arcs.pop()),
            Kind::Polyline => drop(self.polys.pop()),
            Kind::Text => drop(self.texts.pop()),
            Kind::Insert => drop(self.inserts.pop()),
        }
    }

    /// 行の形
    pub fn geom(&self, kind: Kind, at: u32) -> Option<Geom> {
        let at = at as usize;
        Some(match kind {
            Kind::Line => Geom::Line(*self.lines.get(at)?),
            Kind::Circle => Geom::Circle(*self.circles.get(at)?),
            Kind::Arc => Geom::Arc(*self.arcs.get(at)?),
            Kind::Polyline => {
                let poly = self.polys.get(at)?;
                Geom::Polyline {
                    first: poly.first,
                    count: poly.count,
                    flags: poly.flags,
                    width: poly.width,
                }
            }
            Kind::Text => Geom::Text(*self.texts.get(at)?),
            Kind::Insert => Geom::Insert(*self.inserts.get(at)?),
        })
    }
}

/// 画層
#[derive(Clone, Debug, PartialEq)]
pub struct Layer {
    /// 名前。符号化のまま
    pub name: Box<[u8]>,
    /// 画層の表のレコード。図形が指すだけで表にない画層は `None`
    pub record: Option<u32>,
    /// 表のフラグ（70）
    pub flags: i64,
    /// 表の色（62）。負なら非表示
    pub color: i64,
    /// 表の線種の名前（6）
    pub line_type: Box<[u8]>,
}

/// 画層の表。名前は、ASCII の大文字と小文字を区別しない
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Layers {
    /// 画層。表の順、続いて図形に初めて現れた順
    pub list: Vec<Layer>,
    by_name: BTreeMap<Box<[u8]>, u32>,
}

impl Layers {
    /// 名前から番号を引く。なければ、表にない画層として足す
    pub fn intern(&mut self, name: &[u8]) -> u32 {
        let key = name.to_ascii_lowercase();
        if let Some(&id) = self.by_name.get(key.as_slice()) {
            return id;
        }
        let id = self.list.len() as u32;
        self.by_name.insert(key.into(), id);
        self.list.push(Layer {
            name: name.into(),
            record: None,
            flags: 0,
            color: 7,
            line_type: Box::default(),
        });
        id
    }
}

/// 区間の中の画層の名前
#[derive(Default)]
struct Names {
    list: Vec<Box<[u8]>>,
    by_name: BTreeMap<Box<[u8]>, u32>,
    /// 直前に引いた名前と番号
    last: Option<(Box<[u8]>, u32)>,
    key: Vec<u8>,
}

impl Names {
    fn intern(&mut self, name: &[u8]) -> u32 {
        if let Some((last, id)) = &self.last
            && last.as_ref() == name
        {
            return *id;
        }
        self.key.clear();
        self.key.extend(name.iter().map(u8::to_ascii_lowercase));
        let id = match self.by_name.get(self.key.as_slice()) {
            Some(&id) => id,
            None => {
                let id = self.list.len() as u32;
                self.by_name.insert(self.key.as_slice().into(), id);
                self.list.push(name.into());
                id
            }
        };
        self.last = Some((name.into(), id));
        id
    }
}

/// 区間を読んだ結果
pub struct Part {
    /// レコードの先頭
    starts: Vec<u32>,
    marks: Vec<(u32, Mark)>,
    rows: Vec<Row>,
    shapes: Shapes,
    layers: Vec<Box<[u8]>>,
    /// ハンドルを持つレコードと、そのハンドル（0 も含む）
    handles: Vec<(u32, u64)>,
    /// INSERT・POLYLINE・ATTRIB・VERTEX・SEQEND のレコード
    family: Vec<(u32, Followed)>,
    /// 102 の組の中の 330・360 と、DICTIONARY の項目の 350・360 が指すハンドルと、指しているレコード。
    /// 要素を除くときに直すレコードの候補になる
    refs: Vec<(u64, u32)>,
    /// 各レコードの最初の 330（所有者ハンドル）の連長圧縮 `(handle, start_record, end_record)`
    owner_runs: Vec<(u64, u32, u32)>,
    /// 自身の最初のハンドル（5 / DIMSTYLE の 105）と最初の 330 を除く、すべてのハンドル型の値 `(handle, record)`
    handle_refs: Vec<(u64, u32)>,
    /// レコードごとの印。1 はサブクラスの印（100）、2 は 102 または 360 を持つ
    flags: Vec<u8>,
    end: End,
}

impl Part {
    fn empty(end: End) -> Part {
        Part {
            starts: Vec::new(),
            marks: Vec::new(),
            rows: Vec::new(),
            shapes: Shapes::default(),
            layers: Vec::new(),
            handles: Vec::new(),
            family: Vec::new(),
            refs: Vec::new(),
            owner_runs: Vec::new(),
            handle_refs: Vec::new(),
            flags: Vec::new(),
            end,
        }
    }
}

/// レコードの中の、除くときに直す参照（今の実装の `Object::forget` が見るもの）を集める。
/// 実際に直すかは、除くときに `forget` を当てて決めるので、広めに取ってよい
fn references(name: &[u8], tags: &[RawTag<'_>], record: u32, out: &mut Vec<(u64, u32)>) {
    let dictionary = name == b"DICTIONARY";
    let mut depth = 0usize;
    let mut keyed = false;
    for tag in tags {
        let after_key = core::mem::take(&mut keyed);
        let refers = match tag.code {
            102 if tag.value.starts_with(b"{") => {
                depth += 1;
                false
            }
            102 if tag.value.starts_with(b"}") => {
                depth = depth.saturating_sub(1);
                false
            }
            330 | 360 if depth > 0 => true,
            350 | 360 if after_key => true,
            3 if dictionary && depth == 0 => {
                keyed = true;
                false
            }
            _ => false,
        };
        if refers && let Val::Handle(handle) = typed(tag.code, tag.value) {
            out.push((handle, record));
        }
    }
}

/// レコードのハンドル（5 の、16 進として読める最初の値。DIMSTYLE は 105）
fn handle_of(name: &[u8], tags: &[RawTag<'_>]) -> Option<u64> {
    let expected = if name == b"DIMSTYLE" { 105 } else { 5 };
    tags.iter()
        .filter(|tag| tag.code == expected)
        .find_map(|tag| match typed_of(name, tag.code, tag.value) {
            Val::Handle(handle) => Some(handle),
            _ => None,
        })
}

/// 区間 `range` のレコードを読む。区間の始まりは、レコードの先頭であること
fn scan_part(src: &[u8], range: Range<usize>) -> Part {
    let mut part = Part {
        starts: Vec::with_capacity(range.len() / 96),
        rows: Vec::with_capacity(range.len() / 128),
        handles: Vec::with_capacity(range.len() / 96),
        flags: Vec::with_capacity(range.len() / 96),
        ..Part::empty(End::Through)
    };
    let mut names = Names::default();
    let mut tags: Vec<RawTag<'_>> = Vec::with_capacity(64);
    let mut pos = range.start;
    while pos < range.end {
        let head = match scan::next_tag(src, pos) {
            Next::Tag(tag) => tag,
            Next::End => break,
            Next::Invalid => {
                part.end = End::Invalid;
                break;
            }
        };
        let record = part.starts.len() as u32;
        part.starts.push(pos as u32);
        part.flags.push(0);
        // ファイルの最初のタグは、0 でなくてもレコードを始める
        let name = if head.code == 0 {
            src.get(head.value as usize..head.value_end as usize)
                .unwrap_or_default()
                .trim_ascii()
        } else {
            &[]
        };
        pos = head.end as usize;
        if head.code == 0 && name == b"EOF" {
            part.end = End::Eof(head.end);
            break;
        }
        tags.clear();
        let (mut subclassed, mut grouped, mut has_360) = (false, false, false);
        loop {
            match scan::next_tag(src, pos) {
                Next::Tag(tag) if tag.code == 0 => break,
                Next::Tag(tag) => {
                    subclassed |= tag.code == 100;
                    grouped |= tag.code == 102;
                    has_360 |= tag.code == 360;
                    tags.push(RawTag {
                        code: tag.code,
                        value: src
                            .get(tag.value as usize..tag.value_end as usize)
                            .unwrap_or_default(),
                    });
                    pos = tag.end as usize;
                }
                Next::End => break,
                Next::Invalid => {
                    part.end = End::Invalid;
                    break;
                }
            }
        }
        if part.end == End::Invalid {
            break;
        }
        if (subclassed || grouped || has_360)
            && let Some(flag) = part.flags.last_mut()
        {
            *flag = u8::from(subclassed) | (u8::from(grouped || has_360) << 1);
        }
        if grouped || name == b"DICTIONARY" {
            references(name, &tags, record, &mut part.refs);
        }
        if let Some(found) = lift::followed(name, &tags) {
            part.family.push((record, found));
        }
        let mut own_handle = None;
        if name != b"SECTION" && name != b"ENDSEC" {
            let own_code = if name == b"DIMSTYLE" { 105 } else { 5 };
            let mut seen_owner = false;
            for tag in &tags {
                if (name == b"DIMSTYLE" && tag.code == 5)
                    || DataType::of(tag.code) != DataType::Handle
                {
                    continue;
                }
                let Val::Handle(h) = typed_of(name, tag.code, tag.value) else {
                    continue;
                };
                if tag.code == own_code && own_handle.is_none() {
                    own_handle = Some(h);
                    continue;
                }
                if tag.code == 330 && !seen_owner {
                    seen_owner = true;
                    match part.owner_runs.last_mut() {
                        Some((last_h, _, end_rec)) if *last_h == h && *end_rec == record => {
                            *end_rec = record + 1;
                        }
                        _ => part.owner_runs.push((h, record, record + 1)),
                    }
                    continue;
                }
                part.handle_refs.push((h, record));
            }
        }
        let handle = match name {
            b"SECTION" => {
                let section = tags
                    .iter()
                    .find(|tag| tag.code == 2)
                    .map(|tag| tag.value.trim_ascii())
                    .unwrap_or_default();
                let section = match section {
                    b"HEADER" => Section::Header,
                    b"TABLES" => Section::Tables,
                    b"BLOCKS" => Section::Blocks,
                    b"ENTITIES" => Section::Entities,
                    b"OBJECTS" => Section::Objects,
                    _ => Section::Other,
                };
                part.marks.push((record, Mark::Section(section)));
                None
            }
            b"ENDSEC" => {
                part.marks.push((record, Mark::EndSection));
                None
            }
            _ => match Kind::of(name) {
                Some(kind) => match lift::lift(kind, &tags, &mut part.shapes.pools) {
                    Lift::Element(lifted) => {
                        let layer = names.intern(lifted.layer);
                        let geom = part.shapes.push(lifted.geom);
                        part.rows.push(Row {
                            handle: lifted.handle.unwrap_or(0),
                            record,
                            geom,
                            layer,
                            color: lifted.color,
                            kind,
                            class: lifted.class,
                            alive: true,
                        });
                        lifted.handle
                    }
                    Lift::Paper => {
                        part.marks.push((record, Mark::Paper));
                        own_handle
                    }
                    Lift::Not => {
                        part.marks.push((record, Mark::Dropped));
                        own_handle
                    }
                },
                None => {
                    let mark = match name {
                        b"BLOCK" => Some(Mark::Block),
                        b"ENDBLK" => Some(Mark::EndBlock),
                        b"ATTDEF" => Some(Mark::Attdef),
                        _ => TABLE_KINDS
                            .iter()
                            .position(|kind| *kind == name)
                            .map(|table| Mark::Table(table as u8)),
                    };
                    if let Some(mark) = mark {
                        part.marks.push((record, mark));
                    }
                    own_handle
                }
            },
        };
        if let Some(handle) = handle {
            part.handles.push((record, handle));
        }
    }
    part.layers = names.list;
    part
}

/// スレッド 1 本が読む、いちばん小さい量。これより小さく分けると、スレッドを起こす手間のほうが大きい
const PART_BYTES: usize = 256 * 1024;

/// 入力を、多くても `threads` 本のスレッドで読む。小さい入力では、スレッドを減らす。
/// 結果は、スレッドの数に依らない
pub fn scan(src: &[u8], threads: usize) -> Result<Vec<Part>, OpenError> {
    scan_split(src, threads.min(src.len() / PART_BYTES).max(1))
}

/// 入力を `parts` 個までの区間に分け、区間ごとのスレッドで読む
pub fn scan_split(src: &[u8], parts: usize) -> Result<Vec<Part>, OpenError> {
    if src.starts_with(BINARY_SENTINEL) {
        return Err(OpenError::Binary);
    }
    if u32::try_from(src.len()).is_err() {
        return Err(OpenError::TooLarge);
    }
    let cuts = scan::split_points(src, parts);
    if cuts.len() <= 2 {
        return Ok(vec![scan_part(src, 0..src.len())]);
    }
    Ok(thread::scope(|scope| {
        let workers: Vec<_> = cuts
            .windows(2)
            .map(|pair| {
                let range = pair.first().copied().unwrap_or(0)..pair.get(1).copied().unwrap_or(0);
                scope.spawn(move || scan_part(src, range))
            })
            .collect();
        workers
            .into_iter()
            .map(|worker| worker.join().unwrap_or_else(|_| Part::empty(End::Invalid)))
            .collect()
    }))
}

/// ヘッダーの変数
#[derive(Clone, Debug, PartialEq)]
pub struct Variable {
    /// 値の最初のタグの番号（レコードの中）
    pub start: usize,
    /// 値
    pub values: Vec<Value>,
}

/// HEADER のレコードと、変更で見る変数
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Header {
    /// レコードの番号
    pub record: u32,
    /// `$EXTMIN`
    pub extmin: Option<Variable>,
    /// `$EXTMAX`
    pub extmax: Option<Variable>,
    /// `$HANDSEED`
    pub seed: Option<Variable>,
}

/// 名前の表の 1 つの名前
#[derive(Clone, Debug, PartialEq)]
pub struct Spelling {
    /// 表のレコードの綴り（同じ名前がいくつかあれば、最後のもの）。符号化のまま
    pub name: Box<[u8]>,
    /// 同じ名前（Unicode の小文字にして同じ）のレコードの数
    pub count: u32,
    /// そのうち、TABLES のセクションにあるレコードの数
    pub tables: u32,
}

/// ブロックの定義。開くときに 1 回読む
#[derive(Clone, Debug, PartialEq)]
pub struct Definition {
    /// 名前（BLOCK の 2）。符号化のまま。読めなければ空
    pub name: Box<[u8]>,
    /// BLOCK のレコード
    pub record: u32,
    /// 基点
    pub base: [f64; 2],
    /// 中身の行（`Document::members` の中の、最初の番号と数）
    pub members: (u32, u32),
    /// 中身を囲む箱。中身がなければ、空の箱
    pub bounds: [f64; 4],
    /// 属性の定義（ATTDEF）を持つかもしれないか
    pub attdefs: bool,
    /// 記号を足す変更で指せるか
    pub usable: bool,
}

/// 空の箱。索引には入らない
pub const NO_BOX: [f64; 4] = [f64::NAN; 4];

/// 箱を、点を含むように広げる
fn reach(bounds: &mut [f64; 4], [x, y]: [f64; 2]) {
    *bounds = [
        bounds[0].min(x),
        bounds[1].min(y),
        bounds[2].max(x),
        bounds[3].max(y),
    ];
}

/// 何も含まない箱。`reach` で広げる
const EMPTY: [f64; 4] = [
    f64::INFINITY,
    f64::INFINITY,
    f64::NEG_INFINITY,
    f64::NEG_INFINITY,
];

/// 変えたレコードのタグ
#[derive(Clone, Debug, PartialEq)]
pub struct OTag {
    /// グループ・コード
    pub code: i16,
    /// 元のバイト列の中の場所。足したタグでは使わない
    pub span: Span,
    /// 足したタグか
    pub fresh: bool,
    /// 変えた値。変えていなければ `None`
    pub edited: Option<Box<[u8]>>,
}

/// ハンドルから行を引く表
#[derive(Clone, Debug, PartialEq)]
pub enum Handles {
    /// まだ作っていない
    None,
    /// ハンドルの値で引く（行の番号 + 1。0 は、なし）
    Dense(Vec<u32>),
    /// ハンドルが飛び飛びのとき。ハンドルの順に並べた、ハンドルと行
    Sparse(Vec<(u64, u32)>),
}

impl Handles {
    /// ハンドルの行
    pub fn get(&self, handle: u64) -> Option<u32> {
        match self {
            Handles::None => None,
            Handles::Dense(table) => {
                let at = *table.get(usize::try_from(handle).ok()?)?;
                at.checked_sub(1)
            }
            Handles::Sparse(table) => {
                let at = table.binary_search_by_key(&handle, |(key, _)| *key).ok()?;
                table.get(at).map(|(_, row)| *row)
            }
        }
    }
}

/// 変更を受け付けない理由
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Locked {
    /// 同じハンドルを持つオブジェクトがある（今の実装も、変更を拒む）
    DuplicateHandle,
}

/// 読んだものの数
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Counts {
    /// レコード
    pub records: u32,
    /// ENTITIES のセクションのレコード
    pub entities: u32,
    /// 要素（モデル空間の 6 種類。LINE・CIRCLE・ARC・LWPOLYLINE・TEXT・INSERT）
    pub elements: u32,
    /// 6 種類のうち、ペーパー空間のもの
    pub paper: u32,
    /// 6 種類のうち、要素にならなかったもの（今の実装でも要素にならない）
    pub dropped: u32,
    /// 6 種類のうち、ASCII でない文字を Shift_JIS で持つもの（この試作は読まない）
    pub shift_jis: u32,
    /// 6 種類のうち、文字を読めないもの（今の実装でも要素にならない）
    pub unreadable: u32,
    /// 要素のうち、ハンドルがないか 0 で、変更で指せないもの
    pub unaddressable: u32,
    /// 要素の INSERT に付いた属性（ATTRIB）
    pub attribs: u32,
    /// ブロックの定義
    pub definitions: u32,
}

/// 常駐する文書
pub struct Document {
    /// 元のバイト列
    pub(crate) src: Vec<u8>,
    /// レコードの先頭。最後に、EOF のレコードの後ろの位置を置く
    pub(crate) starts: Vec<u32>,
    /// 要素の行。ファイルの順
    pub rows: Vec<Row>,
    /// 形
    pub shapes: Shapes,
    /// 画層
    pub layers: Layers,
    /// 文字の値の符号化
    pub charset: Charset,
    /// HEADER
    pub(crate) header: Option<Header>,
    /// 変更を受け付けない理由
    pub locked: Option<Locked>,
    /// 読んだものの数
    pub counts: Counts,
    /// オブジェクトのレコードとハンドル（0 も含む）。索引を作るまで持つ
    object_handles: Vec<(u32, u64)>,
    /// `$ACADVER`。なければ空
    pub(crate) version: Vec<u8>,
    /// 足すレコードに、サブクラスの印（100）を書くか
    pub(crate) markers: bool,
    /// 名前の表（`TABLE_KINDS` の順）。Unicode の小文字にした名前から引く。
    /// 読める名前のレコードが 1 つもない表は `None`
    pub(crate) names: [Option<BTreeMap<String, Spelling>>; 5],
    /// 名前の表を、今の実装と同じに読めたか。Shift_JIS の図面の、ASCII でない名前は読まない
    pub(crate) names_exact: bool,
    /// モデル空間のブロックのレコードのハンドル。足すレコードの所有者（330）にする
    pub(crate) model_owner: Option<u64>,
    /// ブロックの定義。BLOCKS の順
    pub definitions: Vec<Definition>,
    /// 定義の名前（ASCII の小文字）から、最初の定義の番号を引く。表示と箱に使う
    pub(crate) definition_names: BTreeMap<Box<[u8]>, u32>,
    /// 記号を足す変更で指せる定義。Unicode の小文字にした名前から引く
    pub(crate) usable_definitions: BTreeMap<String, u32>,
    /// 定義の中身の行。BLOCKS の順
    pub members: Vec<Row>,
    /// 定義の中身の表示のバッファー。開くときに 1 回作り、INSERT の行は定義の番号で指す
    pub block_display: Display,
    /// 属性・頂点・終わりの印（ATTRIB・VERTEX・SEQEND）が、どれも直前に開いた INSERT か POLYLINE に付き、
    /// 所有者（330）があれば、その親を指すか。そうでない図面では、要素を足す・除く変更で、付き方が変わりうる
    pub plain: bool,
    /// 追加・スタイル変更で `zumen_acad::single` に渡す共通の名前表と設定
    pub(crate) single_tables: SingleTables,
    /// 元のレコードごとの印（1: 100 あり、2: 102 または 360 あり）
    pub(crate) flags: Vec<u8>,
    /// すべてのセクションの中の範囲（レコードの番号）
    pub(crate) sections: Vec<Range<u32>>,
    /// ENTITIES のセクションの範囲（レコードの番号）
    pub(crate) entities: Vec<Range<u32>>,
    /// ENTITIES のセクションがちょうど 1 つで ENDSEC で閉じているとき、その ENDSEC のレコード。
    /// 足すレコードは、この前に書く
    pub(crate) entities_end: Option<u32>,
    /// OBJECTS セクションのオブジェクト。ハンドルから最初のレコードを引く
    pub(crate) objects_by_handle: BTreeMap<u64, u32>,
    /// `objects_by_handle` のハンドルを指す元のレコード `(target_handle, record)`。ハンドルの順
    pub(crate) object_refs: Vec<(u64, u32)>,
    /// `$HANDSEED` を除く HEADER 変数が持つハンドル
    pub(crate) header_handles: BTreeSet<u64>,
    /// 102 の組と DICTIONARY の項目が指すハンドルと、指しているレコード。ハンドルの順
    pub(crate) refs: Vec<(u64, u32)>,
    /// 変更で除ける行と `objects_by_handle` を除く、固定オブジェクトのハンドルの最大
    pub(crate) fixed_largest: Option<u64>,
    /// 生きている元の行の、ハンドルの最大
    pub(crate) row_largest: Option<u64>,
    /// 除いたレコード
    pub(crate) gone: BTreeSet<u32>,
    /// 足して、生きている行。ハンドルから行を引く
    pub(crate) added: BTreeMap<u64, u32>,
    /// Shift_JIS の属性を持つ INSERT のレコード。動かす変更を受け付けない
    pub(crate) foreign: BTreeSet<u32>,
    /// 変えたレコード。番号が元のレコードの数から先のものは、足したレコード
    pub(crate) overlay: BTreeMap<u32, Vec<OTag>>,
    /// 取り消せる変更
    pub(crate) undo: Vec<crate::edit::Step>,
    /// やり直せる変更
    pub(crate) redo: Vec<crate::edit::Step>,
    /// 表示のバッファー
    pub display: Display,
    /// 要素を囲む箱
    pub boxes: Vec<[f64; 4]>,
    /// ハンドルの表
    pub handles: Handles,
    /// 範囲の索引
    pub grid: Grid,
}

/// レコードが、範囲のどれかに入るか。範囲は、番号の順
fn inside(ranges: &[Range<u32>], record: u32) -> bool {
    let at = ranges.partition_point(|range| range.end <= record);
    ranges.get(at).is_some_and(|range| range.start <= record)
}

/// 所有者（102 の組の外の最初の 330。ハンドルとして読めなければ、ない）
fn owner_of(tags: &[RawTag<'_>]) -> Option<u64> {
    let mut depth = 0usize;
    for tag in tags {
        match tag.code {
            102 if tag.value.starts_with(b"{") => depth += 1,
            102 if tag.value.starts_with(b"}") => depth = depth.saturating_sub(1),
            330 if depth == 0 => {
                return match typed(330, tag.value) {
                    Val::Handle(handle) => Some(handle),
                    _ => None,
                };
            }
            _ => {}
        }
    }
    None
}

/// レコードのバイトの範囲。`starts` は、レコードの先頭と、最後のレコードの後ろの位置
pub(crate) fn range_of(starts: &[u32], record: u32) -> Range<usize> {
    let at = record as usize;
    let start = starts.get(at).copied().unwrap_or(0) as usize;
    let end = starts.get(at + 1).copied().unwrap_or(start as u32) as usize;
    start..end.max(start)
}

/// タグの今の値
pub(crate) fn tag_value<'a>(src: &'a [u8], tag: &'a OTag) -> &'a [u8] {
    match &tag.edited {
        Some(edited) => edited,
        None => src
            .get(tag.span.value as usize..tag.span.value_end as usize)
            .unwrap_or_default(),
    }
}

/// 文字を置く点を、箱に入れる
fn reach_text(bounds: &mut [f64; 4], text: &TextRow) {
    match text.place {
        Place::Anchor { point, .. } => reach(bounds, point),
        Place::Between { first, second, .. } => {
            reach(bounds, first);
            reach(bounds, second);
        }
    }
}

/// 要素を囲む箱。円弧と膨らみのある辺は、広めに取る。文字は、置く点だけ。
/// 記号は、置く点・属性を置く点と、定義の中身の箱を、格子の四隅に写したもの
pub fn bounds(pools: &Pools, definitions: &[Definition], geom: &Geom) -> [f64; 4] {
    match geom {
        Geom::Insert(insert) => {
            let mut bounds = EMPTY;
            reach(&mut bounds, insert.position);
            for attribute in pools.attribs_of(insert) {
                reach_text(&mut bounds, &attribute.text);
            }
            let definition = definitions
                .get(insert.definition as usize)
                .filter(|definition| definition.bounds[0] <= definition.bounds[2]);
            if let Some(definition) = definition {
                let turn = insert.rotation.to_radians();
                let (sin, cos) = (libm::sin(turn), libm::cos(turn));
                let [left, bottom, right, top] = definition.bounds;
                let [columns, rows] = insert.grid;
                let across = f64::from(columns.max(1) - 1) * insert.spacing[0];
                let up = f64::from(rows.max(1) - 1) * insert.spacing[1];
                for [x, y] in [[left, bottom], [right, bottom], [right, top], [left, top]] {
                    let local = [
                        (x - definition.base[0]) * insert.scale[0],
                        (y - definition.base[1]) * insert.scale[1],
                    ];
                    for [dx, dy] in [[0.0, 0.0], [across, 0.0], [across, up], [0.0, up]] {
                        let [u, v] = [local[0] + dx, local[1] + dy];
                        reach(
                            &mut bounds,
                            [
                                insert.position[0] + u * cos - v * sin,
                                insert.position[1] + u * sin + v * cos,
                            ],
                        );
                    }
                }
            }
            bounds
        }
        Geom::Line([x0, y0, x1, y1]) => [x0.min(*x1), y0.min(*y1), x0.max(*x1), y0.max(*y1)],
        Geom::Circle([x, y, radius]) | Geom::Arc([x, y, radius, ..]) => {
            [x - radius, y - radius, x + radius, y + radius]
        }
        Geom::Polyline {
            first,
            count,
            flags,
            ..
        } => {
            let vertices = pools
                .vertices
                .get(*first as usize..(*first + *count) as usize)
                .unwrap_or_default();
            let mut bounds = [
                f64::INFINITY,
                f64::INFINITY,
                f64::NEG_INFINITY,
                f64::NEG_INFINITY,
            ];
            for [x, y, _] in vertices {
                bounds = [
                    bounds[0].min(*x),
                    bounds[1].min(*y),
                    bounds[2].max(*x),
                    bounds[3].max(*y),
                ];
            }
            for (from, to, bulge) in crate::display::edges(vertices, flags & POLY_CLOSED != 0) {
                if bulge != 0.0 {
                    // 弦から円弧のいちばん遠い点までの距離
                    let (dx, dy) = (to[0] - from[0], to[1] - from[1]);
                    let reach = (dx * dx + dy * dy).sqrt() * bulge.abs() / 2.0;
                    bounds = [
                        bounds[0].min(from[0].min(to[0]) - reach),
                        bounds[1].min(from[1].min(to[1]) - reach),
                        bounds[2].max(from[0].max(to[0]) + reach),
                        bounds[3].max(from[1].max(to[1]) + reach),
                    ];
                }
            }
            bounds
        }
        Geom::Text(row) => match row.place {
            Place::Anchor { point, .. } => [point[0], point[1], point[0], point[1]],
            Place::Between { first, second, .. } => [
                first[0].min(second[0]),
                first[1].min(second[1]),
                first[0].max(second[0]),
                first[1].max(second[1]),
            ],
        },
    }
}

/// レコード `range` をタグに分ける
pub(crate) fn tokenize(src: &[u8], range: Range<usize>) -> Vec<OTag> {
    let mut tags = Vec::with_capacity(32);
    let mut pos = range.start;
    while pos < range.end {
        let Next::Tag(span) = scan::next_tag(src, pos) else {
            break;
        };
        tags.push(OTag {
            code: span.code,
            span,
            fresh: false,
            edited: None,
        });
        pos = span.end as usize;
    }
    tags
}

impl Document {
    /// 入力を開く。読み・組み立て・表示・索引を続けて行う
    pub fn open(src: Vec<u8>, threads: usize) -> Result<Document, OpenError> {
        let parts = scan(&src, threads)?;
        let mut document = Document::assemble(src, parts)?;
        document.build_display();
        document.build_index();
        Ok(document)
    }

    /// 区間ごとに読んだものを、ファイルの順に並べる
    pub fn assemble(src: Vec<u8>, parts: Vec<Part>) -> Result<Document, OpenError> {
        // EOF のレコードを読んだ区間までを使う。その前に読めないタグがあれば、開かない
        let mut used = 0;
        let mut trailer = src.len() as u32;
        for part in &parts {
            used += 1;
            match part.end {
                End::Through => {}
                End::Eof(at) => {
                    trailer = at;
                    break;
                }
                End::Invalid => return Err(OpenError::Invalid),
            }
        }
        let parts = parts.into_iter().take(used).collect::<Vec<_>>();
        let records: usize = parts.iter().map(|part| part.starts.len()).sum();
        let mut starts = Vec::with_capacity(records + 1);
        // 覚えておいたレコード。番号は、ファイルの中の番号に直す
        let mut marks = Vec::new();
        let mut family = Vec::new();
        let mut refs = Vec::new();
        let mut handles = Vec::new();
        let mut flags = Vec::with_capacity(records);
        for part in &parts {
            let base = starts.len() as u32;
            marks.extend(
                part.marks
                    .iter()
                    .map(|&(record, mark)| (base + record, mark)),
            );
            family.extend(
                part.family
                    .iter()
                    .map(|&(record, found)| (base + record, found)),
            );
            refs.extend(
                part.refs
                    .iter()
                    .map(|&(handle, record)| (handle, base + record)),
            );
            handles.extend(
                part.handles
                    .iter()
                    .map(|&(record, handle)| (base + record, handle)),
            );
            flags.extend_from_slice(&part.flags);
            starts.extend_from_slice(&part.starts);
        }
        starts.push(trailer);

        // セクション。SECTION で始まり、ENDSEC か次の SECTION で終わる
        let mut entities: Vec<Range<u32>> = Vec::new();
        let mut tables: Vec<Range<u32>> = Vec::new();
        let mut blocks: Vec<Range<u32>> = Vec::new();
        let mut objects: Vec<Range<u32>> = Vec::new();
        let mut sections: Vec<Range<u32>> = Vec::new();
        // ENTITIES を閉じた ENDSEC
        let mut ends: Vec<u32> = Vec::new();
        let mut header = None;
        let mut open: Option<(Section, u32)> = None;
        let mut close = |open: &mut Option<(Section, u32)>, end: u32| {
            let Some((section, start)) = open.take() else {
                return;
            };
            match section {
                Section::Entities => entities.push(start..end),
                Section::Tables => tables.push(start..end),
                Section::Blocks => blocks.push(start..end),
                Section::Objects => objects.push(start..end),
                Section::Header | Section::Other => {}
            }
            sections.push(start..end);
        };
        for &(record, mark) in &marks {
            match mark {
                Mark::Section(section) => {
                    close(&mut open, record);
                    if section == Section::Header && header.is_none() {
                        header = Some(record);
                    }
                    open = Some((section, record + 1));
                }
                Mark::EndSection => {
                    if matches!(open, Some((Section::Entities, _))) {
                        ends.push(record);
                    }
                    close(&mut open, record);
                }
                _ => {}
            }
        }
        close(&mut open, records as u32);
        let entities_end = match (entities.as_slice(), ends.as_slice()) {
            ([range], [end]) if range.end == *end => Some(*end),
            _ => None,
        };

        let mut document = Document {
            src,
            starts,
            rows: Vec::new(),
            shapes: Shapes::default(),
            layers: Layers::default(),
            charset: Charset::Other,
            header: None,
            locked: None,
            counts: Counts {
                records: records as u32,
                entities: entities.iter().map(|range| range.end - range.start).sum(),
                ..Counts::default()
            },
            object_handles: Vec::new(),
            version: Vec::new(),
            markers: false,
            names: Default::default(),
            names_exact: true,
            model_owner: None,
            definitions: Vec::new(),
            definition_names: BTreeMap::new(),
            usable_definitions: BTreeMap::new(),
            members: Vec::new(),
            block_display: Display::default(),
            plain: true,
            single_tables: SingleTables::default(),
            flags: Vec::new(),
            sections: Vec::new(),
            entities: Vec::new(),
            entities_end,
            objects_by_handle: BTreeMap::new(),
            object_refs: Vec::new(),
            header_handles: BTreeSet::new(),
            refs: Vec::new(),
            fixed_largest: None,
            row_largest: None,
            gone: BTreeSet::new(),
            added: BTreeMap::new(),
            foreign: BTreeSet::new(),
            overlay: BTreeMap::new(),
            undo: Vec::new(),
            redo: Vec::new(),
            display: Display::default(),
            boxes: Vec::new(),
            handles: Handles::None,
            grid: Grid::default(),
        };
        document.read_header(header);
        // 名前の表と、画層の表。表の順に番号を振る
        for &(record, mark) in &marks {
            if let Mark::Table(kind) = mark
                && inside(&sections, record)
            {
                let in_tables = inside(&tables, record);
                document.read_name(usize::from(kind), record, in_tables);
                if usize::from(kind) == table::LAYER && in_tables {
                    document.read_layer(record);
                }
            }
        }
        // R13 からの DXF と、サブクラスの印を持つ図形のある図面では、足すレコードにも印を書く
        document.markers = document.version.as_slice() >= b"AC1012".as_slice()
            || entities.iter().any(|range| {
                flags
                    .get(range.start as usize..range.end as usize)
                    .is_some_and(|flags| flags.iter().any(|&f| f & 1 != 0))
            });
        for &(record, mark) in &marks {
            if inside(&entities, record) {
                match mark {
                    Mark::Paper => document.counts.paper += 1,
                    Mark::Dropped => document.counts.dropped += 1,
                    _ => {}
                }
            }
        }

        // 行と形。ENTITIES のセクションにあって、文字を読めるものだけを要素にする。
        // BLOCKS のセクションのものは、定義の中身にする
        let total: usize = parts.iter().map(|part| part.rows.len()).sum();
        document.rows.reserve(total);
        let mut owner_runs: Vec<(u64, u32, u32)> = Vec::new();
        let mut handle_refs: Vec<(u64, u32)> = Vec::new();
        let mut record_base = 0u32;
        for part in parts {
            owner_runs.extend(
                part.owner_runs
                    .iter()
                    .map(|&(h, s, e)| (h, record_base + s, record_base + e)),
            );
            handle_refs.extend(
                part.handle_refs
                    .iter()
                    .map(|&(h, rec)| (h, record_base + rec)),
            );
            let shapes = &mut document.shapes;
            let bases = [
                shapes.lines.len() as u32,
                shapes.circles.len() as u32,
                shapes.arcs.len() as u32,
                shapes.polys.len() as u32,
                shapes.texts.len() as u32,
                shapes.inserts.len() as u32,
            ];
            let vertex_base = shapes.pools.vertices.len() as u32;
            let width_base = shapes.pools.widths.len() as u32;
            let text_base = shapes.pools.text.len() as u32;
            shapes.lines.extend_from_slice(&part.shapes.lines);
            shapes.circles.extend_from_slice(&part.shapes.circles);
            shapes.arcs.extend_from_slice(&part.shapes.arcs);
            shapes
                .polys
                .extend(part.shapes.polys.iter().map(|poly| Poly {
                    first: poly.first + vertex_base,
                    ..*poly
                }));
            let moved = |(start, end): (u32, u32)| (start + text_base, end + text_base);
            shapes
                .texts
                .extend(part.shapes.texts.iter().map(|text| TextRow {
                    content: moved(text.content),
                    style: text.style.map(moved),
                    ..*text
                }));
            shapes
                .inserts
                .extend(part.shapes.inserts.iter().map(|insert| InsertRow {
                    name: moved(insert.name),
                    ..*insert
                }));
            shapes
                .pools
                .vertices
                .extend_from_slice(&part.shapes.pools.vertices);
            shapes
                .pools
                .widths
                .extend_from_slice(&part.shapes.pools.widths);
            shapes.pools.width_first.extend(
                part.shapes
                    .pools
                    .width_first
                    .iter()
                    .map(|&(first, start)| (first + vertex_base, start + width_base)),
            );
            shapes.pools.text.extend_from_slice(&part.shapes.pools.text);
            let layers: Vec<u32> = part
                .layers
                .iter()
                .map(|name| document.layers.intern(name))
                .collect();
            for row in &part.rows {
                let record = record_base + row.record;
                let in_entities = inside(&entities, record);
                if !in_entities && !inside(&blocks, record) {
                    continue;
                }
                match (row.class, document.charset) {
                    (class::ASCII, _) | (class::UTF8, Charset::Utf8) => {}
                    (_, Charset::ShiftJis) => {
                        document.counts.shift_jis += u32::from(in_entities);
                        continue;
                    }
                    _ => {
                        document.counts.unreadable += u32::from(in_entities);
                        continue;
                    }
                }
                let placed = Row {
                    record,
                    geom: row.geom + bases.get(row.kind as usize).copied().unwrap_or(0),
                    layer: layers.get(row.layer as usize).copied().unwrap_or(0),
                    ..*row
                };
                if in_entities {
                    document.counts.unaddressable += u32::from(row.handle == 0);
                    document.rows.push(placed);
                } else {
                    document.members.push(placed);
                }
            }
            record_base += part.starts.len() as u32;
        }
        document.counts.elements = document.rows.len() as u32;
        document.object_handles = handles
            .into_iter()
            .filter(|(record, _)| inside(&sections, *record))
            .collect();
        let mut seen_handles = BTreeSet::new();
        for &(record, handle) in &document.object_handles {
            if seen_handles.insert(handle) && inside(&objects, record) {
                document.objects_by_handle.insert(handle, record);
            }
        }
        if !document.objects_by_handle.is_empty() {
            let mut object_refs: Vec<(u64, u32)> = Vec::new();
            for &(record, handle) in &document.object_handles {
                if document.objects_by_handle.contains_key(&handle) {
                    object_refs.push((handle, record));
                }
            }
            for (handle, start, end) in owner_runs {
                if document.objects_by_handle.contains_key(&handle) {
                    for rec in start..end {
                        if inside(&sections, rec) {
                            object_refs.push((handle, rec));
                        }
                    }
                }
            }
            for (handle, rec) in handle_refs {
                if document.objects_by_handle.contains_key(&handle) && inside(&sections, rec) {
                    object_refs.push((handle, rec));
                }
            }
            object_refs.sort_unstable();
            object_refs.dedup();
            document.object_refs = object_refs;
        }
        refs.retain(|(_, record)| inside(&sections, *record));
        refs.sort_unstable();
        refs.dedup();
        document.refs = refs;
        document.flags = flags;
        document.sections = sections;
        family.retain(|(record, _)| inside(&entities, *record));
        document.attach(&family, &entities);
        document.read_definitions(&marks, &blocks);
        document.entities = entities;
        document.build_single_tables();
        Ok(document)
    }

    /// 属性（ATTRIB）を INSERT に付ける。今の実装の `attached`（`crates/zumen-acad/src/drawing.rs`）と同じ規則で、
    /// 所有者（330）が ENTITIES のオブジェクトを指せばそれを、指さなければ、続くフラグ（66）で開いた直前の
    /// INSERT か POLYLINE を親にする。`family` は、ENTITIES の INSERT・POLYLINE・ATTRIB・VERTEX・SEQEND
    fn attach(&mut self, family: &[(u32, Followed)], entities: &[Range<u32>]) {
        // 所有者が指すレコード。ENTITIES の、そのハンドルを持つ最後のオブジェクト
        let mut owners: BTreeMap<u64, Option<u32>> = family
            .iter()
            .filter(|(_, found)| {
                matches!(found.kind, Follow::Attrib | Follow::Vertex | Follow::Seqend)
            })
            .filter_map(|(_, found)| Some((found.owner?, None)))
            .collect();
        if !owners.is_empty() {
            for &(record, handle) in &self.object_handles {
                if inside(entities, record)
                    && let Some(place) = owners.get_mut(&handle)
                {
                    *place = Some(record);
                }
            }
        }
        // ENTITIES の並びの中の順番。セクションが 2 つあれば、続けて数える
        let mut offsets = Vec::with_capacity(entities.len());
        let mut before = 0u32;
        for range in entities {
            offsets.push(before);
            before += range.end - range.start;
        }
        let position = |record: u32| {
            let at = entities.partition_point(|range| range.end <= record);
            let range = entities.get(at).filter(|range| range.start <= record)?;
            Some(offsets.get(at).copied().unwrap_or(0) + (record - range.start))
        };
        let kind_of = |record: u32| {
            let at = family.binary_search_by_key(&record, |(key, _)| *key).ok()?;
            family.get(at).map(|(_, found)| found.kind)
        };
        // 続くレコードを受ける親と、直前の INSERT・POLYLINE・ATTRIB・VERTEX・SEQEND の順番
        let mut open: Option<u32> = None;
        let mut last: Option<u32> = None;
        let mut pairs: Vec<(u32, u32)> = Vec::new();
        let mut plain = true;
        for &(record, found) in family {
            let Some(at) = position(record) else {
                continue;
            };
            // 間にほかの種類のレコードがあれば、親は閉じている
            if last.is_none_or(|last| last + 1 != at) {
                open = None;
            }
            last = Some(at);
            match found.kind {
                Follow::Insert { opens } | Follow::Polyline { opens } => {
                    open = opens.then_some(record);
                }
                Follow::Attrib | Follow::Vertex | Follow::Seqend => {
                    let by_owner = found
                        .owner
                        .and_then(|owner| owners.get(&owner).copied().flatten());
                    let parent = by_owner.or(open).filter(|parent| {
                        kind_of(*parent)
                            .is_some_and(|parent| single::followed_expected(found.kind, parent))
                    });
                    plain &= parent.is_some()
                        && parent == open
                        && (found.owner.is_none() || by_owner == open);
                    if let Some(parent) = parent
                        && found.kind == Follow::Attrib
                    {
                        pairs.push((parent, record));
                    }
                    if found.kind == Follow::Seqend {
                        open = None;
                    }
                }
            }
        }
        self.plain = plain;

        // INSERT の行ごとに、付いた属性を読む。読めないものは数だけ覚える（今の実装は、動かす変更を拒む）
        pairs.sort_unstable();
        for group in pairs.chunk_by(|a, b| a.0 == b.0) {
            let Some(&(parent, _)) = group.first() else {
                continue;
            };
            let Ok(at) = self.rows.binary_search_by_key(&parent, |row| row.record) else {
                continue;
            };
            let Some(row) = self.rows.get(at).filter(|row| row.kind == Kind::Insert) else {
                continue;
            };
            let geom = row.geom as usize;
            let first = self.shapes.pools.attribs.len() as u32;
            for &(_, record) in group {
                if let Some(attribute) = self.read_attrib(record) {
                    self.shapes.pools.attribs.push(attribute);
                } else if self.charset == Charset::ShiftJis {
                    self.foreign.insert(parent);
                }
            }
            let count = self.shapes.pools.attribs.len() as u32 - first;
            if let Some(insert) = self.shapes.inserts.get_mut(geom) {
                insert.attribs = (first, count);
                insert.attached = group.len() as u32;
            }
            self.counts.attribs += group.len() as u32;
        }
    }

    /// レコードのタグ。最初のタグ（0 と種類）を除く
    pub(crate) fn raw_tags<'a>(&'a self, tags: &'a [OTag]) -> Vec<RawTag<'a>> {
        tags.iter()
            .skip(1)
            .map(|tag| RawTag {
                code: tag.code,
                value: self.value_of(tag),
            })
            .collect()
    }

    /// 属性（ATTRIB）のレコードを読む。読めないか、文字をこの図面の符号化で読めなければ `None`
    pub(crate) fn read_attrib(&mut self, record: u32) -> Option<AttribRow> {
        let tags = match self.overlay.get(&record) {
            Some(tags) => tags.clone(),
            None => tokenize(&self.src, self.record_range(record)),
        };
        let raw: Vec<RawTag<'_>> = tags
            .iter()
            .skip(1)
            .map(|tag| RawTag {
                code: tag.code,
                value: tag_value(&self.src, tag),
            })
            .collect();
        let mut attribute = lift::attrib(&raw, record, &mut self.shapes.pools)?;
        if !matches!(
            (attribute.class, self.charset),
            (class::ASCII, _) | (class::UTF8, Charset::Utf8)
        ) {
            return None;
        }
        attribute.layer = self
            .layers
            .intern(self.shapes.pools.text_of(attribute.layer_name));
        Some(attribute)
    }

    /// 文字の値を読む。UTF-8 の図面は UTF-8、ほかは ASCII だけ（今の実装は、Shift_JIS の図面の文字も読む）
    pub(crate) fn decode<'a>(&self, raw: &'a [u8]) -> Option<&'a str> {
        if self.charset == Charset::Utf8 || raw.is_ascii() {
            core::str::from_utf8(raw).ok()
        } else {
            None
        }
    }

    /// 名前の表のレコードを読む（今の実装の `Born::new`）。名前は、読める最初の 2 の値。
    /// 同じ名前（Unicode の小文字にして同じ）がいくつかあれば、最後の綴りを残す
    fn read_name(&mut self, kind: usize, record: u32, in_tables: bool) {
        let tags = tokenize(&self.src, self.record_range(record));
        let raw = self.raw_tags(&tags);
        let Some(name) = raw.iter().find(|tag| tag.code == 2).map(|tag| tag.value) else {
            return;
        };
        let Some(text) = self.decode(name) else {
            if self.charset == Charset::ShiftJis {
                self.names_exact = false;
            }
            return;
        };
        let key = text.to_lowercase();
        let model = kind == table::BLOCK_RECORD && text.eq_ignore_ascii_case("*Model_Space");
        let handle = handle_of(TABLE_KINDS.get(kind).copied().unwrap_or_default(), &raw);
        let spelling: Box<[u8]> = name.into();
        if model {
            self.model_owner = handle;
        }
        if let Some(names) = self.names.get_mut(kind) {
            let entry = names
                .get_or_insert_with(BTreeMap::new)
                .entry(key)
                .or_insert(Spelling {
                    name: Box::default(),
                    count: 0,
                    tables: 0,
                });
            entry.name = spelling;
            entry.count += 1;
            entry.tables += u32::from(in_tables);
        }
    }

    /// ブロックの定義を読む。中身は、BLOCK から ENDBLK か次の BLOCK までのレコード（今の実装は、所有者（330）が
    /// 指す定義を先に見る。中身は表示と箱にだけ使い、属性の定義の有無は、広めに取る）
    fn read_definitions(&mut self, marks: &[(u32, Mark)], blocks: &[Range<u32>]) {
        struct Found {
            record: u32,
            end: u32,
            owner: Option<u64>,
            key: Option<String>,
            name: Box<[u8]>,
            base: [f64; 2],
            head: bool,
            attdefs: bool,
        }
        let mut found: Vec<Found> = Vec::new();
        let mut attdefs: Vec<(u32, Option<usize>)> = Vec::new();
        let mut open: Option<usize> = None;
        for &(record, mark) in marks {
            if !inside(blocks, record) {
                continue;
            }
            match mark {
                Mark::Block => {
                    if let Some(previous) = open.and_then(|at| found.get_mut(at)) {
                        previous.end = record;
                    }
                    let tags = tokenize(&self.src, self.record_range(record));
                    let raw = self.raw_tags(&tags);
                    let first = |code: i16| raw.iter().find(|tag| tag.code == code);
                    // 有限の実数。値がなければ `None`、読めなければ `Some(None)`
                    let number = |code: i16| {
                        first(code).map(|tag| match typed(code, tag.value) {
                            Val::Real(value) if value.is_finite() => Some(value),
                            _ => None,
                        })
                    };
                    // 基点は、10 がなければ原点。あれば、平面の上の点であること
                    let base = match number(10) {
                        None => Some([0.0, 0.0]),
                        Some(x) => match (x, number(20).flatten(), number(30)) {
                            (Some(x), Some(y), None | Some(Some(0.0))) => Some([x, y]),
                            _ => None,
                        },
                    };
                    // 外部参照（70 の 4）は、足す変更で指さない
                    let local = match first(70).map(|tag| typed(70, tag.value)) {
                        None => true,
                        Some(Val::Int(flags)) => flags & 4 == 0,
                        Some(_) => false,
                    };
                    let name = first(2).map(|tag| tag.value).unwrap_or_default();
                    let key = self
                        .decode(name)
                        .filter(|name| !name.is_empty() && first(2).is_some())
                        .map(str::to_lowercase);
                    open = Some(found.len());
                    found.push(Found {
                        record,
                        end: u32::MAX,
                        owner: owner_of(&raw),
                        key,
                        name: name.into(),
                        base: base.unwrap_or([0.0, 0.0]),
                        head: base.is_some() && local,
                        attdefs: false,
                    });
                }
                Mark::EndBlock => {
                    if let Some(previous) = open.take().and_then(|at| found.get_mut(at)) {
                        previous.end = record;
                    }
                }
                Mark::Attdef => attdefs.push((record, open)),
                _ => {}
            }
        }
        // 属性の定義が入る定義。並びで開いている定義と、所有者が同じ BLOCK の定義。
        // 所有者があって、どの BLOCK の所有者とも合わなければ、どの定義に入るかを決めない
        let mut unclear = false;
        for (record, open) in attdefs {
            if let Some(definition) = open.and_then(|at| found.get_mut(at)) {
                definition.attdefs = true;
            }
            let tags = tokenize(&self.src, self.record_range(record));
            let owner = owner_of(&self.raw_tags(&tags));
            if owner.is_some() {
                let mut hit = false;
                for definition in found.iter_mut().filter(|found| found.owner == owner) {
                    definition.attdefs = true;
                    hit = true;
                }
                unclear |= !hit;
            }
        }
        // 名前を読めない BLOCK があれば、どの名前とも重なりうる
        unclear |= found.iter().any(|found| found.key.is_none());
        let mut counts: BTreeMap<&str, u32> = BTreeMap::new();
        for key in found.iter().filter_map(|found| found.key.as_deref()) {
            *counts.entry(key).or_default() += 1;
        }
        let records = self.names.get(table::BLOCK_RECORD).and_then(Option::as_ref);
        let mut definitions = Vec::with_capacity(found.len());
        for (at, found) in found.iter().enumerate() {
            let layout = found.key.as_deref().is_some_and(|key| {
                key.starts_with("*model_space") || key.starts_with("*paper_space")
            });
            let usable = !unclear
                && self.names_exact
                && found.head
                && !found.attdefs
                && !layout
                && found.key.as_deref().is_some_and(|key| {
                    counts.get(key) == Some(&1)
                        && records.is_none_or(|names| names.contains_key(key))
                });
            // 中身の行と、その箱
            let first = self
                .members
                .partition_point(|row| row.record <= found.record);
            let end = self.members.partition_point(|row| row.record < found.end);
            let mut bounds = EMPTY;
            for row in self.members.get(first..end).unwrap_or_default() {
                if let Some(geom) = self.shapes.geom(row.kind, row.geom) {
                    let [left, bottom, right, top] = self::bounds(&self.shapes.pools, &[], &geom);
                    if left <= right && bottom <= top {
                        reach(&mut bounds, [left, bottom]);
                        reach(&mut bounds, [right, top]);
                    }
                }
            }
            if !found.name.is_empty() {
                self.definition_names
                    .entry(found.name.to_ascii_lowercase().into())
                    .or_insert(at as u32);
            }
            if usable && let Some(key) = &found.key {
                self.usable_definitions.insert(key.clone(), at as u32);
            }
            definitions.push(Definition {
                name: found.name.clone(),
                record: found.record,
                base: found.base,
                members: (first as u32, end.saturating_sub(first) as u32),
                bounds: if bounds[0] <= bounds[2] {
                    bounds
                } else {
                    NO_BOX
                },
                attdefs: found.attdefs,
                usable,
            });
        }
        self.counts.definitions = definitions.len() as u32;
        self.definitions = definitions;
        // INSERT の行が指す定義
        for at in 0..self.shapes.inserts.len() {
            if let Some(insert) = self.shapes.inserts.get(at).copied() {
                let definition = self.definition_of(&insert);
                if let Some(insert) = self.shapes.inserts.get_mut(at) {
                    insert.definition = definition;
                }
            }
        }
    }

    /// 記号が指す定義の番号。名前は、ASCII の大文字と小文字を区別しない。なければ `NO_DEFINITION`
    pub(crate) fn definition_of(&self, insert: &InsertRow) -> u32 {
        let name = self.shapes.pools.text_of(insert.name).to_ascii_lowercase();
        self.definition_names
            .get(name.as_slice())
            .copied()
            .unwrap_or(NO_DEFINITION)
    }

    /// `zumen_acad::single` の共通関数に渡す名前の表と設定を組む
    fn build_single_tables(&mut self) {
        let mut names: BTreeMap<&'static [u8], BTreeMap<String, String>> = BTreeMap::new();
        for (&kind, map) in TABLE_KINDS.iter().zip(&self.names) {
            let Some(map) = map else { continue };
            let mut converted = BTreeMap::new();
            for (key, spelling) in map {
                if let Some(decoded) = self.decode(&spelling.name) {
                    converted.insert(key.clone(), decoded.to_owned());
                }
            }
            names.insert(kind, converted);
        }
        let mut blocks = BTreeMap::new();
        for (key, &at) in &self.usable_definitions {
            if let Some(def) = self.definitions.get(at as usize)
                && let Some(decoded) = self.decode(&def.name)
            {
                blocks.insert(key.clone(), decoded.to_owned());
            }
        }
        self.single_tables = SingleTables {
            model_owner: self.model_owner,
            markers: self.markers,
            version: self.version.clone(),
            charset: self.charset,
            dwg_seed: None,
            names: Arc::new(names),
            blocks: Arc::new(blocks),
        };
    }

    /// HEADER のレコードの変数（9 の名前と、続く値）から、文字の符号化と、変更で見る変数を読む
    fn read_header(&mut self, record: Option<u32>) {
        let Some(record) = record else {
            return;
        };
        let tags = tokenize(&self.src, self.record_range(record));
        let mut header = Header {
            record,
            ..Header::default()
        };
        let mut version: Option<Vec<u8>> = None;
        let mut code_page: Option<Vec<u8>> = None;
        let mut header_handles: BTreeSet<u64> = BTreeSet::new();
        let mut seen: BTreeSet<Vec<u8>> = BTreeSet::new();
        let mut start = 0;
        for chunk in tags.chunk_by(|_, next| next.code != 9) {
            let first = start;
            start += chunk.len();
            let Some((head, rest)) = chunk.split_first() else {
                continue;
            };
            if head.code != 9 {
                continue;
            }
            let name = self.value_of(head).trim_ascii();
            // 同じ名前が 2 度あれば、最初のもの
            if !seen.insert(name.to_vec()) {
                continue;
            }
            if name != b"$HANDSEED" {
                for tag in rest {
                    if DataType::of(tag.code) == DataType::Handle
                        && let Val::Handle(handle) = typed(tag.code, self.value_of(tag))
                    {
                        header_handles.insert(handle);
                    }
                }
            }
            let wanted: [&[u8]; 5] = [
                b"$ACADVER",
                b"$DWGCODEPAGE",
                b"$EXTMIN",
                b"$EXTMAX",
                b"$HANDSEED",
            ];
            if !wanted.contains(&name) {
                continue;
            }
            let values: Vec<Value> = rest
                .iter()
                .map(|tag| value(tag.code, self.value_of(tag)))
                .collect();
            let text = || match values.first().map(|value| &value.data) {
                Some(zumen_acad::Data::Text(text)) => Some(text.trim_ascii().to_vec()),
                _ => None,
            };
            match name {
                b"$ACADVER" => version = text(),
                b"$DWGCODEPAGE" => code_page = text(),
                _ => {
                    let variable = Some(Variable {
                        start: first + 1,
                        values,
                    });
                    match name {
                        b"$EXTMIN" => header.extmin = variable,
                        b"$EXTMAX" => header.extmax = variable,
                        _ => header.seed = variable,
                    }
                }
            }
        }
        self.charset = zumen_acad::charset(version.as_deref(), code_page.as_deref());
        self.version = version.unwrap_or_default();
        self.header_handles = header_handles;
        self.header = Some(header);
    }

    /// 画層の表のレコードを読む
    fn read_layer(&mut self, record: u32) {
        let tags = tokenize(&self.src, self.record_range(record));
        let first = |code: i16| {
            tags.iter()
                .skip(1)
                .find(|tag| tag.code == code)
                .map(|tag| self.value_of(tag))
        };
        let Some(name) = first(2) else {
            return;
        };
        let int = |code: i16, default: i64| match first(code).map(|raw| typed(code, raw)) {
            Some(Val::Int(value)) => value,
            _ => default,
        };
        let (flags, color) = (int(70, 0), int(62, 7));
        let line_type: Box<[u8]> = first(6).unwrap_or_default().into();
        let name = name.to_vec();
        let id = self.layers.intern(&name);
        if let Some(layer) = self.layers.list.get_mut(id as usize)
            && layer.record.is_none()
        {
            layer.record = Some(record);
            layer.flags = flags;
            layer.color = color;
            layer.line_type = line_type;
        }
    }

    /// レコードのバイトの範囲
    pub fn record_range(&self, record: u32) -> Range<usize> {
        range_of(&self.starts, record)
    }

    /// タグの今の値
    pub(crate) fn value_of<'a>(&'a self, tag: &'a OTag) -> &'a [u8] {
        tag_value(&self.src, tag)
    }

    /// 行の形
    pub fn geom(&self, row: u32) -> Option<Geom> {
        let row = self.rows.get(row as usize)?;
        self.shapes.geom(row.kind, row.geom)
    }

    /// 表示のバッファーを作る。定義の中身は、別のバッファーに 1 回だけ置く
    pub fn build_display(&mut self) {
        let mut display = Display::with_rows(self.rows.len());
        for row in &self.rows {
            if let Some(geom) = self.shapes.geom(row.kind, row.geom) {
                display.push(&geom, &self.shapes.pools, [row.layer, row.color]);
            }
        }
        self.display = display;
        let mut blocks = Display::with_rows(self.members.len());
        for row in &self.members {
            if let Some(geom) = self.shapes.geom(row.kind, row.geom) {
                blocks.push(&geom, &self.shapes.pools, [row.layer, row.color]);
            }
        }
        self.block_display = blocks;
    }

    /// ハンドルの表と、範囲の索引を作る。変更を受け付けるかも、ここで決める
    pub fn build_index(&mut self) {
        // 同じハンドル（0 でないもの）を持つオブジェクトがあれば、変更を受け付けない
        let objects = core::mem::take(&mut self.object_handles);
        let handles: Vec<u64> = objects
            .iter()
            .map(|(_, handle)| *handle)
            .filter(|handle| *handle != 0)
            .collect();
        let largest = handles.iter().copied().max().unwrap_or(0);
        let dense = |count: usize| largest <= (count as u64).saturating_mul(8).saturating_add(1024);
        let duplicate = if dense(handles.len()) {
            let mut seen = vec![0u64; largest as usize / 64 + 1];
            handles.iter().any(|&handle| {
                let (word, bit) = (handle as usize / 64, 1u64 << (handle % 64));
                let place = &mut seen[word];
                let was = *place & bit != 0;
                *place |= bit;
                was
            })
        } else {
            let mut sorted = handles.clone();
            sorted.sort_unstable();
            sorted.windows(2).any(|pair| pair.first() == pair.get(1))
        };
        self.locked = duplicate.then_some(Locked::DuplicateHandle);

        // ハンドルの最大。変更で指せる行・一緒に除きうる OBJECTS のものと、常に残るオブジェクトのものを分けて持つ
        let mut rows = self.rows.iter().peekable();
        let mut fixed = None;
        for &(record, handle) in &objects {
            while rows.next_if(|row| row.record < record).is_some() {}
            let addressed = handle != 0 && rows.peek().is_some_and(|row| row.record == record);
            let removable_obj = handle != 0 && self.objects_by_handle.get(&handle) == Some(&record);
            if !addressed && !removable_obj {
                fixed = fixed.max(Some(handle));
            }
        }
        self.fixed_largest = fixed;
        self.row_largest = self
            .rows
            .iter()
            .map(|row| row.handle)
            .filter(|handle| *handle != 0)
            .max();

        let largest = self.row_largest.unwrap_or(0);
        self.handles = if dense(self.rows.len()) {
            let mut table = vec![0u32; largest as usize + 1];
            for (at, row) in self.rows.iter().enumerate() {
                if row.handle != 0
                    && let Some(place) = table.get_mut(row.handle as usize)
                    && *place == 0
                {
                    *place = at as u32 + 1;
                }
            }
            Handles::Dense(table)
        } else {
            let mut table: Vec<(u64, u32)> = (0u32..)
                .zip(&self.rows)
                .filter(|(_, row)| row.handle != 0)
                .map(|(at, row)| (row.handle, at))
                .collect();
            // 同じハンドルは、先の行を残す（並べ替えは、同じ値の順を変えない）
            table.sort_by_key(|(handle, _)| *handle);
            table.dedup_by_key(|(handle, _)| *handle);
            Handles::Sparse(table)
        };

        let boxes: Vec<[f64; 4]> = self
            .rows
            .iter()
            .map(|row| match self.shapes.geom(row.kind, row.geom) {
                Some(geom) => bounds(&self.shapes.pools, &self.definitions, &geom),
                None => NO_BOX,
            })
            .collect();
        self.grid = Grid::build(&boxes);
        self.boxes = boxes;
    }

    /// オブジェクトのハンドルの最大。ハンドルを持つオブジェクトがなければ `None`
    pub(crate) fn largest_handle(&self) -> Option<u64> {
        let added = self.added.last_key_value().map(|(handle, _)| *handle);
        let live_obj = self
            .objects_by_handle
            .iter()
            .rev()
            .filter(|&(handle, _)| *handle != 0)
            .find(|(_, record)| !self.gone.contains(record))
            .map(|(handle, _)| *handle);
        self.fixed_largest
            .max(self.row_largest)
            .max(live_obj)
            .max(added)
    }

    /// 生きている元の行のハンドルの最大を、今の値から下へ探し直す。最大の行を除いたときに呼ぶ
    pub(crate) fn lower_row_largest(&mut self) {
        let Some(top) = self.row_largest else {
            return;
        };
        let alive = |row: u32| self.rows.get(row as usize).is_some_and(|row| row.alive);
        self.row_largest = match &self.handles {
            Handles::None => None,
            Handles::Dense(table) => (1..=top).rev().find(|handle| {
                table
                    .get(*handle as usize)
                    .and_then(|at| at.checked_sub(1))
                    .is_some_and(alive)
            }),
            Handles::Sparse(table) => {
                let end = table.partition_point(|(handle, _)| *handle <= top);
                table
                    .get(..end)
                    .unwrap_or_default()
                    .iter()
                    .rev()
                    .find(|(_, row)| alive(*row))
                    .map(|(handle, _)| *handle)
            }
        };
    }

    /// 生きている行の表示のバッファーを、行の順に並べたもの。開き直した文書と比べるのに使う
    pub fn live_display(&self) -> Vec<RowBuffers> {
        (0u32..)
            .zip(&self.rows)
            .filter(|(_, row)| row.alive)
            .map(|(at, _)| self.display.row(at))
            .collect()
    }

    /// 生きている行の箱を、行の順に並べたもの
    pub fn live_boxes(&self) -> Vec<[f64; 4]> {
        self.rows
            .iter()
            .zip(&self.boxes)
            .filter(|(row, _)| row.alive)
            .map(|(_, item)| *item)
            .collect()
    }

    /// 生きている行の数
    pub fn live_len(&self) -> usize {
        self.rows.len() - self.gone_rows()
    }

    /// 除いて、取り消しのために残している行の数
    pub fn gone_rows(&self) -> usize {
        self.rows.iter().filter(|row| !row.alive).count()
    }

    /// 箱が `rect`（最小の x・y、最大の x・y）と重なる要素の行を、ファイルの順に返す
    pub fn select(&self, rect: [f64; 4], out: &mut Vec<u32>) {
        self.grid.query(&self.boxes, rect, out);
    }

    /// ハンドルから、生きている行を引く。足した行を先に見る
    pub fn row_of(&self, handle: u64) -> Option<u32> {
        let row = self
            .added
            .get(&handle)
            .copied()
            .or_else(|| self.handles.get(handle))?;
        self.rows
            .get(row as usize)
            .is_some_and(|row| row.alive)
            .then_some(row)
    }

    /// 名前の表（`table` の番号で指す）にある名前の綴り。照らすツールが、変更で指す名前を選ぶのに使う
    pub fn table_names(&self, table: usize) -> Vec<String> {
        self.names
            .get(table)
            .and_then(Option::as_ref)
            .map_or_else(Vec::new, |names| {
                names
                    .values()
                    .map(|spelling| String::from_utf8_lossy(&spelling.name).into_owned())
                    .collect()
            })
    }

    /// 元のレコードの数。これから先の番号は、足したレコード
    pub fn source_records(&self) -> u32 {
        self.counts.records
    }

    /// レコードの種類（最初のタグの値）
    pub(crate) fn kind_name(&self, record: u32) -> &[u8] {
        match self.overlay.get(&record).and_then(|tags| tags.first()) {
            Some(tag) => self.value_of(tag).trim_ascii(),
            None => match scan::next_tag(&self.src, self.record_range(record).start) {
                Next::Tag(span) if span.code == 0 => self
                    .src
                    .get(span.value as usize..span.value_end as usize)
                    .unwrap_or_default()
                    .trim_ascii(),
                _ => &[],
            },
        }
    }

    /// レコードが ENTITIES のセクションにあるか。足したレコードは、ENTITIES にある
    pub(crate) fn in_entities(&self, record: u32) -> bool {
        record >= self.counts.records || inside(&self.entities, record)
    }

    /// レコードがいずれかのセクションの中にあるか。足したレコードは、ENTITIES にある
    pub(crate) fn in_section(&self, record: u32) -> bool {
        record >= self.counts.records || inside(&self.sections, record)
    }

    /// 元のバイト列の長さ
    pub fn source_len(&self) -> usize {
        self.src.len()
    }

    /// 取り消せる変更の数
    pub fn undo_len(&self) -> usize {
        self.undo.len()
    }

    /// 変えたレコードの数
    pub fn edited_records(&self) -> usize {
        self.overlay.len()
    }

    /// 元のバイト列のほかに持つ、列・表示・索引のおよそのバイト数
    pub fn resident_bytes(&self) -> usize {
        let shapes = &self.shapes;
        self.starts.len() * 4
            + self.flags.len()
            + self.rows.len() * core::mem::size_of::<Row>()
            + shapes.lines.len() * 32
            + shapes.circles.len() * 24
            + shapes.arcs.len() * 40
            + shapes.polys.len() * core::mem::size_of::<Poly>()
            + shapes.texts.len() * core::mem::size_of::<TextRow>()
            + shapes.inserts.len() * core::mem::size_of::<InsertRow>()
            + shapes.pools.attribs.len() * core::mem::size_of::<AttribRow>()
            + self.members.len() * core::mem::size_of::<Row>()
            + self.definitions.len() * core::mem::size_of::<Definition>()
            + self.refs.len() * 16
            + self.object_refs.len() * 16
            + self.objects_by_handle.len() * 16
            + self.block_display.bytes()
            + shapes.pools.vertices.len() * 24
            + shapes.pools.widths.len() * 16
            + shapes.pools.text.len()
            + self.display.bytes()
            + self.boxes.len() * 32
            + self.grid.bytes()
            + match &self.handles {
                Handles::None => 0,
                Handles::Dense(table) => table.len() * 4,
                Handles::Sparse(table) => table.len() * 16,
            }
    }

    /// 列だけを比べる。文字と画層は中身で比べるので、開き直した文書とも比べられる
    pub fn same_content(&self, other: &Document) -> bool {
        if self.live_len() != other.live_len() || self.charset != other.charset {
            return false;
        }
        let name = |document: &Document, layer: u32| {
            document
                .layers
                .list
                .get(layer as usize)
                .map(|layer| layer.name.to_ascii_lowercase())
        };
        // 文字の行を、中身で比べる
        let same_text = |text: &TextRow, other_text: &TextRow| {
            let (pools, other_pools) = (&self.shapes.pools, &other.shapes.pools);
            (
                text.place,
                text.height,
                text.rotation,
                text.width_factor,
                text.oblique,
                text.flags,
            ) == (
                other_text.place,
                other_text.height,
                other_text.rotation,
                other_text.width_factor,
                other_text.oblique,
                other_text.flags,
            ) && pools.text_of(text.content) == other_pools.text_of(other_text.content)
                && text.style.map(|range| pools.text_of(range))
                    == other_text.style.map(|range| other_pools.text_of(range))
        };
        fn live(document: &Document) -> impl Iterator<Item = &Row> {
            document.rows.iter().filter(|row| row.alive)
        }
        live(self).zip(live(other)).all(|(a, b)| {
            if (a.handle, a.kind, a.color, a.class) != (b.handle, b.kind, b.color, b.class)
                || name(self, a.layer) != name(other, b.layer)
            {
                return false;
            }
            match (
                self.shapes.geom(a.kind, a.geom),
                other.shapes.geom(b.kind, b.geom),
            ) {
                (
                    Some(Geom::Polyline {
                        first,
                        count,
                        flags,
                        width,
                    }),
                    Some(Geom::Polyline {
                        first: other_first,
                        count: other_count,
                        flags: other_flags,
                        width: other_width,
                    }),
                ) => {
                    let vertices = |document: &'_ Document, first: u32, count: u32| {
                        document
                            .shapes
                            .pools
                            .vertices
                            .get(first as usize..(first + count) as usize)
                            .map(<[_]>::to_vec)
                    };
                    (count, flags, width) == (other_count, other_flags, other_width)
                        && vertices(self, first, count) == vertices(other, other_first, other_count)
                        && self.shapes.pools.widths_of(first, count)
                            == other.shapes.pools.widths_of(other_first, other_count)
                }
                (Some(Geom::Text(text)), Some(Geom::Text(other_text))) => {
                    same_text(&text, &other_text)
                }
                (Some(Geom::Insert(insert)), Some(Geom::Insert(other_insert))) => {
                    let (pools, other_pools) = (&self.shapes.pools, &other.shapes.pools);
                    let (attributes, other_attributes) = (
                        pools.attribs_of(&insert),
                        other_pools.attribs_of(&other_insert),
                    );
                    (
                        insert.definition,
                        insert.position,
                        insert.scale,
                        insert.rotation,
                        insert.grid,
                        insert.spacing,
                        insert.attached,
                        insert.followed,
                    ) == (
                        other_insert.definition,
                        other_insert.position,
                        other_insert.scale,
                        other_insert.rotation,
                        other_insert.grid,
                        other_insert.spacing,
                        other_insert.attached,
                        other_insert.followed,
                    ) && pools.text_of(insert.name) == other_pools.text_of(other_insert.name)
                        && attributes.len() == other_attributes.len()
                        && attributes.iter().zip(other_attributes).all(|(a, b)| {
                            (a.hidden, a.color, a.class) == (b.hidden, b.color, b.class)
                                && same_text(&a.text, &b.text)
                                && pools.text_of(a.tag) == other_pools.text_of(b.tag)
                                && name(self, a.layer) == name(other, b.layer)
                        })
                }
                (a, b) => a.is_some() && a == b,
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 線を並べた小さい DXF
    fn source(handles: &[String], seed: &str) -> Vec<u8> {
        let mut text = format!(
            "  0
SECTION
  2
HEADER
  9
$HANDSEED
  5
{seed}
  0
ENDSEC
  0
SECTION
  2
ENTITIES
"
        );
        for (at, handle) in handles.iter().enumerate() {
            text += &format!(
                "  0
LINE
  5
{handle}
  8
0
 10
{at}.0
 20
0.0
 11
{at}.5
 21
1.0
"
            );
        }
        text += "  0
ENDSEC
  0
EOF
";
        text.into_bytes()
    }

    fn named(handles: &[&str]) -> Vec<String> {
        handles.iter().map(|handle| (*handle).to_owned()).collect()
    }

    #[test]
    fn handles_far_apart_are_found() {
        let src = source(&named(&["2A", "FFFFF", "400"]), "100000");
        let document = Document::open(src, 1).expect("開く");
        assert!(matches!(document.handles, Handles::Sparse(_)));
        assert_eq!(document.row_of(0x2A), Some(0));
        assert_eq!(document.row_of(0xFFFFF), Some(1));
        assert_eq!(document.row_of(0x400), Some(2));
        assert_eq!(document.row_of(0x2B), None);
        assert_eq!(document.row_of(0), None);
        assert_eq!(document.locked, None);
    }

    #[test]
    fn a_repeated_handle_locks_the_document_and_points_at_the_first_row() {
        let src = source(&named(&["2A", "2A", "2B"]), "100000");
        let document = Document::open(src, 1).expect("開く");
        assert!(matches!(document.handles, Handles::Dense(_)));
        assert_eq!(document.locked, Some(Locked::DuplicateHandle));
        assert_eq!(document.row_of(0x2A), Some(0));

        let src = source(&named(&["2A", "FFFFF", "FFFFF"]), "100000");
        let document = Document::open(src, 1).expect("開く");
        assert!(matches!(document.handles, Handles::Sparse(_)));
        assert_eq!(document.locked, Some(Locked::DuplicateHandle));
        assert_eq!(document.row_of(0xFFFFF), Some(1));
    }

    #[test]
    fn a_seed_not_above_the_largest_handle_does_not_lock_the_document() {
        // 種は、変更のたびに、ハンドルの最大より大きく直す。開くときには見ない
        for seed in ["2C", "2D"] {
            let src = source(&named(&["2A", "2B", "2C"]), seed);
            let document = Document::open(src, 1).expect("開く");
            assert_eq!(document.locked, None);
            assert_eq!(document.largest_handle(), Some(0x2C));
        }
    }

    #[test]
    fn opening_does_not_depend_on_the_number_of_parts() {
        let handles: Vec<String> = (0x100..0x100 + 3000)
            .map(|handle| format!("{handle:X}"))
            .collect();
        let src = source(&handles, "FFFF");
        let whole = Document::open(src.clone(), 1).expect("開く");
        assert_eq!(whole.rows.len(), 3000);
        for parts in [2, 3, 7, 16] {
            let scanned = scan_split(&src, parts).expect("読む");
            assert!(scanned.len() > 1, "{parts} 個に分かれない");
            let mut split = Document::assemble(src.clone(), scanned).expect("まとめる");
            split.build_display();
            split.build_index();
            assert!(whole.same_content(&split), "{parts}");
            assert!(whole.display == split.display, "{parts}");
            assert_eq!(whole.boxes, split.boxes, "{parts}");
            assert!(whole.grid == split.grid, "{parts}");
            assert_eq!(whole.counts, split.counts, "{parts}");
        }
    }
}
