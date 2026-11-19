//! 常駐文書を運ぶ固定幅の枠。数値は little endian、要求番号は 0 以外。

use std::io::{self, Read, Write};

use zumen_base::{Kind, Limit, Limits, Stop};

pub(super) const HEADER_BYTES: usize = 24;
pub(super) const CONTROL_BYTES: usize = 72;
pub(super) const REPLY_BYTES: usize = 136;
/// 文書 IPC（版 1）の固定枠が運ぶ 7 種の予算単位
pub(super) const KINDS: [Kind; 7] = [
    Kind::InputBytes,
    Kind::DecodedBytes,
    Kind::Records,
    Kind::Elements,
    Kind::Depth,
    Kind::Segments,
    Kind::Work,
];
pub(super) const MAX_INPUT_BYTES: usize = 256 << 20;
/// Open に添える属性ファイル（SAF）の上限
pub(super) const MAX_SAF_BYTES: usize = 64 << 20;
/// Open の 2 語目に立てる、属性ファイル（SAF）ありの印。下位 31 bit がバイト数
pub(super) const OPEN_HAS_SAF: u32 = 0x8000_0000;
pub(super) const MAX_ROWS: usize = 65_536;
/// SetText の内容のバイト数の上限
pub(super) const MAX_TEXT_BYTES: usize = 1 << 20;
pub(super) const NONE: u64 = u64::MAX;
pub(super) const WITH_DISPLAY: u32 = 1;
pub(super) const FULL_DISPLAY: u32 = 2;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u16)]
pub(super) enum Command {
    Cancel = 0,
    Open = 1,
    Display = 2,
    Transform = 3,
    Undo = 4,
    Redo = 5,
    Save = 6,
    Target = 7,
    Diagnostics = 8,
    Close = 9,
    Select = 10,
    Layers = 11,
    Order = 12,
    Remove = 13,
    Copy = 14,
    Restyle = 15,
    LineTypes = 16,
    Texts = 17,
    SetText = 18,
    Symbols = 19,
    Draw = 20,
    SelectPoint = 21,
    DirtySymbols = 22,
    EditLayer = 23,
    EditLineType = 24,
    AddElement = 25,
    AddSymbol = 26,
    TransformSymbol = 27,
}

impl Command {
    fn from_word(value: u16) -> Option<Self> {
        Some(match value {
            0 => Self::Cancel,
            1 => Self::Open,
            2 => Self::Display,
            3 => Self::Transform,
            4 => Self::Undo,
            5 => Self::Redo,
            6 => Self::Save,
            7 => Self::Target,
            8 => Self::Diagnostics,
            9 => Self::Close,
            10 => Self::Select,
            11 => Self::Layers,
            12 => Self::Order,
            13 => Self::Remove,
            14 => Self::Copy,
            15 => Self::Restyle,
            16 => Self::LineTypes,
            17 => Self::Texts,
            18 => Self::SetText,
            19 => Self::Symbols,
            20 => Self::Draw,
            21 => Self::SelectPoint,
            22 => Self::DirtySymbols,
            23 => Self::EditLayer,
            24 => Self::EditLineType,
            25 => Self::AddElement,
            26 => Self::AddSymbol,
            27 => Self::TransformSymbol,
            _ => return None,
        })
    }

    fn length_range(self) -> (usize, usize) {
        match self {
            Self::Cancel | Self::Close => (0, 0),
            Self::Open => (
                CONTROL_BYTES + 8,
                CONTROL_BYTES + 8 + MAX_INPUT_BYTES + MAX_SAF_BYTES,
            ),
            Self::Transform | Self::Copy | Self::TransformSymbol => {
                (CONTROL_BYTES + 56, CONTROL_BYTES + 56 + MAX_ROWS * 4)
            }
            Self::Remove | Self::Texts | Self::Symbols => {
                (CONTROL_BYTES + 8, CONTROL_BYTES + 8 + MAX_ROWS * 4)
            }
            Self::Restyle => (CONTROL_BYTES + 48, CONTROL_BYTES + 56 + MAX_ROWS * 4),
            Self::SetText => (
                CONTROL_BYTES + 8,
                CONTROL_BYTES + 8 + MAX_TEXT_BYTES + MAX_ROWS * 4,
            ),
            Self::Target | Self::DirtySymbols => (CONTROL_BYTES + 4, CONTROL_BYTES + 4),
            Self::Draw => (CONTROL_BYTES + 48, CONTROL_BYTES + 48),
            Self::Diagnostics => (CONTROL_BYTES + 8, CONTROL_BYTES + 8),
            Self::Select => (CONTROL_BYTES + 32, CONTROL_BYTES + 32),
            Self::SelectPoint => (CONTROL_BYTES + 24, CONTROL_BYTES + 24),
            Self::EditLayer => (
                CONTROL_BYTES + 8,
                CONTROL_BYTES + 52 + super::edits::MAX_NAME_BYTES,
            ),
            Self::EditLineType => (
                CONTROL_BYTES + 8,
                CONTROL_BYTES
                    + 24
                    + super::edits::MAX_NAME_BYTES * 2
                    + super::edits::MAX_PATTERN_LEN * 8,
            ),
            Self::AddElement => (
                CONTROL_BYTES + 56,
                CONTROL_BYTES + 56 + MAX_TEXT_BYTES.max(MAX_ROWS * 24),
            ),
            Self::AddSymbol => (
                CONTROL_BYTES + 36,
                CONTROL_BYTES
                    + 32
                    + super::edits::MAX_NAME_BYTES
                    + super::edits::MAX_SYMBOL_ELEMENTS * 48,
            ),
            Self::Display
            | Self::Undo
            | Self::Redo
            | Self::Save
            | Self::Layers
            | Self::Order
            | Self::LineTypes => (CONTROL_BYTES, CONTROL_BYTES),
        }
    }

    fn can_display(self) -> bool {
        matches!(
            self,
            Self::Open
                | Self::Display
                | Self::Transform
                | Self::Remove
                | Self::Restyle
                | Self::SetText
                | Self::EditLayer
                | Self::EditLineType
                | Self::AddSymbol
                | Self::TransformSymbol
                | Self::Undo
                | Self::Redo
        )
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Header {
    pub command: Command,
    pub id: u64,
    pub length: usize,
    pub flags: u32,
}

impl Header {
    pub fn parse(bytes: &[u8; HEADER_BYTES]) -> Option<Self> {
        let mut input = Words(bytes);
        if input.take::<4>()? != *b"ZLIP" || input.u16()? != 1 {
            return None;
        }
        let command = Command::from_word(input.u16()?)?;
        let id = input.u64()?;
        let length = input.u32()? as usize;
        let flags = input.u32()?;
        let (minimum, maximum) = command.length_range();
        if id == 0
            || !(minimum..=maximum).contains(&length)
            || flags & !(WITH_DISPLAY | FULL_DISPLAY) != 0
            || (flags != 0 && !command.can_display())
            || (flags & FULL_DISPLAY != 0 && flags & WITH_DISPLAY == 0)
        {
            return None;
        }
        Some(Self {
            command,
            id,
            length,
            flags,
        })
    }

    pub fn response(self, length: usize) -> io::Result<[u8; HEADER_BYTES]> {
        if length > zumen_live::display_wire::MAX_PACKET_BYTES + REPLY_BYTES {
            return Err(invalid());
        }
        let length = u32::try_from(length).map_err(|_| invalid())?;
        let mut bytes = [0_u8; HEADER_BYTES];
        let mut out = bytes.as_mut_slice();
        out.write_all(b"ZLIR")?;
        out.write_all(&1_u16.to_le_bytes())?;
        out.write_all(&(self.command as u16).to_le_bytes())?;
        out.write_all(&self.id.to_le_bytes())?;
        out.write_all(&length.to_le_bytes())?;
        out.write_all(&self.flags.to_le_bytes())?;
        Ok(bytes)
    }
}

pub(super) fn read_header(input: &mut impl Read) -> io::Result<Option<Header>> {
    let mut bytes = [0_u8; HEADER_BYTES];
    let (first, rest) = bytes.split_first_mut().ok_or_else(invalid)?;
    loop {
        match input.read(std::slice::from_mut(first)) {
            Ok(0) => return Ok(None),
            Ok(_) => break,
            Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
            Err(error) => return Err(error),
        }
    }
    input.read_exact(rest)?;
    Header::parse(&bytes).map(Some).ok_or_else(invalid)
}

pub(super) fn invalid() -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, "文書 IPC の枠が正しくない")
}

pub(super) struct Control {
    pub limits: Limits,
    pub revision: u64,
    pub ack: u64,
}

impl Control {
    pub fn parse(bytes: &[u8; CONTROL_BYTES]) -> Option<Self> {
        let mut input = Words(bytes);
        let mut limits = Limits::default();
        for kind in KINDS {
            let value = input.u64()?;
            limits.set(
                kind,
                if value == NONE {
                    Limit::Unlimited
                } else {
                    Limit::Finite(value)
                },
            );
        }
        Some(Self {
            limits,
            revision: input.u64()?,
            ack: input.u64()?,
        })
    }
}

pub(super) struct Words<'a>(pub &'a [u8]);

impl Words<'_> {
    pub fn take<const N: usize>(&mut self) -> Option<[u8; N]> {
        let (word, rest) = self.0.split_first_chunk::<N>()?;
        self.0 = rest;
        Some(*word)
    }

    pub fn u16(&mut self) -> Option<u16> {
        self.take().map(u16::from_le_bytes)
    }
    pub fn u32(&mut self) -> Option<u32> {
        self.take().map(u32::from_le_bytes)
    }
    pub fn u64(&mut self) -> Option<u64> {
        self.take().map(u64::from_le_bytes)
    }
    pub fn f64(&mut self) -> Option<f64> {
        self.u64().map(f64::from_bits)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[repr(u32)]
pub(super) enum Status {
    #[default]
    Ok = 0,
    Invalid = 1,
    Unsupported = 2,
    Stale = 3,
    Missing = 4,
    Rejected = 5,
    Exceeded = 6,
    Cancelled = 7,
    TimedOut = 8,
    NoDocument = 9,
    AlreadyOpen = 10,
    NoDiagnostics = 11,
}

impl Status {
    pub fn stopped(stop: Stop) -> Self {
        match stop {
            Stop::Exceeded(_) => Self::Exceeded,
            Stop::Cancelled => Self::Cancelled,
            Stop::TimedOut => Self::TimedOut,
        }
    }
}

pub(super) struct Reply {
    pub status: Status,
    pub display_status: Status,
    pub revision: u64,
    pub state: u64,
    pub result: u64,
    pub base: u64,
    pub data_kind: u32,
    pub diagnostics: u32,
    pub used: [u64; 7],
    pub stop: Option<Stop>,
    pub data: Vec<u8>,
}

impl Default for Reply {
    fn default() -> Self {
        Self {
            status: Status::Ok,
            display_status: Status::Ok,
            revision: NONE,
            state: NONE,
            result: 0,
            base: NONE,
            data_kind: 0,
            diagnostics: 0,
            used: [0; 7],
            stop: None,
            data: Vec::new(),
        }
    }
}

impl Reply {
    pub fn write(&self, header: Header, out: &mut impl Write) -> io::Result<()> {
        let mut bytes = [0_u8; REPLY_BYTES];
        let mut fields = bytes.as_mut_slice();
        fields.write_all(&(self.status as u32).to_le_bytes())?;
        fields.write_all(&(self.display_status as u32).to_le_bytes())?;
        fields.write_all(&self.revision.to_le_bytes())?;
        fields.write_all(&self.state.to_le_bytes())?;
        fields.write_all(&self.result.to_le_bytes())?;
        fields.write_all(&self.base.to_le_bytes())?;
        fields.write_all(&self.data_kind.to_le_bytes())?;
        fields.write_all(&self.diagnostics.to_le_bytes())?;
        fields.write_all(
            &u32::try_from(self.data.len())
                .map_err(|_| invalid())?
                .to_le_bytes(),
        )?;
        fields.write_all(&0_u32.to_le_bytes())?;
        for value in self.used {
            fields.write_all(&value.to_le_bytes())?;
        }
        let (kind, limit, requested) = match self.stop {
            None => (0_u32, 0_u64, 0_u64),
            Some(Stop::Cancelled) => (8, 0, 0),
            Some(Stop::TimedOut) => (9, 0, 0),
            Some(Stop::Exceeded(value)) => (
                KINDS
                    .iter()
                    .position(|&kind| kind == value.kind)
                    .map_or(2, |index| index as u32 + 1),
                value.limit,
                value.requested,
            ),
        };
        fields.write_all(&kind.to_le_bytes())?;
        fields.write_all(&limit.to_le_bytes())?;
        fields.write_all(&requested.to_le_bytes())?;
        fields.write_all(&0_u32.to_le_bytes())?;
        let length = REPLY_BYTES
            .checked_add(self.data.len())
            .ok_or_else(invalid)?;
        out.write_all(&header.response(length)?)?;
        out.write_all(&bytes)?;
        out.write_all(&self.data)?;
        out.flush()
    }
}
