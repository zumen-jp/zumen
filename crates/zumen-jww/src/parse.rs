//! JWW のバイト列を、見出し・図形・ブロックの定義・画像のレコードに分ける（決定 0133）。
//!
//! 見出しの終わりは、知っている欄の大きさを足した位置と、図形の一覧の始まりの形（件数と、新しいクラスの印と
//! `CData` で始まるクラスの名前）を探した位置を候補にする。候補ごとに、図形とブロックの定義の一覧を終わりまで
//! 読み、読めた最初の候補を使う。画像の一覧も終わりまで読めた候補を先に選ぶ。

use core::ops::Range;

use zumen_base::{Diagnostic, Kind, Ref, codes};
use zumen_sdk::{Cx, Error, Stop};

use crate::{Class, Record, RecordKind, Tag, stopped};

/// 画像の一覧を持つ最初の版
pub(crate) const IMAGES_SINCE: u32 = 700;

/// 図形の共通の欄に線幅が入る最初の版
const PEN_WIDTH_SINCE: u32 = 351;

/// 寸法に SXF の欄が入る最初の版
const SXF_SINCE: u32 = 420;

/// 図形の一覧に入るクラス
const ENTITIES: [&[u8]; 7] = [
    b"CDataSen",
    b"CDataEnko",
    b"CDataTen",
    b"CDataMoji",
    b"CDataSolid",
    b"CDataBlock",
    b"CDataSunpou",
];

/// ブロックの定義のクラス
const BLOCK: &[u8] = b"CDataList";

/// 点を記号にする線種
const POINT_SYMBOL: u8 = 100;

/// ソリッドの色を自分で持つ色の番号
const SOLID_OWN_COLOR: u16 = 10;

/// 読んだ結果
pub(crate) struct Read {
    pub(crate) version: u32,
    pub(crate) classes: Vec<Class>,
    pub(crate) records: Vec<Record>,
}

/// 読めなかった理由。予算で止めたときは、止めた所を持つ
enum Fail {
    /// 形が読めない
    Invalid,
    Stop(Stop, Range<usize>),
}

/// バイト列を前から読む
struct Reader<'s> {
    src: &'s [u8],
    pos: usize,
}

impl<'s> Reader<'s> {
    fn new(src: &'s [u8], pos: usize) -> Self {
        Self { src, pos }
    }

    fn take(&mut self, n: usize) -> Result<&'s [u8], Fail> {
        let end = self.pos.checked_add(n).ok_or(Fail::Invalid)?;
        let bytes = self.src.get(self.pos..end).ok_or(Fail::Invalid)?;
        self.pos = end;
        Ok(bytes)
    }

    fn skip(&mut self, n: usize) -> Result<(), Fail> {
        self.take(n).map(|_| ())
    }

    fn u8(&mut self) -> Result<u8, Fail> {
        self.take(1)?.first().copied().ok_or(Fail::Invalid)
    }

    fn u16(&mut self) -> Result<u16, Fail> {
        let bytes: [u8; 2] = self.take(2)?.try_into().map_err(|_| Fail::Invalid)?;
        Ok(u16::from_le_bytes(bytes))
    }

    fn u32(&mut self) -> Result<u32, Fail> {
        let bytes: [u8; 4] = self.take(4)?.try_into().map_err(|_| Fail::Invalid)?;
        Ok(u32::from_le_bytes(bytes))
    }

    /// MFC の CString。長さは 1・2・4 バイトへ広げ、`FF FE FF` は UTF-16LE の印
    fn cstring(&mut self) -> Result<(), Fail> {
        let mut unicode = false;
        let mut n = u32::from(self.u8()?);
        if n == 0xFF {
            n = u32::from(self.u16()?);
            if n == 0xFFFE {
                unicode = true;
                n = u32::from(self.u8()?);
                if n == 0xFF {
                    n = u32::from(self.u16()?);
                }
            }
            if n == 0xFFFF {
                n = self.u32()?;
            }
        }
        let n = usize::try_from(n).map_err(|_| Fail::Invalid)?;
        self.skip(if unicode {
            n.checked_mul(2).ok_or(Fail::Invalid)?
        } else {
            n
        })
    }

    /// MFC の件数。`FFFF` なら 4 バイトが続く
    fn count(&mut self) -> Result<u32, Fail> {
        match self.u16()? {
            0xFFFF => self.u32(),
            n => Ok(u32::from(n)),
        }
    }
}

/// 版を読み、見出しの終わりの候補から図形の一覧を読む
pub(crate) fn read(src: &[u8], cx: &mut Cx) -> Result<Read, Error> {
    let mut head = Reader::new(src, 0);
    let version = (|| {
        if head.take(8)? != crate::MAGIC {
            return Err(Fail::Invalid);
        }
        head.u32()
    })();
    let Ok(version) = version else {
        cx.report(Diagnostic::new(codes::JWW_FILE_INVALID));
        return Err(Error::Invalid);
    };
    // 知っている欄の終わりを先に、探した位置を前から試す。同じ位置は 1 度だけ読む
    // 探した位置は、そこで見つけたクラスのオブジェクトを最初に読めた候補だけを使う
    let mut starts: Vec<(usize, Option<usize>)> = header_end(src, version)
        .into_iter()
        .map(|start| (start, None))
        .collect();
    for (start, first) in scan(src) {
        if !starts.iter().any(|&(seen, _)| seen == start) {
            starts.push((start, Some(first)));
        }
    }
    // 画像の一覧まで読める候補を先に、なければ図形とブロックの定義の一覧が読める候補を使う
    let mut loose = None;
    for &(start, first) in &starts {
        match body(src, version, start, cx) {
            Ok(read)
                if first.is_some_and(|first| {
                    read.records.first().map(|record| record.span.start) != Some(first)
                }) => {}
            Ok(read) if read.trailer_read => return Ok(finish(src, version, start, read)),
            Ok(read) => {
                loose.get_or_insert((start, read));
            }
            Err(Fail::Invalid) => {}
            Err(Fail::Stop(stop, place)) => return Err(stopped(cx, stop, Some(place))),
        }
    }
    match loose {
        Some((start, read)) => {
            // 後ろの読めないバイト列は、そのまま残す
            let end = read.records.last().map_or(start, |record| record.span.end);
            cx.report(
                Diagnostic::new(codes::JWW_TRAILER_INVALID)
                    .at(Ref::at_bytes(end as u64, src.len() as u64)),
            );
            Ok(finish(src, version, start, read))
        }
        None => {
            cx.report(Diagnostic::new(codes::JWW_BODY_INVALID));
            Err(Error::Invalid)
        }
    }
}

/// 候補の本体の読み
struct Body {
    classes: Vec<Class>,
    records: Vec<Record>,
    /// 画像の一覧を終わりまで読めたか。版 700 より前は、後に何もないこと
    trailer_read: bool,
}

/// 見出しのレコードを前に、残りのバイトがあれば後ろに足す
fn finish(src: &[u8], version: u32, start: usize, body: Body) -> Read {
    let mut records = Vec::with_capacity(body.records.len() + 2);
    records.push(Record {
        span: 0..start,
        kind: RecordKind::Header,
        tag: None,
        body: 0..start,
        block: None,
    });
    let end = body.records.last().map_or(start, |record| record.span.end);
    // 見出しを前に足すので、ブロックの定義の位置を 1 つ後ろにする
    records.extend(body.records.into_iter().map(|mut record| {
        record.block = record.block.map(|at| at + 1);
        record
    }));
    if end < src.len() {
        records.push(Record {
            span: end..src.len(),
            kind: RecordKind::Trailer,
            tag: None,
            body: end..src.len(),
            block: None,
        });
    }
    Read {
        version,
        classes: body.classes,
        records,
    }
}

/// 図形の一覧の始まりの形を探す。新しいクラスの印（`FF FF`）・スキーマ・名前の長さ（8〜32）・`CData` が続く所の、
/// 前の件数と、見つけた所。図形がなくブロックの定義だけがあるときのために、前の 2 つの件数の位置も候補にする
fn scan(src: &[u8]) -> Vec<(usize, usize)> {
    let mut starts = Vec::new();
    let mut at = 0;
    while let Some(found) = src
        .get(at..)
        .and_then(|rest| rest.windows(11).position(is_class_start))
    {
        let i = at + found;
        if let Some(count) = i.checked_sub(2) {
            starts.push((count, i));
        }
        // 件数が 4 バイトの形（FF FF と u32）
        if let Some(count) = i.checked_sub(6)
            && src.get(count..count + 2) == Some(&[0xFF, 0xFF])
        {
            starts.push((count, i));
        }
        if let Some(empty) = i.checked_sub(4)
            && src.get(empty..empty + 2) == Some(&[0, 0])
        {
            starts.push((empty, i));
        }
        at = i + 1;
    }
    starts
}

fn is_class_start(window: &[u8]) -> bool {
    let Some((tag, rest)) = window.split_first_chunk::<2>() else {
        return false;
    };
    let Some((_schema, rest)) = rest.split_first_chunk::<2>() else {
        return false;
    };
    let Some((len, name)) = rest.split_first_chunk::<2>() else {
        return false;
    };
    *tag == [0xFF, 0xFF] && (8..=32).contains(&u16::from_le_bytes(*len)) && name == b"CData"
}

/// 知っている欄の大きさを足した、見出しの終わり
fn header_end(src: &[u8], version: u32) -> Result<usize, Fail> {
    let mut r = Reader::new(src, 12);
    // メモ・用紙の大きさ・書き込むレイヤグループ・16 のレイヤグループ（148 バイトずつ）・全体の設定（156 バイト）
    r.cstring()?;
    r.skip(8 + 16 * 148 + 156)?;
    // レイヤとレイヤグループの名前
    for _ in 0..256 + 16 {
        r.cstring()?;
    }
    let since = |v: u32, n: usize| if version >= v { n } else { 0 };
    // 表示の設定。日影・天空図・3 次元・表示の範囲・マークジャンプ・文字の描画・複線の間隔・ペン・線種・点など
    let jumps = if version >= 300 { 8 * 28 } else { 4 * 24 };
    r.skip(
        28 + since(300, 16)
            + 4
            + 24
            + 24
            + jumps
            + since(300, 56)
            + 80
            + 8
            + 80
            + 160
            + 128
            + 100
            + 64
            + 44
            + since(223, 60)
            + since(225, 32)
            + since(230, 8),
    )?;
    if version >= SXF_SINCE {
        // SXF の色・印刷の色・線種・利用者の線種
        r.skip(257 * 8)?;
        for _ in 0..257 {
            r.cstring()?;
            r.skip(16)?;
        }
        r.skip(33 * 16)?;
        for _ in 0..33 {
            r.cstring()?;
            r.skip(84)?;
        }
    }
    // 文字の設定
    r.skip(380)?;
    Ok(r.pos)
}

/// 図形の一覧とブロックの定義の一覧を読む
fn body(src: &[u8], version: u32, start: usize, cx: &mut Cx) -> Result<Body, Fail> {
    let mut list = List {
        r: Reader::new(src, start),
        version,
        classes: Vec::new(),
        numbers: Vec::new(),
        records: Vec::new(),
    };
    let entities = list.r.count()?;
    for _ in 0..entities {
        list.object(false, None, cx)?;
    }
    let blocks = list.r.count()?;
    for _ in 0..blocks {
        let at = list.records.len();
        list.object(true, None, cx)?;
        // ブロックの定義のレコードは、中の一覧の件数まで。中の図形は、その後ろに読んだ順で並ぶ
        let inner = list.r.count()?;
        if let Some(record) = list.records.get_mut(at) {
            record.span.end = list.r.pos;
            record.body.end = list.r.pos;
        }
        for _ in 0..inner {
            list.object(false, Some(at), cx)?;
        }
    }
    let mut trailer_read = list.r.pos == src.len();
    if version >= IMAGES_SINCE
        && let Ok(images) = images(src, list.r.pos, cx)
    {
        trailer_read = true;
        list.records.extend(images);
    }
    Ok(Body {
        classes: list.classes,
        records: list.records,
        trailer_read,
    })
}

/// 一覧を読む状態。クラスとオブジェクトの番号は、ファイルの全体で 1 から数える
struct List<'s> {
    r: Reader<'s>,
    version: u32,
    classes: Vec<Class>,
    /// 番号からクラスへ。オブジェクトの番号は `None`
    numbers: Vec<Option<usize>>,
    records: Vec<Record>,
}

impl List<'_> {
    fn object(&mut self, block: bool, parent: Option<usize>, cx: &mut Cx) -> Result<(), Fail> {
        let start = self.r.pos;
        let word = self.r.u16()?;
        let (tag, class) = match word {
            0 => {
                return self.push(
                    Record {
                        span: start..self.r.pos,
                        kind: RecordKind::Null,
                        tag: None,
                        body: self.r.pos..self.r.pos,
                        block: parent,
                    },
                    cx,
                );
            }
            0xFFFF => {
                let schema = self.r.u16()?;
                let len = usize::from(self.r.u16()?);
                let name_start = self.r.pos;
                self.r.skip(len)?;
                let class = self.classes.len();
                self.classes.push(Class {
                    name: name_start..self.r.pos,
                    schema,
                });
                self.number(Some(class))?;
                (Tag::New, class)
            }
            0x7FFF => {
                let big = self.r.u32()?;
                if big & 0x8000_0000 == 0 {
                    return Err(Fail::Invalid);
                }
                (Tag::Big, self.class_of(big & 0x7FFF_FFFF)?)
            }
            word if word & 0x8000 != 0 => (Tag::Known, self.class_of(u32::from(word & 0x7FFF))?),
            // 読んだオブジェクトへの参照は、Jw_cad は書かない
            _ => return Err(Fail::Invalid),
        };
        self.number(None)?;
        let name = self
            .classes
            .get(class)
            .and_then(|class| self.r.src.get(class.name.clone()))
            .ok_or(Fail::Invalid)?;
        let body_start = self.r.pos;
        let kind = if block {
            if name != BLOCK {
                return Err(Fail::Invalid);
            }
            self.block_head()?;
            RecordKind::Block
        } else {
            self.entity(name)?;
            RecordKind::Entity
        };
        self.push(
            Record {
                span: start..self.r.pos,
                kind,
                tag: Some((tag, class)),
                body: body_start..self.r.pos,
                block: parent,
            },
            cx,
        )
    }

    fn push(&mut self, record: Record, cx: &mut Cx) -> Result<(), Fail> {
        cx.check(Kind::Records, 1)
            .map_err(|stop| Fail::Stop(stop, record.span.clone()))?;
        self.records.push(record);
        Ok(())
    }

    fn number(&mut self, class: Option<usize>) -> Result<(), Fail> {
        // 番号は 31 ビットまで
        if self.numbers.len() >= 0x7FFF_FFFF {
            return Err(Fail::Invalid);
        }
        self.numbers.push(class);
        Ok(())
    }

    fn class_of(&self, number: u32) -> Result<usize, Fail> {
        let at = usize::try_from(number.checked_sub(1).ok_or(Fail::Invalid)?)
            .map_err(|_| Fail::Invalid)?;
        self.numbers.get(at).copied().flatten().ok_or(Fail::Invalid)
    }

    /// 共通の欄。線種と色を返す（点とソリッドの大きさが値で変わる）
    fn base(&mut self) -> Result<(u8, u16), Fail> {
        self.r.skip(4)?;
        let pen_style = self.r.u8()?;
        let pen_color = self.r.u16()?;
        if self.version >= PEN_WIDTH_SINCE {
            self.r.skip(2)?;
        }
        self.r.skip(6)?;
        Ok((pen_style, pen_color))
    }

    fn entity(&mut self, name: &[u8]) -> Result<(), Fail> {
        if !ENTITIES.contains(&name) {
            return Err(Fail::Invalid);
        }
        let (pen_style, pen_color) = self.base()?;
        match name {
            b"CDataSen" => self.r.skip(32),
            b"CDataEnko" => self.r.skip(7 * 8 + 4),
            b"CDataTen" => self.point(pen_style),
            b"CDataMoji" => self.text(),
            b"CDataSolid" => self
                .r
                .skip(64 + if pen_color == SOLID_OWN_COLOR { 4 } else { 0 }),
            b"CDataBlock" => self.r.skip(5 * 8 + 4),
            _ => self.dimension(),
        }
    }

    fn point(&mut self, pen_style: u8) -> Result<(), Fail> {
        self.r
            .skip(16 + 4 + if pen_style == POINT_SYMBOL { 20 } else { 0 })
    }

    fn text(&mut self) -> Result<(), Fail> {
        self.r.skip(32 + 4 + 32)?;
        self.r.cstring()?;
        self.r.cstring()
    }

    /// 寸法。線と文字を印なしで持ち、版 420 からは SXF の形と、線 2 本と点 4 つが続く
    fn dimension(&mut self) -> Result<(), Fail> {
        self.base()?;
        self.r.skip(32)?;
        self.base()?;
        self.text()?;
        if self.version >= SXF_SINCE {
            self.r.skip(2)?;
            for _ in 0..2 {
                self.base()?;
                self.r.skip(32)?;
            }
            for _ in 0..4 {
                let (pen_style, _) = self.base()?;
                self.point(pen_style)?;
            }
        }
        Ok(())
    }

    /// ブロックの定義の、中の一覧の前まで。番号・使われているか・時刻・名前
    fn block_head(&mut self) -> Result<(), Fail> {
        self.base()?;
        self.r.skip(12)?;
        self.r.cstring()
    }
}

/// 画像の一覧。件数（4 バイト）と、名前・大きさ・中身。終わりまで読めなければ読まない
fn images(src: &[u8], start: usize, cx: &mut Cx) -> Result<Vec<Record>, Fail> {
    let mut r = Reader::new(src, start);
    let count = r.u32()?;
    let mut records = vec![Record {
        span: start..r.pos,
        kind: RecordKind::ImageCount,
        tag: None,
        body: start..r.pos,
        block: None,
    }];
    for _ in 0..count {
        let at = r.pos;
        r.cstring()?;
        let size = usize::try_from(r.u32()?).map_err(|_| Fail::Invalid)?;
        let data = r.pos;
        r.skip(size)?;
        let record = Record {
            span: at..r.pos,
            kind: RecordKind::Image,
            tag: None,
            body: data..r.pos,
            block: None,
        };
        cx.check(Kind::Records, 1)
            .map_err(|stop| Fail::Stop(stop, record.span.clone()))?;
        records.push(record);
    }
    if r.pos != src.len() {
        return Err(Fail::Invalid);
    }
    Ok(records)
}
