use std::io::{self, Cursor};

use zumen_base::{Kind, Limit, Limits};
use zumen_drawing::{Affine, Hash};
use zumen_live::display_wire::Packet;

use super::host::{Host, Request};
use super::wire::{self, Command, Header, Reply, Status};
use super::{read_request, wire::Words};

const DXF: &[u8] = b"0\nSECTION\n2\nHEADER\n9\n$INSUNITS\n70\n4\n0\nENDSEC\n\
    0\nSECTION\n2\nENTITIES\n\
    0\nLINE\n5\n10\n8\n0\n10\n0\n20\n0\n11\n1\n21\n0\n\
    0\nLINE\n5\n11\n8\n0\n10\n2\n20\n0\n11\n3\n21\n0\n\
    0\nENDSEC\n0\nEOF\n";

fn request(
    command: Command,
    id: u64,
    revision: u64,
    ack: u64,
    flags: u32,
    limits: Limits,
    data: &[u8],
) -> Request {
    let mut bytes = Vec::new();
    for kind in Kind::ALL {
        let value = match limits.get(kind) {
            Limit::Finite(value) => value,
            Limit::Unlimited => wire::NONE,
        };
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    bytes.extend_from_slice(&revision.to_le_bytes());
    bytes.extend_from_slice(&ack.to_le_bytes());
    bytes.extend_from_slice(data);
    read_request(
        &mut Cursor::new(bytes.clone()),
        Header {
            command,
            id,
            length: bytes.len(),
            flags,
        },
        zumen_base::CancelToken::new(),
    )
    .unwrap()
}

fn call(
    host: &mut Host,
    command: Command,
    id: u64,
    revision: u64,
    ack: u64,
    flags: u32,
    data: &[u8],
) -> Reply {
    host.handle(&mut request(
        command,
        id,
        revision,
        ack,
        flags,
        Limits::default(),
        data,
    ))
}

fn open(host: &mut Host) -> Reply {
    let mut data = 1_u32.to_le_bytes().to_vec();
    data.extend_from_slice(&0_u32.to_le_bytes());
    data.extend_from_slice(DXF);
    let reply = call(host, Command::Open, 1, wire::NONE, wire::NONE, 1, &data);
    assert_eq!(reply.status, Status::Ok);
    assert_eq!(reply.display_status, Status::Ok);
    assert_eq!(reply.revision, 0);
    assert_eq!(reply.data_kind, 1);
    reply
}

fn transform(rows: &[u32], affine: Affine) -> Vec<u8> {
    let mut bytes = (rows.len() as u32).to_le_bytes().to_vec();
    bytes.extend_from_slice(&0_u32.to_le_bytes());
    for value in affine.m {
        bytes.extend_from_slice(&value.to_bits().to_le_bytes());
    }
    for row in rows {
        bytes.extend_from_slice(&row.to_le_bytes());
    }
    bytes
}

fn inspect_packet<T>(bytes: &[u8], run: impl FnOnce(Packet<'_>) -> T) -> T {
    let mut budget = zumen_base::Budget::new(Limits::default());
    let mut diagnostics = zumen_base::Diagnostics::new();
    let cancel = zumen_base::CancelToken::new();
    let packet = Packet::parse(
        bytes,
        &mut zumen_base::Cx::new(&mut budget, &mut diagnostics, &cancel),
    )
    .unwrap();
    run(packet)
}

#[test]
fn old_acknowledgements_cannot_clear_new_edits_or_undo() {
    let mut host = Host::default();
    let initial = open(&mut host);
    assert!(inspect_packet(&initial.data, |packet| packet.full()));
    let data = transform(&[0], Affine::translation(1.0, 0.0));
    let first = call(&mut host, Command::Transform, 2, 0, 0, 1, &data);
    assert_eq!(first.status, Status::Ok);
    assert_eq!(first.revision, 1);
    assert_eq!(first.base, 0);
    let next = call(&mut host, Command::Transform, 3, 1, 0, 1, &data);
    assert_eq!(next.revision, 2);
    let replay = call(&mut host, Command::Display, 4, 2, 1, 0, &[]);
    assert_eq!(replay.data, next.data);
    let clear = call(&mut host, Command::Display, 5, 2, 2, 0, &[]);
    assert_eq!(clear.base, 2);
    assert!(inspect_packet(&clear.data, |packet| packet
        .columns()
        .iter()
        .all(|column| column.blocks().next().is_none())));
    let undone = call(&mut host, Command::Undo, 6, 2, 2, 1, &[]);
    assert_eq!(undone.revision, 3);
    let replay = call(&mut host, Command::Display, 7, 3, 1, 0, &[]);
    assert_eq!(replay.data, undone.data);
}

#[test]
#[expect(clippy::print_stdout, reason = "ネイティブと WASI の比較記録")]
fn committed_edit_survives_a_stopped_display_read() {
    let data = transform(&[0], Affine::translation(2.0, 3.0));
    let mut baseline = Host::default();
    open(&mut baseline);
    let mut normal = request(Command::Transform, 2, 0, 0, 0, Limits::default(), &data);
    let expected = baseline.handle(&mut normal);
    assert_eq!(expected.status, Status::Ok);
    let required = normal.budget.used(Kind::DecodedBytes);
    let mut host = Host::default();
    open(&mut host);
    let before = call(&mut host, Command::Save, 2, 0, wire::NONE, 0, &[]).data;
    let mut limited = request(
        Command::Transform,
        3,
        0,
        0,
        1,
        Limits::default().with(Kind::DecodedBytes, Limit::Finite(required)),
        &data,
    );
    let result = host.handle(&mut limited);
    assert_eq!(result.status, Status::Ok);
    assert_eq!(result.display_status, Status::Exceeded);
    assert_eq!(result.revision, 1);
    assert!(result.data.is_empty());
    let saved = call(&mut host, Command::Save, 4, 1, wire::NONE, 0, &[]).data;
    assert_ne!(saved, before);
    assert_eq!(
        saved,
        call(&mut baseline, Command::Save, 3, 1, wire::NONE, 0, &[]).data
    );
    let display = call(&mut host, Command::Display, 5, 1, 0, 0, &[]);
    assert!(!display.data.is_empty());
    assert_eq!(call(&mut host, Command::Undo, 6, 1, 1, 0, &[]).revision, 2);
    assert_eq!(
        call(&mut host, Command::Save, 7, 2, wire::NONE, 0, &[]).data,
        before
    );
    assert_eq!(
        call(&mut host, Command::Redo, 8, 2, wire::NONE, 0, &[]).revision,
        3
    );
    assert_eq!(
        call(&mut host, Command::Save, 9, 3, wire::NONE, 0, &[]).data,
        saved
    );
    let mut record = Vec::new();
    result.write(limited.header, &mut record).unwrap();
    record.extend_from_slice(&display.data);
    record.extend_from_slice(&saved);
    println!("native-live-commit {:?}", Hash::document(&record));
}

#[test]
fn stale_and_missing_rows_leave_saved_content_unchanged() {
    let mut host = Host::default();
    open(&mut host);
    let before = call(&mut host, Command::Save, 2, 0, wire::NONE, 0, &[]).data;
    let data = transform(&[0, u32::MAX], Affine::translation(1.0, 2.0));
    assert_eq!(
        call(&mut host, Command::Transform, 3, 0, wire::NONE, 1, &data).status,
        Status::Missing
    );
    let stale = call(&mut host, Command::Transform, 4, 9, wire::NONE, 1, &data);
    assert_eq!(stale.status, Status::Stale);
    assert_eq!(stale.revision, 0);
    assert_eq!(
        call(&mut host, Command::Save, 5, 0, wire::NONE, 0, &[]).data,
        before
    );
    let target = call(
        &mut host,
        Command::Target,
        6,
        0,
        wire::NONE,
        0,
        &0_u32.to_le_bytes(),
    );
    assert_eq!(target.status, Status::Ok);
    let mut words = Words(&target.data);
    assert_eq!(words.u32(), Some(11));
    assert!(words.take::<16>().is_some());
    assert_eq!(words.0, b"space/0/h10");
}

#[test]
fn headers_and_replies_have_independent_fixed_width_records() {
    let bytes = [
        b'Z', b'L', b'I', b'P', 1, 0, 6, 0, 7, 0, 0, 0, 0, 0, 0, 0, 72, 0, 0, 0, 0, 0, 0, 0,
    ];
    let header = Header::parse(&bytes).unwrap();
    assert_eq!(header.command, Command::Save);
    assert_eq!(header.id, 7);
    assert_eq!(
        wire::read_header(&mut Cursor::new(&bytes)).unwrap(),
        Some(header)
    );
    for end in 1..24 {
        assert_eq!(
            wire::read_header(&mut Cursor::new(&bytes[..end]))
                .unwrap_err()
                .kind(),
            io::ErrorKind::UnexpectedEof
        );
    }
    assert!(wire::read_header(&mut Cursor::new(&[])).unwrap().is_none());
    for offset in [0, 4, 7, 20, 21, 22, 23] {
        let mut bad = bytes;
        bad[offset] ^= 0x80;
        assert!(Header::parse(&bad).is_none());
    }
    let mut bad = bytes;
    bad[16..20].copy_from_slice(&u32::MAX.to_le_bytes());
    assert!(Header::parse(&bad).is_none());
    let mut output = Vec::new();
    Reply::default().write(header, &mut output).unwrap();
    assert_eq!(output.len(), 160);
    assert_eq!(&output[..8], b"ZLIR\x01\x00\x06\x00");
    assert_eq!(&output[16..20], &136_u32.to_le_bytes());
    assert_eq!(&output[32..48], &[255; 16]);
    assert_eq!(&output[56..64], &[255; 8]);
    assert!(
        header
            .response(zumen_live::display_wire::MAX_PACKET_BYTES + 137)
            .is_err()
    );
}

#[test]
fn zero_operation_budget_stops_without_decoding_the_received_body() {
    let limits = Limits::default().with(Kind::Work, Limit::Finite(0));
    let mut value = request(
        Command::Open,
        1,
        wire::NONE,
        wire::NONE,
        1,
        limits,
        &[1, 0, 0, 0, 0, 0, 0, 0],
    );
    assert!(value.failure.is_some());
    assert_eq!(value.data, [1, 0, 0, 0, 0, 0, 0, 0]);
    assert_eq!(value.budget.used(Kind::DecodedBytes), 0);
    let result = Host::default().handle(&mut value);
    assert_eq!(result.status, Status::Exceeded);
    assert_eq!(result.revision, wire::NONE);
}

#[test]
fn stopped_requests_keep_the_document_and_diagnostics_for_retry() {
    let mut host = Host::default();
    open(&mut host);
    let before = call(&mut host, Command::Save, 2, 0, wire::NONE, 0, &[]).data;
    let data = transform(&[0], Affine::translation(1.0, 2.0));
    let limits = Limits::default().with(Kind::Work, Limit::Finite(0));
    let stopped = host.handle(&mut request(
        Command::Transform,
        3,
        0,
        wire::NONE,
        1,
        limits,
        &data,
    ));
    assert_eq!(stopped.status, Status::Exceeded);
    assert_eq!(stopped.revision, 0);
    assert_eq!(stopped.diagnostics, 1);
    let mut limited = request(
        Command::Diagnostics,
        4,
        0,
        wire::NONE,
        0,
        Limits::default().with(Kind::DecodedBytes, Limit::Finite(256)),
        &3_u64.to_le_bytes(),
    );
    assert_eq!(host.handle(&mut limited).status, Status::Exceeded);
    let diagnostics = call(
        &mut host,
        Command::Diagnostics,
        5,
        0,
        wire::NONE,
        0,
        &3_u64.to_le_bytes(),
    );
    assert_eq!(diagnostics.status, Status::Ok);
    assert_eq!(diagnostics.data_kind, 4);
    assert_eq!(diagnostics.diagnostics, stopped.diagnostics);
    let text = std::str::from_utf8(&diagnostics.data).unwrap();
    assert!(text.contains("budget.work.exceeded"), "{text}");
    let retry = call(
        &mut host,
        Command::Diagnostics,
        6,
        0,
        wire::NONE,
        0,
        &3_u64.to_le_bytes(),
    );
    assert_eq!(retry.data, diagnostics.data);
    assert_eq!(
        call(&mut host, Command::Save, 7, 0, wire::NONE, 0, &[]).data,
        before
    );
    assert_eq!(
        call(
            &mut host,
            Command::Diagnostics,
            8,
            0,
            wire::NONE,
            0,
            &3_u64.to_le_bytes()
        )
        .status,
        Status::NoDiagnostics
    );
    let mut cancelled = request(
        Command::Transform,
        9,
        0,
        wire::NONE,
        1,
        Limits::default(),
        &data,
    );
    cancelled.cancel.cancel();
    assert_eq!(host.handle(&mut cancelled).status, Status::Cancelled);
    assert_eq!(
        call(&mut host, Command::Save, 10, 0, wire::NONE, 0, &[]).data,
        before
    );
    assert_eq!(
        call(&mut host, Command::Transform, 11, 0, 0, 1, &data).status,
        Status::Ok
    );
}
