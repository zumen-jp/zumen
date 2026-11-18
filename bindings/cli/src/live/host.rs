//! 1 子プロセスの 1 文書を、要求順に処理する。

use zumen_base::{Budget, CancelToken, Cx, Diagnostics, Error, Kind, Stop};
use zumen_drawing::{Affine, ColorSystem, Layer};
use zumen_live::{Revision, RowId, display_wire};

use super::diagnostics;
use super::document::{Document, Problem};
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
    Target(RowId),
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
        Command::Transform => {
            let count = input.u32()? as usize;
            if count > wire::MAX_ROWS || input.u32()? != 0 {
                return None;
            }
            let mut m = [0_f64; 6];
            for value in &mut m {
                *value = input.f64()?;
            }
            (input.0.len() == count.checked_mul(4)?).then_some(Operation::Transform {
                transform: Affine { m },
                rows: input.0,
            })
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
                    let count = rows.len() / 4;
                    checked(cx, Kind::DecodedBytes, rows.len() as u64)?;
                    checked(cx, Kind::Work, count as u64)?;
                    let mut ids = Vec::with_capacity(count);
                    let mut words = Words(rows);
                    while let Some(row) = words.u32() {
                        ids.push(RowId(row));
                    }
                    reply.result = document.transform(revision, &ids, transform, cx)?.len() as u64;
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
                Operation::Select(rect) => {
                    let rows = document.select(rect, cx)?;
                    let length = rows
                        .len()
                        .checked_mul(4)
                        .and_then(|n| n.checked_add(4))
                        .filter(|&n| n <= display_wire::MAX_PACKET_BYTES)
                        .ok_or(Problem::Core(Error::Invalid))?;
                    checked(cx, Kind::DecodedBytes, length as u64)?;
                    checked(cx, Kind::Work, rows.len() as u64)?;
                    let mut data = Vec::with_capacity(length);
                    let count =
                        u32::try_from(rows.len()).map_err(|_| Problem::Core(Error::Invalid))?;
                    data.extend_from_slice(&count.to_le_bytes());
                    for row in rows {
                        data.extend_from_slice(&row.0.to_le_bytes());
                    }
                    reply.data = data;
                    reply.data_kind = 5;
                    reply.result = u64::from(count);
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
