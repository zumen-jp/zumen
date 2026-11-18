//! 1 子プロセスの 1 文書を、要求順に処理する。

use zumen_base::{Budget, CancelToken, Cx, Diagnostics, Error, Kind, Stop};
use zumen_drawing::{
    Affine, Color, ColorCode, ColorSystem, Inherit, Layer, LineType, LineWeight, PaperLength,
    PatternBasis, Restyle,
};
use zumen_live::{Revision, RowId, display_wire};

use super::diagnostics;
use super::document::{Document, DrawShape, Problem, RowEdit, Symbols};
use super::wire::{self, Command, Control, Header, Reply, Status, Words};

pub(super) struct Request {
    pub header: Header,
    pub control: Control,
    pub data: Vec<u8>,
    pub budget: Budget,
    pub diagnostics: Diagnostics,
    pub cancel: CancelToken,
    pub failure: Option<Stop>,
}

#[derive(Default)]
pub(super) struct Host {
    document: Option<Document>,
    diagnostics: Option<(u64, Diagnostics)>,
}

enum Operation<'a> {
    Open { threads: usize, bytes: &'a [u8] },
    Transform { transform: Affine, rows: &'a [u8] },
    Copy { transform: Affine, rows: &'a [u8] },
    Remove(&'a [u8]),
    Texts(&'a [u8]),
    Symbols(&'a [u8]),
    SetText { content: &'a str, rows: &'a [u8] },
    Restyle { style: WireStyle, rows: &'a [u8] },
    Target(RowId),
    Draw { template: RowId, shape: DrawShape },
    Select([f64; 4]),
    Diagnostics(u64),
    Empty,
}

fn operation(command: Command, bytes: &[u8]) -> Option<Operation<'_>> {
    let mut input = Words(bytes);
    match command {
        Command::Open => {
            let threads = input.u32()?;
            if !(1..=64).contains(&threads) || input.u32()? != 0 {
                return None;
            }
            Some(Operation::Open {
                threads: threads as usize,
                bytes: input.0,
            })
        }
        Command::Transform | Command::Copy => {
            let count = input.u32()? as usize;
            if count > wire::MAX_ROWS || input.u32()? != 0 {
                return None;
            }
            let mut m = [0_f64; 6];
            for value in &mut m {
                *value = input.f64()?;
            }
            if input.0.len() != count.checked_mul(4)? {
                return None;
            }
            let transform = Affine { m };
            Some(if command == Command::Copy {
                Operation::Copy {
                    transform,
                    rows: input.0,
                }
            } else {
                Operation::Transform {
                    transform,
                    rows: input.0,
                }
            })
        }
        Command::Restyle => {
            let count = input.u32()? as usize;
            if count > wire::MAX_ROWS {
                return None;
            }
            let style = WireStyle::read(&mut input)?;
            (input.0.len() == count.checked_mul(4)?).then_some(Operation::Restyle {
                style,
                rows: input.0,
            })
        }
        Command::Remove | Command::Texts | Command::Symbols => {
            let count = input.u32()? as usize;
            if count > wire::MAX_ROWS || input.u32()? != 0 {
                return None;
            }
            (input.0.len() == count.checked_mul(4)?).then_some(match command {
                Command::Texts => Operation::Texts(input.0),
                Command::Symbols => Operation::Symbols(input.0),
                _ => Operation::Remove(input.0),
            })
        }
        Command::SetText => {
            let count = input.u32()? as usize;
            let length = input.u32()? as usize;
            if count > wire::MAX_ROWS || length > wire::MAX_TEXT_BYTES {
                return None;
            }
            let padded = length.next_multiple_of(4);
            if input.0.len() != padded.checked_add(count.checked_mul(4)?)? {
                return None;
            }
            let (text, rows) = input.0.split_at(padded);
            let (content, padding) = text.split_at(length);
            if padding.iter().any(|&byte| byte != 0) {
                return None;
            }
            Some(Operation::SetText {
                content: std::str::from_utf8(content).ok()?,
                rows,
            })
        }
        Command::Draw => {
            let template = RowId(input.u32()?);
            let kind = input.u32()?;
            let mut v = [0_f64; 5];
            for value in &mut v {
                *value = input.f64()?;
            }
            if !input.0.is_empty() || v.iter().any(|value| !value.is_finite()) {
                return None;
            }
            // 使わない値は 0。半径は正
            let shape = match kind {
                1 if v[4] == 0.0 => DrawShape::Line([v[0], v[1], v[2], v[3]]),
                2 if v[3] == 0.0 && v[4] == 0.0 && v[2] > 0.0 => {
                    DrawShape::Circle([v[0], v[1], v[2]])
                }
                3 if v[2] > 0.0 => DrawShape::Arc(v),
                _ => return None,
            };
            Some(Operation::Draw { template, shape })
        }
        Command::Target => {
            let row = RowId(input.u32()?);
            input.0.is_empty().then_some(Operation::Target(row))
        }
        Command::Select => {
            let mut rect = [0_f64; 4];
            for value in &mut rect {
                *value = input.f64()?;
            }
            input.0.is_empty().then_some(Operation::Select(rect))
        }
        Command::Diagnostics => {
            let id = input.u64()?;
            input.0.is_empty().then_some(Operation::Diagnostics(id))
        }
        _ => bytes.is_empty().then_some(Operation::Empty),
    }
}

/// Restyle の要求のスタイル。画層は表示のスタイルの画層の値で受け、図面モデルの画層に引く前の形
#[derive(Clone, Copy, Debug, PartialEq)]
struct WireStyle {
    layer: Option<u32>,
    color: Option<Inherit<Color>>,
    line_weight: Option<Inherit<LineWeight>>,
    line_type_scale: Option<f64>,
    /// 線種。自分の値は線種の表の値（図面モデルの `LineTypeId`）
    line_type: Option<Inherit<u32>>,
}

impl WireStyle {
    /// 変える項目 u32、画層 u32、色の種類 u32・色のフラグ u32・色番号 u32・RGB u32、線幅の種類 u32、線幅 f64、
    /// 線種の倍率 f64。変える項目の bit 4 が立っていれば、続けて線種の種類 u32・線種 u32。使わない値と予約ビットは 0
    fn read(input: &mut Words<'_>) -> Option<Self> {
        let mask = input.u32()?;
        let layer = input.u32()?;
        let (color_kind, color_flags, code, rgb) =
            (input.u32()?, input.u32()?, input.u32()?, input.u32()?);
        let weight_kind = input.u32()?;
        let (width, scale) = (input.f64()?, input.f64()?);
        if mask == 0 || mask > 0b1_1111 {
            return None;
        }
        let layer = if mask & 1 != 0 {
            Some(layer)
        } else {
            (layer == 0).then_some(None)?
        };
        let color = if mask & 2 != 0 {
            Some(match color_kind {
                0 | 1 if color_flags == 0 && code == 0 && rgb == 0 => {
                    if color_kind == 0 {
                        Inherit::Layer
                    } else {
                        Inherit::Block
                    }
                }
                2 if (1..=3).contains(&color_flags) => {
                    let code = if color_flags & 1 != 0 {
                        let system = match code >> 24 {
                            1 => ColorSystem::Aci,
                            2 => ColorSystem::Sxf,
                            3 => ColorSystem::Jww,
                            _ => return None,
                        };
                        Some(ColorCode {
                            system,
                            value: code & 0x00ff_ffff,
                        })
                    } else if code == 0 {
                        None
                    } else {
                        return None;
                    };
                    let rgb = if color_flags & 2 != 0 {
                        if rgb > 0x00ff_ffff {
                            return None;
                        }
                        let [_, r, g, b] = rgb.to_be_bytes();
                        Some([r, g, b])
                    } else if rgb == 0 {
                        None
                    } else {
                        return None;
                    };
                    Inherit::Own(Color { code, rgb })
                }
                _ => return None,
            })
        } else if color_kind == 0 && color_flags == 0 && code == 0 && rgb == 0 {
            None
        } else {
            return None;
        };
        let line_weight = if mask & 4 != 0 {
            match weight_kind {
                0 if width == 0.0 => Some(Inherit::Layer),
                1 if width == 0.0 => Some(Inherit::Block),
                2 if width.is_finite() && width >= 0.0 => {
                    Some(Inherit::Own(LineWeight::Width(PaperLength(width))))
                }
                3 if width == 0.0 => Some(Inherit::Own(LineWeight::Default)),
                _ => return None,
            }
        } else if weight_kind == 0 && width.to_bits() == 0 {
            None
        } else {
            return None;
        };
        let line_type_scale = if mask & 8 != 0 {
            (scale.is_finite() && scale > 0.0).then_some(Some(scale))?
        } else if scale.to_bits() == 0 {
            None
        } else {
            return None;
        };
        let line_type = if mask & 16 != 0 {
            let (kind, value) = (input.u32()?, input.u32()?);
            Some(match kind {
                0 if value == 0 => Inherit::Layer,
                1 if value == 0 => Inherit::Block,
                2 => Inherit::Own(value),
                _ => return None,
            })
        } else {
            None
        };
        Some(Self {
            layer,
            color,
            line_weight,
            line_type_scale,
            line_type,
        })
    }

    /// 画層の値を表示の画層の表で、線種の値を線種の表で、図面モデルの ID に引く。表にない値は `None`
    fn resolve(self, layers: &[(u32, Layer)], line_types: &[LineType]) -> Option<Restyle> {
        let layer = match self.layer {
            None => None,
            Some(value) => Some(layers.iter().find(|(slot, _)| *slot == value)?.1.id),
        };
        let line_type = match self.line_type {
            None => None,
            Some(Inherit::Layer) => Some(Inherit::Layer),
            Some(Inherit::Block) => Some(Inherit::Block),
            Some(Inherit::Own(value)) => Some(Inherit::Own(
                line_types
                    .iter()
                    .find(|line_type| line_type.id.0 == value)?
                    .id,
            )),
        };
        Some(Restyle {
            layer,
            color: self.color,
            line_type,
            line_type_scale: self.line_type_scale,
            line_weight: self.line_weight,
        })
    }
}

/// 要求の行番号 `u32` の列を読む。読む前に要求の予算で数える
fn decode_rows(rows: &[u8], cx: &mut Cx<'_>) -> Result<Vec<RowId>, Problem> {
    let count = rows.len() / 4;
    checked(cx, Kind::DecodedBytes, rows.len() as u64)?;
    checked(cx, Kind::Work, count as u64)?;
    let mut ids = Vec::with_capacity(count);
    let mut words = Words(rows);
    while let Some(row) = words.u32() {
        ids.push(RowId(row));
    }
    Ok(ids)
}

/// 行の列を、件数 `u32` と行番号 `u32` の列に写す
fn encode_rows(rows: &[RowId], cx: &mut Cx<'_>) -> Result<Vec<u8>, Problem> {
    let length = rows
        .len()
        .checked_mul(4)
        .and_then(|n| n.checked_add(4))
        .filter(|&n| n <= display_wire::MAX_PACKET_BYTES)
        .ok_or(Problem::Core(Error::Invalid))?;
    checked(cx, Kind::DecodedBytes, length as u64)?;
    checked(cx, Kind::Work, rows.len() as u64)?;
    let count = u32::try_from(rows.len()).map_err(|_| Problem::Core(Error::Invalid))?;
    let mut data = Vec::with_capacity(length);
    data.extend_from_slice(&count.to_le_bytes());
    for row in rows {
        data.extend_from_slice(&row.0.to_le_bytes());
    }
    Ok(data)
}

/// 画層の表の 1 項目の固定部分（値・フラグ・RGB・色の符号・名前の長さ）
const LAYER_FIXED_BYTES: usize = 20;

/// 画層の表を、項目数 `u32` と項目の列に写す。名前は UTF-8 で、4 バイトの境界まで 0 で埋める
fn encode_layers(layers: &[(u32, Layer)], cx: &mut Cx<'_>) -> Result<Vec<u8>, Problem> {
    let mut length = 4_usize;
    for (_, layer) in layers {
        length = layer
            .name
            .len()
            .checked_next_multiple_of(4)
            .and_then(|name| name.checked_add(LAYER_FIXED_BYTES))
            .and_then(|item| length.checked_add(item))
            .filter(|&n| n <= display_wire::MAX_PACKET_BYTES)
            .ok_or(Problem::Core(Error::Invalid))?;
    }
    checked(cx, Kind::DecodedBytes, length as u64)?;
    checked(cx, Kind::Work, layers.len() as u64)?;
    let count = u32::try_from(layers.len()).map_err(|_| Problem::Core(Error::Invalid))?;
    let mut data = Vec::with_capacity(length);
    data.extend_from_slice(&count.to_le_bytes());
    for (value, layer) in layers {
        let mut flags = 0_u32;
        if let Some(state) = layer.state {
            flags |= 1;
            flags |= u32::from(state.visible) << 1;
            flags |= u32::from(state.printable) << 2;
            flags |= u32::from(state.editable) << 3;
            flags |= u32::from(state.frozen) << 4;
        }
        let color = layer.style.and_then(|style| style.color);
        let mut rgb = 0_u32;
        if let Some([r, g, b]) = color.and_then(|color| color.rgb) {
            flags |= 1 << 5;
            rgb = (u32::from(r) << 16) | (u32::from(g) << 8) | u32::from(b);
        }
        let mut code = 0_u32;
        if let Some(found) = color.and_then(|color| color.code) {
            flags |= 1 << 6;
            let system = match found.system {
                ColorSystem::Aci => 1_u32,
                ColorSystem::Sxf => 2,
                ColorSystem::Jww => 3,
            };
            code = (system << 24) | (found.value & 0x00FF_FFFF);
        }
        flags |= u32::from(layer.follows_placement) << 7;
        let name = u32::try_from(layer.name.len()).map_err(|_| Problem::Core(Error::Invalid))?;
        for word in [*value, flags, rgb, code, name] {
            data.extend_from_slice(&word.to_le_bytes());
        }
        data.extend_from_slice(layer.name.as_bytes());
        data.resize(data.len().next_multiple_of(4), 0);
    }
    Ok(data)
}

/// 記号の定義を、定義の数 `u32`・要求した定義の数 `u32`・定義ごとの先頭の行 `u32` と行の数 `u32` の列と、
/// その後ろに全量の表示のパケットを続けて写す
fn encode_symbols(symbols: &Symbols, cx: &mut Cx<'_>) -> Result<Vec<u8>, Problem> {
    let display =
        display_wire::encode(&symbols.display, true, cx).map_err(|error| match error {
            display_wire::Error::Invalid => Problem::Core(Error::Invalid),
            display_wire::Error::Stopped(stop) => {
                cx.report(stop.to_diagnostic());
                Problem::Core(Error::Stopped(stop))
            }
        })?;
    let length = symbols
        .ranges
        .len()
        .checked_mul(8)
        .and_then(|ranges| ranges.checked_add(8 + display.len()))
        .filter(|&n| n <= display_wire::MAX_PACKET_BYTES)
        .ok_or(Problem::Core(Error::Invalid))?;
    checked(cx, Kind::DecodedBytes, length as u64)?;
    checked(cx, Kind::Work, symbols.ranges.len() as u64)?;
    let count = u32::try_from(symbols.ranges.len()).map_err(|_| Problem::Core(Error::Invalid))?;
    let mut data = Vec::with_capacity(length);
    data.extend_from_slice(&symbols.total.to_le_bytes());
    data.extend_from_slice(&count.to_le_bytes());
    for [first, rows] in &symbols.ranges {
        data.extend_from_slice(&first.to_le_bytes());
        data.extend_from_slice(&rows.to_le_bytes());
    }
    data.extend_from_slice(&display);
    Ok(data)
}

/// 文字の 1 項目の固定部分（種類・内容の長さ）
const TEXT_FIXED_BYTES: usize = 8;

/// 行の文字を、件数 `u32` と、種類 `u32`・内容のバイト数 `u32`・4 バイトの境界まで 0 で埋めた UTF-8 の内容の列に写す
fn encode_texts(texts: &[(u32, String)], cx: &mut Cx<'_>) -> Result<Vec<u8>, Problem> {
    let mut length = 4_usize;
    for (_, content) in texts {
        length = content
            .len()
            .checked_next_multiple_of(4)
            .and_then(|text| text.checked_add(TEXT_FIXED_BYTES))
            .and_then(|item| length.checked_add(item))
            .filter(|&n| n <= display_wire::MAX_PACKET_BYTES)
            .ok_or(Problem::Core(Error::Invalid))?;
    }
    checked(cx, Kind::DecodedBytes, length as u64)?;
    checked(cx, Kind::Work, texts.len() as u64)?;
    let word = |value: usize| u32::try_from(value).map_err(|_| Problem::Core(Error::Invalid));
    let mut data = Vec::with_capacity(length);
    data.extend_from_slice(&word(texts.len())?.to_le_bytes());
    for (kind, content) in texts {
        data.extend_from_slice(&kind.to_le_bytes());
        data.extend_from_slice(&word(content.len())?.to_le_bytes());
        data.extend_from_slice(content.as_bytes());
        data.resize(data.len().next_multiple_of(4), 0);
    }
    Ok(data)
}

/// 線種の表の 1 項目の固定部分（値・フラグ・名前の長さ・説明の長さ・列の数）
const LINE_TYPE_FIXED_BYTES: usize = 20;

/// 線種の表を、項目数 `u32` と項目の列に写す。名前と説明は UTF-8 で 4 バイトの境界まで 0 で埋め、列は f64 で続ける
fn encode_line_types(line_types: &[LineType], cx: &mut Cx<'_>) -> Result<Vec<u8>, Problem> {
    let mut length = 4_usize;
    for line_type in line_types {
        let (description, pattern) = line_type.definition.as_ref().map_or((0, 0), |definition| {
            (definition.description.len(), definition.pattern.len())
        });
        length = line_type
            .name
            .len()
            .checked_next_multiple_of(4)
            .and_then(|name| name.checked_add(description.checked_next_multiple_of(4)?))
            .and_then(|text| text.checked_add(pattern.checked_mul(8)?))
            .and_then(|item| item.checked_add(LINE_TYPE_FIXED_BYTES))
            .and_then(|item| length.checked_add(item))
            .filter(|&n| n <= display_wire::MAX_PACKET_BYTES)
            .ok_or(Problem::Core(Error::Invalid))?;
    }
    checked(cx, Kind::DecodedBytes, length as u64)?;
    checked(cx, Kind::Work, line_types.len() as u64)?;
    let count = u32::try_from(line_types.len()).map_err(|_| Problem::Core(Error::Invalid))?;
    let word = |value: usize| u32::try_from(value).map_err(|_| Problem::Core(Error::Invalid));
    let mut data = Vec::with_capacity(length);
    data.extend_from_slice(&count.to_le_bytes());
    for line_type in line_types {
        let definition = line_type.definition.as_ref();
        let mut flags = 0_u32;
        if let Some(definition) = definition {
            flags |= 1;
            flags |= u32::from(definition.basis == PatternBasis::Paper) << 1;
            flags |= u32::from(!definition.marks.is_empty()) << 2;
        }
        let description = definition.map_or("", |definition| definition.description.as_str());
        let pattern = definition.map_or(&[][..], |definition| definition.pattern.as_slice());
        for value in [
            line_type.id.0,
            flags,
            word(line_type.name.len())?,
            word(description.len())?,
            word(pattern.len())?,
        ] {
            data.extend_from_slice(&value.to_le_bytes());
        }
        for text in [line_type.name.as_str(), description] {
            data.extend_from_slice(text.as_bytes());
            data.resize(data.len().next_multiple_of(4), 0);
        }
        for value in pattern {
            data.extend_from_slice(&value.to_le_bytes());
        }
    }
    Ok(data)
}

fn checked(cx: &mut Cx<'_>, kind: Kind, amount: u64) -> Result<(), Problem> {
    cx.check(kind, amount).map_err(|stop| {
        cx.report(stop.to_diagnostic());
        Problem::Core(Error::Stopped(stop))
    })
}

impl Reply {
    fn problem(&mut self, problem: Problem) {
        self.status = match problem {
            Problem::Stale { .. } => Status::Stale,
            Problem::Missing(row) => {
                self.result = u64::from(row.0);
                Status::Missing
            }
            Problem::Core(error) => match error {
                Error::Invalid => Status::Invalid,
                Error::Unsupported => Status::Unsupported,
                Error::Rejected => Status::Rejected,
                Error::Stopped(stop) => {
                    self.stop = Some(stop);
                    Status::stopped(stop)
                }
            },
        };
    }
}

impl Host {
    pub fn handle(&mut self, request: &mut Request) -> Reply {
        let mut reply = Reply::default();
        {
            let mut cx = Cx::new(
                &mut request.budget,
                &mut request.diagnostics,
                &request.cancel,
            );
            if let Some(stop) = request.failure {
                reply.problem(Problem::Core(Error::Stopped(stop)));
            } else if let Err(problem) = self.execute(
                request.header,
                &request.control,
                &request.data,
                &mut reply,
                &mut cx,
            ) {
                reply.problem(problem);
            }
        }
        if let Some(document) = &self.document {
            reply.revision = document.revision().0;
            reply.state = document.state_id();
        }
        reply.used = Kind::ALL.map(|kind| request.budget.used(kind));
        if request.header.command != Command::Diagnostics {
            reply.diagnostics = u32::try_from(request.diagnostics.len()).unwrap_or(u32::MAX);
            self.diagnostics = Some((request.header.id, std::mem::take(&mut request.diagnostics)));
        }
        reply
    }

    fn execute(
        &mut self,
        header: Header,
        control: &Control,
        bytes: &[u8],
        reply: &mut Reply,
        cx: &mut Cx<'_>,
    ) -> Result<(), Problem> {
        if header.command != Command::Close {
            checked(cx, Kind::Work, 0)?;
        }
        let Some(operation) = operation(header.command, bytes) else {
            reply.status = Status::Invalid;
            return Ok(());
        };
        if header.command == Command::Close {
            return Ok(());
        }
        if let Operation::Diagnostics(id) = operation {
            if control.ack != wire::NONE {
                reply.status = Status::Invalid;
                return Ok(());
            }
            let Some((stored_id, diagnostics)) = &self.diagnostics else {
                reply.status = Status::NoDiagnostics;
                return Ok(());
            };
            if *stored_id != id {
                reply.status = Status::NoDiagnostics;
                return Ok(());
            }
            reply.data = diagnostics::encode(diagnostics, cx)?;
            reply.data_kind = 4;
            reply.diagnostics = u32::try_from(diagnostics.len()).unwrap_or(u32::MAX);
            reply.result = id;
            return Ok(());
        }
        if let Operation::Open { threads, bytes } = operation {
            if self.document.is_some() {
                reply.status = Status::AlreadyOpen;
                return Ok(());
            }
            if control.revision != wire::NONE || control.ack != wire::NONE {
                reply.status = Status::Invalid;
                return Ok(());
            }
            let document = Document::open(bytes, threads, cx)?;
            reply.result = u64::from(document.live_count());
            self.document = Some(document);
        } else {
            let Some(document) = &mut self.document else {
                reply.status = Status::NoDocument;
                return Ok(());
            };
            let revision = Revision(control.revision);
            document.require_revision(revision)?;
            // 受領確認は既に受け取った表示に対する操作。編集の成否とは独立する。
            if control.ack != wire::NONE {
                document.confirm(Revision(control.ack));
            }
            match operation {
                Operation::Transform { transform, rows } => {
                    let ids = decode_rows(rows, cx)?;
                    reply.result = document
                        .edit(revision, &ids, &RowEdit::Move(transform), cx)?
                        .len() as u64;
                }
                Operation::Restyle { style, rows } => {
                    let ids = decode_rows(rows, cx)?;
                    let layers = match style.layer {
                        Some(_) => document.layers(cx)?,
                        None => Vec::new(),
                    };
                    let line_types = match style.line_type {
                        Some(Inherit::Own(_)) => document.line_types(cx)?,
                        _ => Vec::new(),
                    };
                    let restyle = style
                        .resolve(&layers, &line_types)
                        .ok_or(Problem::Core(Error::Invalid))?;
                    reply.result = document
                        .edit(revision, &ids, &RowEdit::Restyle(restyle), cx)?
                        .len() as u64;
                }
                Operation::SetText { content, rows } => {
                    let ids = decode_rows(rows, cx)?;
                    checked(cx, Kind::DecodedBytes, content.len() as u64)?;
                    let edit = RowEdit::Text(content.to_owned());
                    reply.result = document.edit(revision, &ids, &edit, cx)?.len() as u64;
                }
                Operation::Remove(rows) => {
                    let ids = decode_rows(rows, cx)?;
                    reply.result =
                        document.edit(revision, &ids, &RowEdit::Remove, cx)?.len() as u64;
                }
                Operation::Copy { transform, rows } => {
                    let ids = decode_rows(rows, cx)?;
                    let before = document.row_count();
                    document.edit(revision, &ids, &RowEdit::Copy(transform), cx)?;
                    // 写しは、当てる前の表の末尾から対象の順に足される（試験で `Applied` と照らす）。
                    // 確定した後なので、ここでは断らない
                    let copies: Vec<RowId> = (before..document.row_count())
                        .filter_map(RowId::from_index)
                        .collect();
                    reply.data = encode_rows(&copies, cx)?;
                    reply.data_kind = 5;
                    reply.result = copies.len() as u64;
                }
                Operation::Draw { template, shape } => {
                    let before = document.row_count();
                    document.draw(revision, template, shape, cx)?;
                    // 足した要素は、当てる前の表の末尾に足される。確定した後なので、ここでは断らない
                    let added: Vec<RowId> = (before..document.row_count())
                        .filter_map(RowId::from_index)
                        .collect();
                    reply.data = encode_rows(&added, cx)?;
                    reply.data_kind = 5;
                    reply.result = added.len() as u64;
                }
                Operation::Target(row) => {
                    let target = document.target(row, cx)?;
                    let length = target
                        .id
                        .len()
                        .checked_add(20)
                        .ok_or(Problem::Core(Error::Invalid))?;
                    checked(cx, Kind::DecodedBytes, length as u64)?;
                    checked(cx, Kind::Work, 1 + target.id.len() as u64 / 1024)?;
                    let mut data = Vec::with_capacity(length);
                    data.extend_from_slice(
                        &u32::try_from(target.id.len())
                            .map_err(|_| Problem::Core(Error::Invalid))?
                            .to_le_bytes(),
                    );
                    data.extend_from_slice(&target.hash.0);
                    data.extend_from_slice(target.id.as_bytes());
                    reply.data = data;
                    reply.data_kind = 3;
                    reply.result = u64::from(row.0);
                }
                Operation::Texts(rows) => {
                    let ids = decode_rows(rows, cx)?;
                    let texts = document.texts(&ids, cx)?;
                    reply.data = encode_texts(&texts, cx)?;
                    reply.data_kind = 9;
                    reply.result = texts.len() as u64;
                }
                Operation::Symbols(indices) => {
                    let indices: Vec<u32> = decode_rows(indices, cx)?
                        .into_iter()
                        .map(|RowId(index)| index)
                        .collect();
                    let symbols = document.symbols(&indices, cx)?;
                    reply.data = encode_symbols(&symbols, cx)?;
                    reply.data_kind = 10;
                    reply.result = symbols.ranges.len() as u64;
                }
                Operation::Select(rect) => {
                    let rows = document.select(rect, cx)?;
                    reply.data = encode_rows(&rows, cx)?;
                    reply.data_kind = 5;
                    reply.result = rows.len() as u64;
                }
                Operation::Empty => match header.command {
                    Command::Undo => reply.result = u64::from(document.undo(revision, cx)?),
                    Command::Redo => reply.result = u64::from(document.redo(revision, cx)?),
                    Command::Save => {
                        reply.data = document.save(cx)?;
                        reply.data_kind = 2;
                        reply.result = reply.data.len() as u64;
                    }
                    Command::Layers => {
                        let layers = document.layers(cx)?;
                        reply.data = encode_layers(&layers, cx)?;
                        reply.data_kind = 6;
                        reply.result = layers.len() as u64;
                    }
                    Command::LineTypes => {
                        let line_types = document.line_types(cx)?;
                        reply.data = encode_line_types(&line_types, cx)?;
                        reply.data_kind = 8;
                        reply.result = line_types.len() as u64;
                    }
                    Command::Order => {
                        let rows = document.order(cx)?;
                        reply.data = encode_rows(&rows, cx)?;
                        reply.data_kind = 7;
                        reply.result = rows.len() as u64;
                    }
                    Command::Display => {}
                    _ => {
                        reply.status = Status::Invalid;
                        return Ok(());
                    }
                },
                Operation::Open { .. } | Operation::Diagnostics(_) => {
                    reply.status = Status::Invalid;
                    return Ok(());
                }
            }
        }
        if (header.flags & wire::WITH_DISPLAY != 0 || header.command == Command::Display)
            && let Some(document) = &mut self.document
        {
            // 編集の成功はこの読み取りの失敗でも保持し、別の状態フィールドで返す。
            match document.display(header.flags & wire::FULL_DISPLAY != 0, cx) {
                Ok(display) => {
                    reply.revision = display.revision.0;
                    reply.base = display.base.map_or(wire::NONE, |revision| revision.0);
                    reply.data = display.bytes;
                    reply.data_kind = 1;
                }
                Err(display_wire::Error::Invalid) => reply.display_status = Status::Invalid,
                Err(display_wire::Error::Stopped(stop)) => {
                    cx.report(stop.to_diagnostic());
                    reply.display_status = Status::stopped(stop);
                    reply.stop = Some(stop);
                }
            }
        }
        Ok(())
    }
}
