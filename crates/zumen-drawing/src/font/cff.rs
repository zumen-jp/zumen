//! CFF・CFF2 の表の字形の命令（Type 2 charstring）をたどり、字形の外形を求める（決定 0186）。
//!
//! 仕様は Adobe の Technical Note #5176（CFF）・#5177（Type 2 Charstring）と、OpenType の仕様の
//! CFF2 の節に従う。外形は、曲線の制御点ではなく、曲線の極値まで含めた正確な外形にする。
//!
//! 受け取ったファイルは信用しない。副ルーチンの入れ子は 10 段（#5177 の付録 B）、引数のスタックは
//! CFF で 48、CFF2 で 513 に抑え、1 つの字形で読む数と演算子の数にも上限（[`WORK`]）を設ける。
//! 位置の計算は `u64` で行い、`usize` の幅で結果が変わらないようにする。

use super::{Unread, i16_at, slice, u16_at, u32_at};

/// 副ルーチンの入れ子の上限（#5177 の付録 B）
const DEPTH: usize = 10;
/// 1 つの字形で読む数と演算子の数の上限。上限を超えた字形は読めないものとする
const WORK: u32 = 1 << 16;
/// 引数のスタックの上限（#5177 の付録 B。CFF2 は OpenType の仕様の CFF2 の節）
const STACK: usize = 48;
const STACK2: usize = 513;
/// Font DICT（FDArray）の数の上限。CFF の FDSelect は 1 バイトで Font DICT を指す
const FONTS: u32 = 256;
/// Private DICT の大きさの上限。Font DICT が同じ Private DICT を指しても、読む量を抑える
const PRIVATE: u64 = 1 << 16;

/// 読んだ CFF・CFF2 の表
pub(super) struct Cff<'a> {
    charstrings: Index<'a>,
    globals: Index<'a>,
    /// Font DICT ごとの局所の副ルーチンと vsindex の既定値。CID でない CFF は 1 つ
    fonts: Vec<Local<'a>>,
    select: Select<'a>,
    /// CFF2 の ItemVariationStore。blend の領域の数を引く
    store: &'a [u8],
    stack: usize,
}

/// INDEX（#5176 の 5 節。CFF2 は数が 32 ビット）
#[derive(Clone, Copy)]
struct Index<'a> {
    count: u32,
    size: u8,
    offsets: &'a [u8],
    data: &'a [u8],
}

#[derive(Clone, Copy)]
struct Local<'a> {
    subrs: Index<'a>,
    vsindex: u16,
}

/// 字形から Font DICT を引く FDSelect（#5176 の 19 節、CFF2 の形式 4）
#[derive(Clone, Copy)]
enum Select<'a> {
    /// Font DICT が 1 つ
    One,
    /// 形式 0: 字形ごとに 1 バイト
    Bytes(&'a [u8]),
    /// 形式 3・4: 区間の並び（番兵を含む）と区間の数。形式 4 は字形が 32 ビット、Font DICT が 16 ビット
    Ranges(&'a [u8], u32, bool),
}

/// Top DICT・Font DICT から読む値
struct Top {
    charstrings: Option<i64>,
    private: Option<(i64, i64)>,
    charstring_type: i64,
    cid: bool,
    fdarray: Option<i64>,
    fdselect: Option<i64>,
    vstore: Option<i64>,
}

impl<'a> Index<'a> {
    const EMPTY: Self = Self {
        count: 0,
        size: 1,
        offsets: &[],
        data: &[],
    };

    /// `at` から INDEX を読み、INDEX と、その次の位置を返す
    fn read(table: &'a [u8], at: u64, long: bool) -> Option<(Self, u64)> {
        let head = if long { 4 } else { 2 };
        let bytes = slice(table, at, head)?;
        let count = if long {
            u32_at(bytes, 0)?
        } else {
            u32::from(u16_at(bytes, 0)?)
        };
        if count == 0 {
            return Some((Self::EMPTY, at + head));
        }
        let size = *slice(table, at + head, 1)?.first()?;
        if !(1..=4).contains(&size) {
            return None;
        }
        let length = (u64::from(count) + 1) * u64::from(size);
        let offsets = slice(table, at + head + 1, length)?;
        let mut index = Self {
            count,
            size,
            offsets,
            data: &[],
        };
        // 位置は、データの直前のバイトから 1 で数える
        let last = index.offset(count)?.checked_sub(1)?;
        let start = at + head + 1 + length;
        index.data = slice(table, start, last)?;
        Some((index, start + last))
    }

    fn offset(&self, i: u32) -> Option<u64> {
        let size = u64::from(self.size);
        let bytes = slice(self.offsets, u64::from(i) * size, size)?;
        Some(
            bytes
                .iter()
                .fold(0, |value, &byte| value << 8 | u64::from(byte)),
        )
    }

    fn get(&self, i: u32) -> Option<&'a [u8]> {
        if i >= self.count {
            return None;
        }
        let start = self.offset(i)?.checked_sub(1)?;
        let end = self.offset(i + 1)?.checked_sub(1)?;
        slice(self.data, start, end.checked_sub(start)?)
    }
}

/// DICT（#5176 の 4 節）を読み、演算子ごとに引数を渡す。2 バイトの演算子は `0x0C00` に 2 バイト目を
/// 足す。実数は位置に使わないので 0 として読む
fn dict(data: &[u8], limit: usize, mut visit: impl FnMut(u16, &[i64])) -> Option<()> {
    let mut stack: Vec<i64> = Vec::new();
    let mut at = 0;
    while let Some(&b0) = data.get(at) {
        at += 1;
        let value = match b0 {
            // CFF2 は 22（vsindex）・23（blend）・24（vstore）を演算子に足した
            0..=27 => {
                let op = if b0 == 12 {
                    let b1 = *data.get(at)?;
                    at += 1;
                    0x0C00 | u16::from(b1)
                } else {
                    u16::from(b0)
                };
                visit(op, &stack);
                stack.clear();
                continue;
            }
            28 => {
                let value = i16_at(data, at)?;
                at += 2;
                i64::from(value)
            }
            29 => {
                let value = u32_at(data, at)? as i32;
                at += 4;
                i64::from(value)
            }
            30 => {
                // 実数は 4 ビットずつ並び、0xF で終わる
                loop {
                    let byte = *data.get(at)?;
                    at += 1;
                    if byte >> 4 == 0xF || byte & 0xF == 0xF {
                        break;
                    }
                }
                0
            }
            32..=246 => i64::from(b0) - 139,
            247..=254 => {
                let b1 = i64::from(*data.get(at)?);
                at += 1;
                if b0 <= 250 {
                    (i64::from(b0) - 247) * 256 + b1 + 108
                } else {
                    -(i64::from(b0) - 251) * 256 - b1 - 108
                }
            }
            _ => return None,
        };
        if stack.len() >= limit {
            return None;
        }
        stack.push(value);
    }
    Some(())
}

fn top(data: &[u8], limit: usize) -> Option<Top> {
    let mut top = Top {
        charstrings: None,
        private: None,
        charstring_type: 2,
        cid: false,
        fdarray: None,
        fdselect: None,
        vstore: None,
    };
    dict(data, limit, |op, args| {
        let last = args.last().copied();
        match op {
            17 => top.charstrings = last,
            18 => {
                if let [.., size, offset] = *args {
                    top.private = Some((size, offset));
                }
            }
            24 => top.vstore = last,
            0x0C06 => top.charstring_type = last.unwrap_or(2),
            0x0C1E => top.cid = true,
            0x0C24 => top.fdarray = last,
            0x0C25 => top.fdselect = last,
            _ => {}
        }
    })?;
    Some(top)
}

/// Private DICT から局所の副ルーチン（Subrs、Private DICT の頭からの位置）と vsindex を読む。
/// Private DICT がなければ、副ルーチンを持たない
fn private<'a>(
    table: &'a [u8],
    at: Option<(i64, i64)>,
    long: bool,
    limit: usize,
) -> Option<Local<'a>> {
    let Some((size, offset)) = at else {
        return Some(Local {
            subrs: Index::EMPTY,
            vsindex: 0,
        });
    };
    let (size, offset) = (u64::try_from(size).ok()?, u64::try_from(offset).ok()?);
    if size > PRIVATE {
        return None;
    }
    let data = slice(table, offset, size)?;
    let (mut subrs, mut vsindex) = (None, 0);
    dict(data, limit, |op, args| match op {
        19 => subrs = args.last().copied(),
        22 => vsindex = args.last().copied().unwrap_or(0),
        _ => {}
    })?;
    let subrs = match subrs {
        Some(at) => Index::read(table, offset.checked_add(u64::try_from(at).ok()?)?, long)?.0,
        None => Index::EMPTY,
    };
    Some(Local {
        subrs,
        vsindex: u16::try_from(vsindex).ok()?,
    })
}

fn position(value: Option<i64>) -> Option<u64> {
    u64::try_from(value?).ok()
}

impl<'a> Cff<'a> {
    /// `CFF ` か（`version2` が真なら）`CFF2` の表を読む。Type 2 でない字形の命令と、Font DICT の
    /// 多すぎるものは [`Unread::Cff`] で断る
    pub(super) fn parse(table: &'a [u8], version2: bool) -> Result<Self, Unread> {
        let broken = Unread::Table(if version2 { "CFF2" } else { "CFF " });
        let limit = if version2 { STACK2 } else { STACK };
        // 見出し: major（0）・minor（1）・hdrSize（2）。CFF2 は topDictLength（3）が続く
        let major = table.first().copied();
        let header = u64::from(*table.get(2).ok_or(broken)?);
        let (top, globals) = if version2 {
            if major != Some(2) {
                return Err(broken);
            }
            let length = u64::from(u16_at(table, 3).ok_or(broken)?);
            let top = slice(table, header, length).ok_or(broken)?;
            let (globals, _) = Index::read(table, header + length, true).ok_or(broken)?;
            (top, globals)
        } else {
            if major != Some(1) {
                return Err(broken);
            }
            // Name INDEX・Top DICT INDEX・String INDEX・Global Subr INDEX の順に並ぶ
            let (_, at) = Index::read(table, header, false).ok_or(broken)?;
            let (tops, at) = Index::read(table, at, false).ok_or(broken)?;
            let (_, at) = Index::read(table, at, false).ok_or(broken)?;
            let (globals, _) = Index::read(table, at, false).ok_or(broken)?;
            (tops.get(0).ok_or(broken)?, globals)
        };
        let top = self::top(top, limit).ok_or(broken)?;
        if top.charstring_type != 2 {
            return Err(Unread::Cff);
        }
        let at = position(top.charstrings).ok_or(broken)?;
        let (charstrings, _) = Index::read(table, at, version2).ok_or(broken)?;

        let mut fonts = Vec::new();
        let select = if version2 || top.cid {
            let at = position(top.fdarray).ok_or(broken)?;
            let (array, _) = Index::read(table, at, version2).ok_or(broken)?;
            if array.count > FONTS {
                return Err(Unread::Cff);
            }
            for i in 0..array.count {
                let font = self::top(array.get(i).ok_or(broken)?, limit).ok_or(broken)?;
                fonts.push(private(table, font.private, version2, limit).ok_or(broken)?);
            }
            match top.fdselect {
                None if version2 && array.count == 1 => Select::One,
                at => select(table, position(at).ok_or(broken)?).ok_or(broken)?,
            }
        } else {
            fonts.push(private(table, top.private, false, limit).ok_or(broken)?);
            Select::One
        };
        // CFF2 の vstore: 長さ（u16）に続く ItemVariationStore
        let store = match position(top.vstore) {
            Some(at) if version2 => {
                let length =
                    u16_at(table, usize::try_from(at).map_err(|_| broken)?).ok_or(broken)?;
                slice(table, at + 2, u64::from(length)).ok_or(broken)?
            }
            _ => &[],
        };
        Ok(Self {
            charstrings,
            globals,
            fonts,
            select,
            store,
            stack: limit,
        })
    }

    /// 字形の外形。字形がないか、命令が壊れているか、上限を超えれば `None`。描かない字形は `Some(None)`
    pub(super) fn bounds(&self, id: u16) -> Option<Option<[f64; 4]>> {
        let code = self.charstrings.get(u32::from(id))?;
        let local = *self.fonts.get(self.select.font(id)?)?;
        Machine {
            cff: self,
            local,
            stack: Vec::new(),
            stems: 0,
            pen: Pen::default(),
        }
        .run(code)
    }

    /// vsindex の ItemVariationData の領域の数（OpenType の仕様の Item variation store の節）
    fn regions(&self, vsindex: u16) -> Option<u16> {
        // ItemVariationStore: format（0）・itemVariationDataCount（6）・itemVariationDataOffsets（8）
        if u16_at(self.store, 0)? != 1 || vsindex >= u16_at(self.store, 6)? {
            return None;
        }
        let at = u32_at(self.store, 8 + usize::from(vsindex) * 4)?;
        let data = self.store.get(usize::try_from(at).ok()?..)?;
        // ItemVariationData: itemCount（0）・wordDeltaCount（2）・regionIndexCount（4）
        u16_at(data, 4)
    }
}

fn select(table: &[u8], at: u64) -> Option<Select<'_>> {
    let rest = table.get(usize::try_from(at).ok()?..)?;
    match rest.first()? {
        0 => Some(Select::Bytes(rest.get(1..)?)),
        3 => {
            let count = u16_at(rest, 1)?;
            let ranges = slice(rest, 3, u64::from(count) * 3 + 2)?;
            Some(Select::Ranges(ranges, u32::from(count), false))
        }
        4 => {
            let count = u32_at(rest, 1)?;
            let ranges = slice(rest, 5, u64::from(count) * 6 + 4)?;
            Some(Select::Ranges(ranges, count, true))
        }
        _ => None,
    }
}

impl Select<'_> {
    fn font(&self, id: u16) -> Option<usize> {
        match *self {
            Select::One => Some(0),
            Select::Bytes(bytes) => bytes.get(usize::from(id)).map(|&fd| usize::from(fd)),
            Select::Ranges(ranges, count, wide) => {
                // 区間: first・fd。first の昇順と仮定し、first が id 以下の最後の区間を探す
                let stride: u64 = if wide { 6 } else { 3 };
                let first = |i: u32| -> Option<u32> {
                    let at = usize::try_from(u64::from(i) * stride).ok()?;
                    if wide {
                        u32_at(ranges, at)
                    } else {
                        u16_at(ranges, at).map(u32::from)
                    }
                };
                let id = u32::from(id);
                let (mut low, mut high) = (0, count);
                while low < high {
                    let middle = low + (high - low) / 2;
                    if first(middle)? <= id {
                        low = middle + 1;
                    } else {
                        high = middle;
                    }
                }
                // 番兵（字形の数）を超える字形は、どの区間にも入らない
                if low == 0 || id >= first(count)? {
                    return None;
                }
                let at =
                    usize::try_from(u64::from(low - 1) * stride + if wide { 4 } else { 2 }).ok()?;
                if wide {
                    u16_at(ranges, at).map(usize::from)
                } else {
                    ranges.get(at).map(|&fd| usize::from(fd))
                }
            }
        }
    }
}

/// 字形の命令を解く機械
struct Machine<'c, 'a> {
    cff: &'c Cff<'a>,
    local: Local<'a>,
    stack: Vec<f64>,
    /// 宣言されたステム・ヒントの数。hintmask の長さを決める
    stems: u32,
    pen: Pen,
}

impl<'a> Machine<'_, 'a> {
    fn run(mut self, code: &'a [u8]) -> Option<Option<[f64; 4]>> {
        // 副ルーチンを呼んだ所。再帰でなく配列に積み、深さを DEPTH で抑える
        let mut frames: Vec<(&'a [u8], usize)> = Vec::new();
        let (mut code, mut at) = (code, 0usize);
        let mut work = 0u32;
        let mut vsindex = self.local.vsindex;
        loop {
            let Some(&b0) = code.get(at) else {
                // 命令の終わり。CFF2 は return・endchar を持たず、終わりで戻る
                match frames.pop() {
                    Some((caller, resume)) => {
                        (code, at) = (caller, resume);
                        continue;
                    }
                    None => break,
                }
            };
            work += 1;
            if work > WORK {
                return None;
            }
            at += 1;
            let value = match b0 {
                28 => {
                    let value = i16_at(code, at)?;
                    at += 2;
                    f64::from(value)
                }
                32..=246 => f64::from(i16::from(b0) - 139),
                247..=254 => {
                    let b1 = i32::from(*code.get(at)?);
                    at += 1;
                    let value = if b0 <= 250 {
                        (i32::from(b0) - 247) * 256 + b1 + 108
                    } else {
                        -(i32::from(b0) - 251) * 256 - b1 - 108
                    };
                    f64::from(value)
                }
                // 16.16 の固定小数点
                255 => {
                    let value = u32_at(code, at)? as i32;
                    at += 4;
                    f64::from(value) / 65536.0
                }
                _ => {
                    let op = if b0 == 12 {
                        let b1 = *code.get(at)?;
                        at += 1;
                        0x0C00 | u16::from(b1)
                    } else {
                        u16::from(b0)
                    };
                    match op {
                        // hstem・vstem・hstemhm・vstemhm
                        1 | 3 | 18 | 23 => self.count_stems(),
                        // hintmask・cntrmask: 前の引数は vstem で、ステム 8 つごとに 1 バイトの印が続く
                        19 | 20 => {
                            self.count_stems();
                            let bytes = usize::try_from(self.stems.div_ceil(8)).ok()?;
                            at = at.checked_add(bytes).filter(|&end| end <= code.len())?;
                        }
                        // callsubr・callgsubr: スタックを消さない
                        10 | 29 => {
                            let subrs = if op == 10 {
                                self.local.subrs
                            } else {
                                self.cff.globals
                            };
                            let index = biased(self.stack.pop()?, subrs.count)?;
                            let target = subrs.get(index)?;
                            if frames.len() >= DEPTH {
                                return None;
                            }
                            frames.push((code, at));
                            (code, at) = (target, 0);
                            continue;
                        }
                        // return
                        11 => {
                            (code, at) = frames.pop()?;
                            continue;
                        }
                        // endchar。引数が 4 つ以上なら、廃止された seac（標準の符号化で字形を重ねる）で、読まない
                        14 => {
                            if self.stack.len() >= 4 {
                                return None;
                            }
                            break;
                        }
                        // vsindex（CFF2）
                        15 => {
                            vsindex = u16::try_from(integer(self.stack.pop()?)?).ok()?;
                        }
                        // blend（CFF2）: 既定の値だけを残し、差分を捨てる。スタックを消さない
                        16 => {
                            self.blend(vsindex)?;
                            continue;
                        }
                        _ => self.pen.draw(op, &self.stack)?,
                    }
                    self.stack.clear();
                    continue;
                }
            };
            if self.stack.len() >= self.cff.stack {
                return None;
            }
            self.stack.push(value);
        }
        Some(self.pen.bounds)
    }

    fn count_stems(&mut self) {
        let pairs = u32::try_from(self.stack.len() / 2).unwrap_or(u32::MAX);
        self.stems = self.stems.saturating_add(pairs);
    }

    /// blend: 引数は n 個の既定の値・n × 領域の数の差分・n。可変フォントの既定の形だけを読む
    fn blend(&mut self, vsindex: u16) -> Option<()> {
        let count = u64::try_from(integer(self.stack.pop()?)?).ok()?;
        let regions = u64::from(self.cff.regions(vsindex)?);
        let total = count.checked_mul(regions + 1)?;
        let length = self.stack.len() as u64;
        let base = length.checked_sub(total)?;
        self.stack.truncate(usize::try_from(base + count).ok()?);
        Some(())
    }
}

/// 副ルーチンの番号。引数に、副ルーチンの数で決まる偏りを足す（#5177 の 4.7 節）
fn biased(value: f64, count: u32) -> Option<u32> {
    let bias = if count < 1240 {
        107
    } else if count < 33900 {
        1131
    } else {
        32768
    };
    u32::try_from(integer(value)? + bias).ok()
}

/// 整数の引数。整数でなければ `None`
fn integer(value: f64) -> Option<i64> {
    (value.fract() == 0.0 && value.abs() <= 2_147_483_648.0).then_some(value as i64)
}

/// 現在の点と、描いた外形
#[derive(Default)]
struct Pen {
    point: [f64; 2],
    bounds: Option<[f64; 4]>,
}

impl Pen {
    /// 描く演算子（#5177 の 4.1 節と 4.4 節の flex）。引数の数が合わなければ `None`。
    /// 最初の演算子の前に置かれる送りの引数は、移動では後ろから数えて読み飛ばす
    fn draw(&mut self, op: u16, args: &[f64]) -> Option<()> {
        let n = args.len();
        match op {
            // rmoveto・hmoveto・vmoveto
            21 => {
                let [.., dx, dy] = *args else { return None };
                self.move_by(dx, dy);
            }
            22 => self.move_by(*args.last()?, 0.0),
            4 => self.move_by(0.0, *args.last()?),
            // rlineto
            5 => {
                if n == 0 || !n.is_multiple_of(2) {
                    return None;
                }
                for &[dx, dy] in args.as_chunks::<2>().0 {
                    self.line(dx, dy);
                }
            }
            // hlineto・vlineto: 横と縦を交互に
            6 | 7 => {
                if n == 0 {
                    return None;
                }
                for (i, &d) in args.iter().enumerate() {
                    if (i % 2 == 0) == (op == 6) {
                        self.line(d, 0.0);
                    } else {
                        self.line(0.0, d);
                    }
                }
            }
            // rrcurveto
            8 => {
                if n == 0 || !n.is_multiple_of(6) {
                    return None;
                }
                for &[a, b, c, d, e, f] in args.as_chunks::<6>().0 {
                    self.curve(a, b, c, d, e, f);
                }
            }
            // rcurveline: 曲線の後に直線
            24 => {
                if n < 8 || !(n - 2).is_multiple_of(6) {
                    return None;
                }
                let (curves, line) = args.split_at(n - 2);
                for &[a, b, c, d, e, f] in curves.as_chunks::<6>().0 {
                    self.curve(a, b, c, d, e, f);
                }
                let [dx, dy] = *line else { return None };
                self.line(dx, dy);
            }
            // rlinecurve: 直線の後に曲線
            25 => {
                if n < 8 || !n.is_multiple_of(2) {
                    return None;
                }
                let (lines, curve) = args.split_at(n - 6);
                for &[dx, dy] in lines.as_chunks::<2>().0 {
                    self.line(dx, dy);
                }
                let [a, b, c, d, e, f] = *curve else {
                    return None;
                };
                self.curve(a, b, c, d, e, f);
            }
            // vvcurveto・hhcurveto: 引数が 4 で割って 1 余れば、最初は横（縦）の差から始まる
            26 | 27 => {
                if n < 4 || n % 4 > 1 {
                    return None;
                }
                let (first, rest) = if n % 4 == 1 {
                    let (first, rest) = args.split_first()?;
                    (*first, rest)
                } else {
                    (0.0, args)
                };
                for (i, &[a, b, c, d]) in rest.as_chunks::<4>().0.iter().enumerate() {
                    let side = if i == 0 { first } else { 0.0 };
                    if op == 26 {
                        self.curve(side, a, b, c, 0.0, d);
                    } else {
                        self.curve(a, side, b, c, d, 0.0);
                    }
                }
            }
            // vhcurveto・hvcurveto: 縦と横の接線を交互に。4 で割って 1 余れば、最後の曲線の終わりの差
            30 | 31 => {
                if n < 4 || n % 4 > 1 {
                    return None;
                }
                let (body, extra) = if n % 4 == 1 {
                    let (last, body) = args.split_last()?;
                    (body, *last)
                } else {
                    (args, 0.0)
                };
                let curves = body.as_chunks::<4>().0;
                let mut horizontal = op == 31;
                for (i, &[a, b, c, d]) in curves.iter().enumerate() {
                    let e = if i + 1 == curves.len() { extra } else { 0.0 };
                    if horizontal {
                        self.curve(a, 0.0, b, c, e, d);
                    } else {
                        self.curve(0.0, a, b, c, d, e);
                    }
                    horizontal = !horizontal;
                }
            }
            // hflex
            0x0C22 => {
                let [dx1, dx2, dy2, dx3, dx4, dx5, dx6] = *args else {
                    return None;
                };
                self.curve(dx1, 0.0, dx2, dy2, dx3, 0.0);
                self.curve(dx4, 0.0, dx5, -dy2, dx6, 0.0);
            }
            // flex（最後の引数は flex の深さで、外形に効かない）
            0x0C23 => {
                let [a, b, c, d, e, f, g, h, i, j, k, l, _] = *args else {
                    return None;
                };
                self.curve(a, b, c, d, e, f);
                self.curve(g, h, i, j, k, l);
            }
            // hflex1
            0x0C24 => {
                let [dx1, dy1, dx2, dy2, dx3, dx4, dx5, dy5, dx6] = *args else {
                    return None;
                };
                self.curve(dx1, dy1, dx2, dy2, dx3, 0.0);
                self.curve(dx4, 0.0, dx5, dy5, dx6, -(dy1 + dy2 + dy5));
            }
            // flex1: 最後の引数は、差の大きい向きの差。もう一方の向きは始めの点に戻る
            0x0C25 => {
                let [a, b, c, d, e, f, g, h, i, j, last] = *args else {
                    return None;
                };
                let (dx, dy) = (a + c + e + g + i, b + d + f + h + j);
                let (x, y) = if dx.abs() > dy.abs() {
                    (last, -dy)
                } else {
                    (-dx, last)
                };
                self.curve(a, b, c, d, e, f);
                self.curve(g, h, i, j, x, y);
            }
            // 予約の演算子と、算術・記憶の演算子（Type 2 の 4.5・4.6 節）は読まない
            _ => return None,
        }
        Some(())
    }

    fn move_by(&mut self, dx: f64, dy: f64) {
        let [x, y] = self.point;
        self.point = [x + dx, y + dy];
    }

    fn include(&mut self, x: f64, y: f64) {
        self.bounds = Some(match self.bounds {
            None => [x, y, x, y],
            Some([l, b, r, t]) => [l.min(x), b.min(y), r.max(x), t.max(y)],
        });
    }

    fn line(&mut self, dx: f64, dy: f64) {
        let [x, y] = self.point;
        self.include(x, y);
        self.move_by(dx, dy);
        let [x, y] = self.point;
        self.include(x, y);
    }

    fn curve(&mut self, a: f64, b: f64, c: f64, d: f64, e: f64, f: f64) {
        let [x0, y0] = self.point;
        let [x1, y1] = [x0 + a, y0 + b];
        let [x2, y2] = [x1 + c, y1 + d];
        let [x3, y3] = [x2 + e, y2 + f];
        self.include(x0, y0);
        self.include(x3, y3);
        for x in extrema(x0, x1, x2, x3).into_iter().flatten() {
            self.include(x, y0);
        }
        for y in extrema(y0, y1, y2, y3).into_iter().flatten() {
            self.include(x0, y);
        }
        self.point = [x3, y3];
    }
}

/// 3 次の Bézier 曲線の 1 つの軸の、0 < t < 1 の極値。制御点が両端の間にあれば極値はない
fn extrema(p0: f64, p1: f64, p2: f64, p3: f64) -> [Option<f64>; 2] {
    let (low, high) = (p0.min(p3), p0.max(p3));
    if (low..=high).contains(&p1) && (low..=high).contains(&p2) {
        return [None, None];
    }
    // 微分の 1/3: a t² + b t + c
    let a = p3 - p0 + 3.0 * (p1 - p2);
    let b = 2.0 * (p0 - 2.0 * p1 + p2);
    let c = p1 - p0;
    let roots = if a == 0.0 {
        [(b != 0.0).then(|| -c / b), None]
    } else {
        let discriminant = b * b - 4.0 * a * c;
        if discriminant < 0.0 {
            [None, None]
        } else {
            let root = discriminant.sqrt();
            [Some((-b + root) / (2.0 * a)), Some((-b - root) / (2.0 * a))]
        }
    };
    roots.map(|t| {
        let t = t.filter(|&t| t > 0.0 && t < 1.0)?;
        let u = 1.0 - t;
        Some(u * u * u * p0 + 3.0 * u * u * t * p1 + 3.0 * u * t * t * p2 + t * t * t * p3)
    })
}
