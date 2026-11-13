//! 常駐する文書。元のバイト列を並列に読んで列に置き、変更・取り消し・保存・範囲の選択に答える。
//!
//! 元のバイト列は変えずに持ち、変えたレコードだけを、タグの並びとして別に持つ（`overlay`）。
//! 列（行・形・表示・索引）は、いつもタグの並びから読み直した値と同じになるように保つ。

use std::collections::BTreeMap;
use std::ops::Range;
use std::thread;

use zumen_acad::Value;

use crate::display::Display;
use crate::grid::Grid;
use crate::lift::{self, Geom, Kind, Lift, POLY_CLOSED, Place, Pools, RawTag, TextRow, class};
use crate::scan::{self, Next, Span};
use crate::value::{Val, typed, value};

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

/// 文字の値の符号化
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Charset {
    /// UTF-8（`$ACADVER` が AC1021 から）
    Utf8,
    /// Shift_JIS（`$DWGCODEPAGE` が ANSI_932）
    ShiftJis,
    /// ほか。ASCII だけを読む
    Other,
}

/// セクション
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Section {
    Header,
    Tables,
    Entities,
    Other,
}

/// 読むときに覚えておくレコード
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Mark {
    Section(Section),
    EndSection,
    Layer,
    /// 5 種類の図形のうち、ペーパー空間のもの
    Paper,
    /// 5 種類の図形のうち、要素にならなかったもの
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
    /// 折れ線の頂点と文字の中身
    pub pools: Pools,
}

impl Shapes {
    /// 形を足して、表の中の番号を返す
    fn push(&mut self, geom: Geom) -> u32 {
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
    /// オブジェクトのハンドル（0 を除く）
    handles: Vec<u64>,
    end: End,
}

/// レコードのハンドル（5 の、16 進として読める最初の値。DIMSTYLE は 105）
fn handle_of(name: &[u8], tags: &[RawTag<'_>]) -> Option<u64> {
    let expected = if name == b"DIMSTYLE" { 105 } else { 5 };
    tags.iter()
        .filter(|tag| tag.code == expected)
        .find_map(|tag| match typed(tag.code, tag.value) {
            Val::Handle(handle) => Some(handle),
            _ => None,
        })
}

/// 区間 `range` のレコードを読む。区間の始まりは、レコードの先頭であること
fn scan_part(src: &[u8], range: Range<usize>) -> Part {
    let mut part = Part {
        starts: Vec::with_capacity(range.len() / 96),
        marks: Vec::new(),
        rows: Vec::with_capacity(range.len() / 128),
        shapes: Shapes::default(),
        layers: Vec::new(),
        handles: Vec::with_capacity(range.len() / 96),
        end: End::Through,
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
        loop {
            match scan::next_tag(src, pos) {
                Next::Tag(tag) if tag.code == 0 => break,
                Next::Tag(tag) => {
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
                    b"ENTITIES" => Section::Entities,
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
                        });
                        lifted.handle
                    }
                    Lift::Paper => {
                        part.marks.push((record, Mark::Paper));
                        handle_of(name, &tags)
                    }
                    Lift::Not => {
                        part.marks.push((record, Mark::Dropped));
                        handle_of(name, &tags)
                    }
                },
                None => {
                    if name == b"LAYER" {
                        part.marks.push((record, Mark::Layer));
                    }
                    handle_of(name, &tags)
                }
            },
        };
        if let Some(handle) = handle.filter(|handle| *handle != 0) {
            part.handles.push(handle);
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
            .map(|worker| {
                worker.join().unwrap_or_else(|_| Part {
                    starts: Vec::new(),
                    marks: Vec::new(),
                    rows: Vec::new(),
                    shapes: Shapes::default(),
                    layers: Vec::new(),
                    handles: Vec::new(),
                    end: End::Invalid,
                })
            })
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
    /// `$HANDSEED` の、5 のハンドル
    pub seed: Option<u64>,
}

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
    /// `$HANDSEED` がハンドルの最大より大きくない（今の実装は、変更のたびに `$HANDSEED` を直す。この試作は直さない）
    HandleSeed,
}

/// 読んだものの数
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Counts {
    /// レコード
    pub records: u32,
    /// ENTITIES のセクションのレコード
    pub entities: u32,
    /// 要素（モデル空間の 5 種類）
    pub elements: u32,
    /// 5 種類のうち、ペーパー空間のもの
    pub paper: u32,
    /// 5 種類のうち、要素にならなかったもの（今の実装でも要素にならない）
    pub dropped: u32,
    /// 5 種類のうち、ASCII でない文字を Shift_JIS で持つもの（この試作は読まない）
    pub shift_jis: u32,
    /// 5 種類のうち、文字を読めないもの（今の実装でも要素にならない）
    pub unreadable: u32,
    /// 要素のうち、ハンドルがないか 0 で、変更で指せないもの
    pub unaddressable: u32,
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
    /// オブジェクトのハンドル。索引を作るまで持つ
    object_handles: Vec<u64>,
    /// 変えたレコード
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

/// 要素を囲む箱。円弧と膨らみのある辺は、広めに取る。文字は、置く点だけ
pub fn bounds(pools: &Pools, geom: &Geom) -> [f64; 4] {
    match geom {
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
        for part in &parts {
            let base = starts.len() as u32;
            marks.extend(
                part.marks
                    .iter()
                    .map(|&(record, mark)| (base + record, mark)),
            );
            starts.extend_from_slice(&part.starts);
        }
        starts.push(trailer);

        // セクション。SECTION で始まり、ENDSEC で終わる
        let mut entities: Vec<Range<u32>> = Vec::new();
        let mut tables: Vec<Range<u32>> = Vec::new();
        let mut header = None;
        let mut open: Option<(Section, u32)> = None;
        let mut close = |open: &mut Option<(Section, u32)>, end: u32| match open.take() {
            Some((Section::Entities, start)) => entities.push(start..end),
            Some((Section::Tables, start)) => tables.push(start..end),
            _ => {}
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
                Mark::EndSection => close(&mut open, record),
                Mark::Layer | Mark::Paper | Mark::Dropped => {}
            }
        }
        close(&mut open, records as u32);
        let inside = |ranges: &[Range<u32>], record: u32| {
            let at = ranges.partition_point(|range| range.end <= record);
            ranges.get(at).is_some_and(|range| range.start <= record)
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
            overlay: BTreeMap::new(),
            undo: Vec::new(),
            redo: Vec::new(),
            display: Display::default(),
            boxes: Vec::new(),
            handles: Handles::None,
            grid: Grid::default(),
        };
        document.read_header(header);
        // 画層の表。表の順に番号を振る
        for &(record, mark) in &marks {
            if mark == Mark::Layer && inside(&tables, record) {
                document.read_layer(record);
            }
        }
        for &(record, mark) in &marks {
            if inside(&entities, record) {
                match mark {
                    Mark::Paper => document.counts.paper += 1,
                    Mark::Dropped => document.counts.dropped += 1,
                    _ => {}
                }
            }
        }

        // 行と形。ENTITIES のセクションにあって、文字を読めるものだけを要素にする
        let total: usize = parts.iter().map(|part| part.rows.len()).sum();
        document.rows.reserve(total);
        let mut record_base = 0u32;
        for part in parts {
            let shapes = &mut document.shapes;
            let bases = [
                shapes.lines.len() as u32,
                shapes.circles.len() as u32,
                shapes.arcs.len() as u32,
                shapes.polys.len() as u32,
                shapes.texts.len() as u32,
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
            let charset = document.charset;
            let counts = &mut document.counts;
            document.rows.extend(part.rows.iter().filter_map(|row| {
                let record = record_base + row.record;
                if !inside(&entities, record) {
                    return None;
                }
                match (row.class, charset) {
                    (class::ASCII, _) | (class::UTF8, Charset::Utf8) => {}
                    (_, Charset::ShiftJis) => {
                        counts.shift_jis += 1;
                        return None;
                    }
                    _ => {
                        counts.unreadable += 1;
                        return None;
                    }
                }
                if row.handle == 0 {
                    counts.unaddressable += 1;
                }
                Some(Row {
                    record,
                    geom: row.geom + bases.get(row.kind as usize).copied().unwrap_or(0),
                    layer: layers.get(row.layer as usize).copied().unwrap_or(0),
                    ..*row
                })
            }));
            record_base += part.starts.len() as u32;
            document.object_handles.extend_from_slice(&part.handles);
        }
        document.counts.elements = document.rows.len() as u32;
        Ok(document)
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
        let mut seen: Vec<&[u8]> = Vec::new();
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
            let wanted: [&[u8]; 5] = [
                b"$ACADVER",
                b"$DWGCODEPAGE",
                b"$EXTMIN",
                b"$EXTMAX",
                b"$HANDSEED",
            ];
            if !wanted.contains(&name) || seen.contains(&name) {
                continue;
            }
            seen.push(name);
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
                b"$HANDSEED" => {
                    header.seed = values.iter().find_map(|value| match value.data {
                        zumen_acad::Data::Handle(seed) if value.code == 5 => Some(seed),
                        _ => None,
                    });
                }
                _ => {
                    let variable = Some(Variable {
                        start: first + 1,
                        values,
                    });
                    if name == b"$EXTMIN" {
                        header.extmin = variable;
                    } else {
                        header.extmax = variable;
                    }
                }
            }
        }
        self.charset = if version.is_some_and(|version| version.as_slice() >= b"AC1021".as_slice())
        {
            Charset::Utf8
        } else if code_page.is_some_and(|page| page.eq_ignore_ascii_case(b"ANSI_932")) {
            Charset::ShiftJis
        } else {
            Charset::Other
        };
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

    /// 表示のバッファーを作る
    pub fn build_display(&mut self) {
        let mut display = Display::with_rows(self.rows.len());
        for row in &self.rows {
            if let Some(geom) = self.shapes.geom(row.kind, row.geom) {
                display.push(&geom, &self.shapes.pools, [row.layer, row.color]);
            }
        }
        self.display = display;
    }

    /// ハンドルの表と、範囲の索引を作る。変更を受け付けるかも、ここで決める
    pub fn build_index(&mut self) {
        // 同じハンドルを持つオブジェクトがあれば、変更を受け付けない
        let handles = core::mem::take(&mut self.object_handles);
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
        let seed = self.header.as_ref().and_then(|header| header.seed);
        self.locked = if duplicate {
            Some(Locked::DuplicateHandle)
        } else if !handles.is_empty() && seed.is_some_and(|seed| largest >= seed) {
            Some(Locked::HandleSeed)
        } else {
            None
        };

        let largest = self.rows.iter().map(|row| row.handle).max().unwrap_or(0);
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
                Some(geom) => bounds(&self.shapes.pools, &geom),
                None => [f64::NAN; 4],
            })
            .collect();
        self.grid = Grid::build(&boxes);
        self.boxes = boxes;
    }

    /// 箱が `rect`（最小の x・y、最大の x・y）と重なる要素の行を、ファイルの順に返す
    pub fn select(&self, rect: [f64; 4], out: &mut Vec<u32>) {
        self.grid.query(&self.boxes, rect, out);
    }

    /// ハンドルから行を引く
    pub fn row_of(&self, handle: u64) -> Option<u32> {
        self.handles.get(handle)
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
            + self.rows.len() * core::mem::size_of::<Row>()
            + shapes.lines.len() * 32
            + shapes.circles.len() * 24
            + shapes.arcs.len() * 40
            + shapes.polys.len() * core::mem::size_of::<Poly>()
            + shapes.texts.len() * core::mem::size_of::<TextRow>()
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
        if self.rows.len() != other.rows.len() || self.charset != other.charset {
            return false;
        }
        let name = |document: &Document, layer: u32| {
            document
                .layers
                .list
                .get(layer as usize)
                .map(|layer| layer.name.to_ascii_lowercase())
        };
        self.rows.iter().zip(&other.rows).all(|(a, b)| {
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
    fn a_seed_not_above_the_largest_handle_locks_the_document() {
        let src = source(&named(&["2A", "2B", "2C"]), "2C");
        let document = Document::open(src, 1).expect("開く");
        assert_eq!(document.locked, Some(Locked::HandleSeed));
        let src = source(&named(&["2A", "2B", "2C"]), "2D");
        let document = Document::open(src, 1).expect("開く");
        assert_eq!(document.locked, None);
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
