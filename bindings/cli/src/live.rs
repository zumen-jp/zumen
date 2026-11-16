//! 1 文書を常駐させる、固定幅バイナリの標準入出力。

mod diagnostics;
mod document;
mod host;
mod wire;

use std::io::{self, Read, Write};
use std::sync::{Arc, Mutex, PoisonError, mpsc};

use zumen_base::{Budget, CancelToken, Cx, Diagnostics, Kind, Limits};

use host::{Host, Request};
use wire::{Command, Control, Header};

type Active = Arc<Mutex<Option<(u64, CancelToken)>>>;

enum Event {
    Request(Box<Request>),
    End(io::Result<()>),
}

pub(super) fn serve() -> Result<(), String> {
    run(io::stdin(), &mut io::stdout()).map_err(|error| format!("文書 IPC を続けられない: {error}"))
}

fn cancel_active(active: &Active) {
    if let Some((_, cancel)) = active
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .as_ref()
    {
        cancel.cancel();
    }
}

fn run(input: impl Read + Send + 'static, out: &mut impl Write) -> io::Result<()> {
    let (sender, receiver) = mpsc::sync_channel(1);
    let active: Active = Arc::new(Mutex::new(None));
    let reader_active = Arc::clone(&active);
    std::thread::Builder::new()
        .name("zumen-live-input".to_owned())
        .spawn(move || {
            let result = receive(input, &sender, &reader_active);
            cancel_active(&reader_active);
            let _ = sender.send(Event::End(result));
        })?;
    let mut host = Host::default();
    while let Ok(event) = receiver.recv() {
        match event {
            Event::Request(mut request) => {
                let reply = host.handle(&mut request);
                {
                    let mut current = active.lock().unwrap_or_else(PoisonError::into_inner);
                    if current
                        .as_ref()
                        .is_some_and(|(id, _)| *id == request.header.id)
                    {
                        *current = None;
                    }
                }
                if let Err(error) = reply.write(request.header, out) {
                    cancel_active(&active);
                    return Err(error);
                }
                if request.header.command == Command::Close {
                    return Ok(());
                }
            }
            Event::End(result) => return result,
        }
    }
    Ok(())
}

fn receive(
    mut input: impl Read,
    sender: &mpsc::SyncSender<Event>,
    active: &Active,
) -> io::Result<()> {
    let mut last_id = 0;
    while let Some(header) = wire::read_header(&mut input)? {
        if header.command == Command::Cancel {
            if let Some((id, cancel)) = active
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .as_ref()
                && *id == header.id
            {
                cancel.cancel();
            }
            continue;
        }
        if header.id <= last_id {
            return Err(wire::invalid());
        }
        let cancel = CancelToken::new();
        {
            let mut current = active.lock().unwrap_or_else(PoisonError::into_inner);
            if current.is_some() {
                return Err(wire::invalid());
            }
            *current = Some((header.id, cancel.clone()));
        }
        last_id = header.id;
        let request = read_request(&mut input, header, cancel)?;
        let final_request = header.command == Command::Close;
        if sender.send(Event::Request(Box::new(request))).is_err() {
            return Ok(());
        }
        if final_request {
            return Ok(());
        }
    }
    Ok(())
}

fn read_request(input: &mut impl Read, header: Header, cancel: CancelToken) -> io::Result<Request> {
    if header.command == Command::Close {
        return Ok(Request {
            header,
            control: Control {
                limits: Limits::default(),
                revision: wire::NONE,
                ack: wire::NONE,
            },
            data: Vec::new(),
            budget: Budget::new(Limits::default()),
            diagnostics: Diagnostics::new(),
            cancel,
            failure: None,
        });
    }
    let mut control = [0_u8; wire::CONTROL_BYTES];
    input.read_exact(&mut control)?;
    let control = Control::parse(&control).ok_or_else(wire::invalid)?;
    let mut budget = Budget::new(control.limits);
    let mut diagnostics = Diagnostics::new();
    let mut cx = Cx::new(&mut budget, &mut diagnostics, &cancel);
    let length = header
        .length
        .checked_sub(wire::CONTROL_BYTES)
        .ok_or_else(wire::invalid)?;
    // Session の push と同じく、受信領域には操作の Cx と別の通信上限を設ける。
    // Header がコマンド別の最大長を確かめてから確保する。操作が停止しても枠を読み終え、
    // 常駐文書と次の要求を失わない。本文の複製はせず、そのままコアへ貸す。
    let mut data = vec![0_u8; length];
    input.read_exact(&mut data)?;
    let input_bytes = if header.command == Command::Open {
        wire::HEADER_BYTES + wire::CONTROL_BYTES + 8
    } else {
        wire::HEADER_BYTES + header.length
    };
    let preflight = cx
        .check(Kind::Work, 1)
        .and_then(|()| cx.check(Kind::InputBytes, input_bytes as u64))
        .and_then(|()| cx.check(Kind::DecodedBytes, 256));
    if let Err(stop) = preflight {
        cx.report(stop.to_diagnostic());
        return Ok(Request {
            header,
            control,
            data,
            budget,
            diagnostics,
            cancel,
            failure: Some(stop),
        });
    }
    Ok(Request {
        header,
        control,
        data,
        budget,
        diagnostics,
        cancel,
        failure: None,
    })
}

#[cfg(test)]
mod tests;
