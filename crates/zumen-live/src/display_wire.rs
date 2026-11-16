//! 表示の 13 列を、64 要素の塊で送るバイナリの形式（`docs/live.md`、#878）。
//!
//! 整数と浮動小数点のビット列を little endian で置く。変更追跡は消さないため、書き出し・送信の失敗後も
//! 同じ差分を取り出せる。受信側の反映が済んだ版を確認してから、呼び出し側が [`Display::clear_dirty`] する。
//! 文書の版、行の ID、画層・文字・記号の定義は、この表示の列とは別に同期する。

use core::fmt;
use core::ops::Range;

use zumen_base::{Cx, Kind, Stop};

use crate::{DIRTY_BLOCK, DirtyColumn, Display, Paged};

/// 1 パケットと、受信後の全列の合計バイト数の上限（1 GiB）。要求の予算は別に共有する。
pub const MAX_PACKET_BYTES: usize = 1 << 30;
const HEADER: usize = 16;
const COLUMN_HEADER: usize = 16;
const BLOCK_HEADER: usize = 8;
const WIDTHS: [u32; 13] = [8, 1, 2, 7, 1, 2, 8, 1, 2, 9, 3, 1, 2];

/// 表示の列。パケット内の順番と番号は固定する。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u32)]
pub enum Column {
    /// 線分の座標（f32 × 8）
    Segments,
    /// 線分の持ち主（u32）
    SegmentOwner,
    /// 線分の画層と色（u32 × 2）
    SegmentStyle,
    /// 円弧の座標と角（f32 × 7）
    Arcs,
    /// 円弧の持ち主（u32）
    ArcOwner,
    /// 円弧の画層と色（u32 × 2）
    ArcStyle,
    /// 文字の位置と姿勢（f32 × 8）
    Texts,
    /// 文字の持ち主（u32）
    TextOwner,
    /// 文字の画層と色（u32 × 2）
    TextStyle,
    /// 挿入の位置と尺度（f32 × 9）
    Instances,
    /// 挿入の定義番号・列数・行数（u32 × 3）
    InstanceRef,
    /// 挿入の持ち主（u32）
    InstanceOwner,
    /// 挿入の画層と色（u32 × 2）
    InstanceStyle,
}

const COLUMNS: [Column; 13] = [
    Column::Segments,
    Column::SegmentOwner,
    Column::SegmentStyle,
    Column::Arcs,
    Column::ArcOwner,
    Column::ArcStyle,
    Column::Texts,
    Column::TextOwner,
    Column::TextStyle,
    Column::Instances,
    Column::InstanceRef,
    Column::InstanceOwner,
    Column::InstanceStyle,
];

/// 通信の形か上限が合わない、または共有する予算・中断で止まった。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    /// ヘッダー・列・範囲・長さが契約と違う。ハード上限の超過も含む。
    Invalid,
    /// 共有する予算か中断で止まった。
    Stopped(Stop),
}

impl From<Stop> for Error {
    fn from(stop: Stop) -> Self {
        Self::Stopped(stop)
    }
}

impl fmt::Display for Error {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Invalid => out.write_str("表示のパケットの形か長さが契約と違う"),
            Self::Stopped(stop) => write!(out, "表示のパケットの処理が止まった: {stop:?}"),
        }
    }
}

impl std::error::Error for Error {}

trait Word: Copy {
    fn word(self) -> u32;
}
impl Word for u32 {
    fn word(self) -> u32 {
        self
    }
}
impl Word for f32 {
    fn word(self) -> u32 {
        self.to_bits()
    }
}

trait Row {
    const WIDTH: usize;
    fn write(&self, out: &mut Vec<u8>);
}
impl Row for u32 {
    const WIDTH: usize = 1;
    fn write(&self, out: &mut Vec<u8>) {
        out.extend_from_slice(&self.to_le_bytes());
    }
}
impl<T: Word, const N: usize> Row for [T; N] {
    const WIDTH: usize = N;
    fn write(&self, out: &mut Vec<u8>) {
        for value in self {
            out.extend_from_slice(&value.word().to_le_bytes());
        }
    }
}

fn blocks(
    dirty: &DirtyColumn,
    rows: usize,
    full: bool,
    cx: &mut Cx<'_>,
    mut visit: impl FnMut(Range<usize>, &mut Cx<'_>) -> Result<(), Stop>,
) -> Result<(), Stop> {
    if full {
        for start in (0..rows).step_by(DIRTY_BLOCK) {
            cx.check(Kind::Work, 1)?;
            visit(start..start.saturating_add(DIRTY_BLOCK).min(rows), cx)?;
        }
        Ok(())
    } else {
        dirty.visit_blocks_with_cx(rows, cx, visit)
    }
}

#[derive(Clone, Copy, Default)]
struct Plan {
    rows: u32,
    count: u32,
    bytes: u64,
}

struct Encoder<'a, 'b> {
    out: Option<&'a mut Vec<u8>>,
    plans: &'a mut [Plan; 13],
    full: bool,
    cx: &'a mut Cx<'b>,
}

impl Encoder<'_, '_> {
    fn column<T: Row>(
        &mut self,
        id: Column,
        values: &Paged<T>,
        dirty: &DirtyColumn,
    ) -> Result<(), Error> {
        self.cx.check(Kind::Work, 1)?;
        let plan = self.plans.get_mut(id as usize).ok_or(Error::Invalid)?;
        if let Some(out) = &mut self.out {
            for value in [id as u32, T::WIDTH as u32, plan.rows, plan.count] {
                out.extend_from_slice(&value.to_le_bytes());
            }
            blocks(dirty, values.len(), self.full, self.cx, |range, cx| {
                cx.check(Kind::Records, 1)?;
                cx.check(Kind::Work, (range.len() * T::WIDTH) as u64)?;
                out.extend_from_slice(&(range.start as u32).to_le_bytes());
                out.extend_from_slice(&(range.len() as u32).to_le_bytes());
                if let Some(rows) = values.page_slice(range.start, range.end) {
                    for row in rows {
                        row.write(out);
                    }
                }
                Ok(())
            })?;
        } else {
            plan.rows = u32::try_from(values.len()).map_err(|_| Error::Invalid)?;
            plan.bytes = COLUMN_HEADER as u64;
            blocks(dirty, values.len(), self.full, self.cx, |range, _| {
                plan.count += 1;
                plan.bytes += (BLOCK_HEADER + range.len() * T::WIDTH * 4) as u64;
                Ok(())
            })?;
        }
        Ok(())
    }

    fn display(&mut self, d: &Display) -> Result<(), Error> {
        self.column(Column::Segments, &d.segments, &d.dirty.segments)?;
        self.column(
            Column::SegmentOwner,
            &d.segment_owner,
            &d.dirty.segment_owner,
        )?;
        self.column(
            Column::SegmentStyle,
            &d.segment_style,
            &d.dirty.segment_style,
        )?;
        self.column(Column::Arcs, &d.arcs, &d.dirty.arcs)?;
        self.column(Column::ArcOwner, &d.arc_owner, &d.dirty.arc_owner)?;
        self.column(Column::ArcStyle, &d.arc_style, &d.dirty.arc_style)?;
        self.column(Column::Texts, &d.texts, &d.dirty.texts)?;
        self.column(Column::TextOwner, &d.text_owner, &d.dirty.text_owner)?;
        self.column(Column::TextStyle, &d.text_style, &d.dirty.text_style)?;
        self.column(Column::Instances, &d.instances, &d.dirty.instances)?;
        self.column(Column::InstanceRef, &d.instance_ref, &d.dirty.instance_ref)?;
        self.column(
            Column::InstanceOwner,
            &d.instance_owner,
            &d.dirty.instance_owner,
        )?;
        self.column(
            Column::InstanceStyle,
            &d.instance_style,
            &d.dirty.instance_style,
        )
    }
}

fn valid_lengths(rows: &[u32; 13]) -> bool {
    let mut size = 0u64;
    let mut previous = 0;
    for (index, (&count, width)) in rows.iter().zip(WIDTHS).enumerate() {
        if matches!(index, 0 | 3 | 6 | 9) {
            previous = count;
        }
        if count != previous {
            return false;
        }
        size += u64::from(count) * u64::from(width) * 4;
    }
    size <= MAX_PACKET_BYTES as u64
}

/// 全列または変更した塊を、予算検査後に 1 つのパケットへ写す。成功・失敗とも変更追跡を保つ。
pub fn encode(display: &Display, full: bool, cx: &mut Cx<'_>) -> Result<Vec<u8>, Error> {
    let mut plans = [Plan::default(); 13];
    Encoder {
        out: None,
        plans: &mut plans,
        full,
        cx,
    }
    .display(display)?;
    let bytes = plans.iter().fold(HEADER as u64, |n, p| n + p.bytes);
    if bytes > MAX_PACKET_BYTES as u64 || !valid_lengths(&plans.map(|p| p.rows)) {
        return Err(Error::Invalid);
    }
    cx.check(Kind::DecodedBytes, bytes)?;
    let mut out = Vec::with_capacity(bytes as usize);
    out.extend_from_slice(b"ZLDP");
    out.extend_from_slice(&1u16.to_le_bytes());
    out.extend_from_slice(&u16::from(full).to_le_bytes());
    out.extend_from_slice(&(bytes as u32).to_le_bytes());
    out.extend_from_slice(&13u32.to_le_bytes());
    Encoder {
        out: Some(&mut out),
        plans: &mut plans,
        full,
        cx,
    }
    .display(display)?;
    Ok(out)
}

/// 検証済みの 1 列。値は元のパケットを借り、確保しない。
#[derive(Clone, Copy, Debug)]
pub struct PacketColumn<'a> {
    kind: Column,
    rows: u32,
    stride: u32,
    bytes: &'a [u8],
}

impl<'a> PacketColumn<'a> {
    /// 列の種類。
    pub const fn kind(&self) -> Column {
        self.kind
    }
    /// 反映後の要素数。塊がなくても、縮んだ長さを反映する。
    pub const fn rows(&self) -> u32 {
        self.rows
    }
    /// 1 要素のバイト数。
    pub const fn stride(&self) -> u32 {
        self.stride
    }
    /// 64 要素以下の塊を昇順に借りる。検証は [`Packet::parse`] で済ませる。
    pub fn blocks(&self) -> Blocks<'a> {
        Blocks {
            bytes: self.bytes,
            stride: self.stride,
        }
    }
}

/// 検証済みの 1 塊。数値は little endian のビット列。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Block<'a> {
    /// 列の先頭からの要素番号（64 の倍数）
    pub start: u32,
    /// 要素数（1 から 64。64 未満は列の末尾だけ）
    pub rows: u32,
    /// 要素数と列の stride の積と同じ長さの値
    pub bytes: &'a [u8],
}

/// 検証済みの塊を順に借りる反復子。
pub struct Blocks<'a> {
    bytes: &'a [u8],
    stride: u32,
}

impl<'a> Iterator for Blocks<'a> {
    type Item = Block<'a>;
    fn next(&mut self) -> Option<Self::Item> {
        let start = take_u32(&mut self.bytes).ok()?;
        let rows = take_u32(&mut self.bytes).ok()?;
        let bytes = take(&mut self.bytes, (rows * self.stride) as usize).ok()?;
        Some(Block { start, rows, bytes })
    }
}

/// 全体を検証した表示のパケット。元のバイト列を借り、一時配列を確保しない。
#[derive(Debug)]
pub struct Packet<'a> {
    full: bool,
    columns: [PacketColumn<'a>; 13],
}

impl<'a> Packet<'a> {
    /// 全列を置き換えるパケットか。差分では、呼び出し側が前後の文書の版を照合する。
    pub const fn full(&self) -> bool {
        self.full
    }
    /// ヘッダー、13 列の長さ、塊の順序・範囲・終端をすべて確かめてから返す。
    pub fn parse(bytes: &'a [u8], cx: &mut Cx<'_>) -> Result<Self, Error> {
        cx.check(Kind::InputBytes, bytes.len() as u64)?;
        if bytes.len() > MAX_PACKET_BYTES {
            return Err(Error::Invalid);
        }
        let mut input = bytes;
        if take(&mut input, 6)? != b"ZLDP\x01\0" {
            return Err(Error::Invalid);
        }
        let flags = take(&mut input, 2)?;
        let full = match flags {
            [0, 0] => false,
            [1, 0] => true,
            _ => return Err(Error::Invalid),
        };
        if take_u32(&mut input)? as usize != bytes.len() || take_u32(&mut input)? != 13 {
            return Err(Error::Invalid);
        }
        let mut columns = [PacketColumn {
            kind: Column::Segments,
            rows: 0,
            stride: 0,
            bytes: &[],
        }; 13];
        for ((column, kind), width) in columns.iter_mut().zip(COLUMNS).zip(WIDTHS) {
            cx.check(Kind::Work, 1)?;
            if take_u32(&mut input)? != kind as u32 || take_u32(&mut input)? != width {
                return Err(Error::Invalid);
            }
            let rows = take_u32(&mut input)?;
            let count = take_u32(&mut input)?;
            if count > rows.div_ceil(DIRTY_BLOCK as u32)
                || (full && count != rows.div_ceil(DIRTY_BLOCK as u32))
            {
                return Err(Error::Invalid);
            }
            let start_bytes = input;
            let mut end = 0;
            for _ in 0..count {
                cx.check(Kind::Records, 1)?;
                cx.check(Kind::Work, 1)?;
                let first = take_u32(&mut input)?;
                let size = take_u32(&mut input)?;
                if first >= rows
                    || first % DIRTY_BLOCK as u32 != 0
                    || first < end
                    || (full && first != end)
                    || size != (rows - first).min(DIRTY_BLOCK as u32)
                {
                    return Err(Error::Invalid);
                }
                take(&mut input, (size * width * 4) as usize)?;
                end = first + size;
            }
            let used = start_bytes.len() - input.len();
            *column = PacketColumn {
                kind,
                rows,
                stride: width * 4,
                bytes: start_bytes.get(..used).ok_or(Error::Invalid)?,
            };
        }
        if !input.is_empty() || !valid_lengths(&columns.map(|c| c.rows)) {
            return Err(Error::Invalid);
        }
        Ok(Self { full, columns })
    }

    /// 固定順の 13 列を借りる。
    pub const fn columns(&self) -> &[PacketColumn<'a>; 13] {
        &self.columns
    }
}

fn take<'a>(input: &mut &'a [u8], count: usize) -> Result<&'a [u8], Error> {
    let (head, tail) = input.split_at_checked(count).ok_or(Error::Invalid)?;
    *input = tail;
    Ok(head)
}

fn take_u32(input: &mut &[u8]) -> Result<u32, Error> {
    let word = take(input, 4)?.first_chunk::<4>().ok_or(Error::Invalid)?;
    Ok(u32::from_le_bytes(*word))
}

#[cfg(test)]
mod tests;
