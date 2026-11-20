use super::*;
use zumen_base::{Budget, CancelToken, Diagnostics, Limit, Limits};

fn context<T>(limits: Limits, run: impl FnOnce(&mut Cx<'_>) -> T) -> (T, [u64; Kind::ALL.len()]) {
    let mut budget = Budget::new(limits);
    let mut diagnostics = Diagnostics::new();
    let cancel = CancelToken::new();
    let result = run(&mut Cx::new(&mut budget, &mut diagnostics, &cancel));
    (result, Kind::ALL.map(|kind| budget.used(kind)))
}

fn append(display: &mut Display, i: u32) {
    let f = f32::from_bits(0x3f80_0000 + i);
    display.segments.push([f; 8]);
    display.segment_owner.push(i);
    display.segment_style.push([i, !i]);
    display.arcs.push([f; 7]);
    display.arc_owner.push(i);
    display.arc_style.push([i, !i]);
    display.texts.push([f; 8]);
    display.text_owner.push(i);
    display.text_style.push([i, !i]);
    display.instances.push([f; 9]);
    display.instance_ref.push([i, 2, 3]);
    display.instance_owner.push(i);
    display.instance_style.push([i, !i]);
}

fn fixture(count: u32) -> Display {
    let mut display = Display::default();
    for i in 0..count {
        append(&mut display, i);
    }
    display.clear_dirty();
    display
}

fn encoded(display: &Display, full: bool) -> Vec<u8> {
    context(Limits::default(), |cx| encode(display, full, cx))
        .0
        .unwrap()
}

fn mirror(bytes: &[u8], columns: &mut [Vec<u8>; 13]) {
    context(Limits::default(), |cx| {
        let packet = Packet::parse(bytes, cx).unwrap();
        for (to, from) in columns.iter_mut().zip(packet.columns()) {
            to.resize((from.rows() * from.stride()) as usize, 0);
            for block in from.blocks() {
                let start = (block.start * from.stride()) as usize;
                to[start..start + block.bytes.len()].copy_from_slice(block.bytes);
            }
        }
    });
}

#[test]
fn packet_delta_reconstructs_all_columns_across_block_page_growth_and_shrink() {
    let mut display = fixture(1030);
    let mut received = std::array::from_fn(|_| Vec::new());
    mirror(&encoded(&display, true), &mut received);
    for row in [0, 63, 64, 1023, 1024, 1029] {
        *display.segments.get_mut(row).unwrap() = [row as f32; 8];
        display.dirty.segments.mark_one(row);
        *display.instance_ref.get_mut(row).unwrap() = [99, 4, 5];
        display.dirty.instance_ref.mark_one(row);
        *display.text_owner.get_mut(row).unwrap() = u32::MAX;
        display.dirty.text_owner.mark_one(row);
    }
    let delta = encoded(&display, false);
    assert!(delta.len() < encoded(&display, true).len() / 4);
    let before = format!("{:?}", display.dirty);
    assert_eq!(delta, encoded(&display, false));
    assert_eq!(before, format!("{:?}", display.dirty));
    context(Limits::default(), |cx| {
        let packet = Packet::parse(&delta, cx).unwrap();
        assert!(!packet.full());
        assert_eq!(
            packet.columns()[0]
                .blocks()
                .map(|b| (b.start, b.rows))
                .collect::<Vec<_>>(),
            [(0, 64), (64, 64), (960, 64), (1024, 6)]
        );
        assert_eq!(packet.columns()[1].blocks().count(), 0);
    });
    mirror(&delta, &mut received);
    let mut expected = std::array::from_fn(|_| Vec::new());
    mirror(&encoded(&display, true), &mut expected);
    assert_eq!(received, expected);
    display.clear_dirty();
    assert_eq!(encoded(&display, false).len(), HEADER + 13 * COLUMN_HEADER);

    append(&mut display, 1030);
    display.mark_all_dirty();
    mirror(&encoded(&display, false), &mut received);
    mirror(&encoded(&display, true), &mut expected);
    assert_eq!(received, expected);
    display.clear_dirty();
    display.segments.truncate(1);
    display.segment_owner.truncate(1);
    display.segment_style.truncate(1);
    mirror(&encoded(&display, false), &mut received);
    mirror(&encoded(&display, true), &mut expected);
    assert_eq!(received, expected);
    assert_eq!(received[0].len(), 32);
    assert!(display.has_dirty());
}

#[test]
fn packet_header_and_numeric_bits_are_little_endian_and_fixed() {
    let mut display = fixture(1);
    let bits = [
        0,
        0x8000_0000,
        0x3f80_0000,
        0xbf80_0000,
        0x7f80_0000,
        0xff80_0000,
        0x7fc0_1234,
        1,
    ];
    *display.segments.get_mut(0).unwrap() = bits.map(f32::from_bits);
    let bytes = encoded(&display, true);
    assert_eq!(&bytes[..8], b"ZLDP\x01\0\x01\0");
    assert_eq!(
        u32::from_le_bytes(bytes[8..12].try_into().unwrap()),
        bytes.len() as u32
    );
    assert_eq!(
        &bytes[12..40],
        &[
            13, 0, 0, 0, 0, 0, 0, 0, 8, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0
        ]
    );
    let expected: Vec<u8> = bits.into_iter().flat_map(u32::to_le_bytes).collect();
    assert_eq!(&bytes[40..72], expected);
    context(Limits::default(), |cx| {
        let packet = Packet::parse(&bytes, cx).unwrap();
        assert!(packet.full());
        assert_eq!(packet.columns().map(|c| c.kind()), COLUMNS);
        assert_eq!(packet.columns()[0].blocks().next().unwrap().bytes, expected);
    });
}

#[test]
fn packet_rejects_truncation_bad_headers_lengths_and_overlapping_blocks() {
    let bytes = encoded(&fixture(1), true);
    for end in 0..bytes.len() {
        assert_eq!(
            context(Limits::default(), |cx| Packet::parse(&bytes[..end], cx)
                .map(|_| ()))
            .0,
            Err(Error::Invalid),
            "truncated at {end}"
        );
    }
    for offset in [0, 4, 6, 8, 12, 16, 20, 24, 28, 32, 36] {
        let mut bad = bytes.clone();
        bad[offset] ^= 2;
        assert_eq!(
            context(Limits::default(), |cx| Packet::parse(&bad, cx).map(|_| ())).0,
            Err(Error::Invalid),
            "header at {offset}"
        );
    }
    let mut bad = encoded(&fixture(130), true);
    let second = HEADER + COLUMN_HEADER + BLOCK_HEADER + 64 * 32;
    bad[second..second + 4].copy_from_slice(&0u32.to_le_bytes());
    assert_eq!(
        context(Limits::default(), |cx| Packet::parse(&bad, cx).map(|_| ())).0,
        Err(Error::Invalid)
    );
    let mut invalid = fixture(1);
    invalid.arc_owner.push(2);
    assert_eq!(
        context(Limits::default(), |cx| encode(&invalid, true, cx)).0,
        Err(Error::Invalid)
    );
    let empty = encoded(&Display::default(), true);
    assert_eq!(empty.len(), HEADER + 13 * COLUMN_HEADER);
    context(Limits::default(), |cx| {
        assert!(Packet::parse(&empty, cx).is_ok());
    });
}

#[test]
fn packet_budget_boundaries_and_cancellation_keep_retry_bytes() {
    let mut display = fixture(130);
    display.mark_all_dirty();
    for full in [false, true] {
        let (result, used) = context(Limits::default(), |cx| encode(&display, full, cx));
        let bytes = result.unwrap();
        for kind in [Kind::DecodedBytes, Kind::Records, Kind::Work] {
            let amount = used[Kind::ALL.iter().position(|v| *v == kind).unwrap()];
            let limits = Limits::default().with(kind, Limit::Finite(amount - 1));
            assert!(matches!(
                context(limits, |cx| encode(&display, full, cx)).0,
                Err(Error::Stopped(Stop::Exceeded(_)))
            ));
            assert_eq!(encoded(&display, full), bytes);
            assert!(
                context(Limits::default().with(kind, Limit::Finite(amount)), |cx| {
                    encode(&display, full, cx)
                })
                .0
                .is_ok()
            );
        }
        let (_, read_used) = context(Limits::default(), |cx| {
            Packet::parse(&bytes, cx).map(|_| ())
        });
        for kind in [Kind::InputBytes, Kind::Records, Kind::Work] {
            let amount = read_used[Kind::ALL.iter().position(|v| *v == kind).unwrap()];
            assert!(matches!(
                context(
                    Limits::default().with(kind, Limit::Finite(amount - 1)),
                    |cx| Packet::parse(&bytes, cx).map(|_| ())
                )
                .0,
                Err(Error::Stopped(Stop::Exceeded(_)))
            ));
        }
        let mut budget = Budget::default();
        let mut diagnostics = Diagnostics::new();
        let cancel = CancelToken::new();
        cancel.cancel();
        let mut cx = Cx::new(&mut budget, &mut diagnostics, &cancel);
        assert_eq!(
            encode(&display, full, &mut cx),
            Err(Error::Stopped(Stop::Cancelled))
        );
        assert_eq!(
            Packet::parse(&bytes, &mut cx).map(|_| ()),
            Err(Error::Stopped(Stop::Cancelled))
        );
    }
}

#[test]
#[allow(clippy::print_stdout)]
fn packet_native_wasm_contract_record() {
    let mut display = fixture(1);
    display.mark_all_dirty();
    let (result, used) = context(Limits::default(), |cx| encode(&display, false, cx));
    let bytes = result.unwrap();
    let (_, read_used) = context(Limits::default(), |cx| Packet::parse(&bytes, cx).unwrap());
    println!("display-wire {bytes:?} encode={used:?} read={read_used:?}");
}
