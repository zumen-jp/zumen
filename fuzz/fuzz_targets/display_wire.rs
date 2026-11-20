//! 表示差分のバイト列・範囲・予算停止と、同じ入力の再現を確かめる。

#![no_main]

use libfuzzer_sys::fuzz_target;
use zumen_base::{Budget, CancelToken, Cx, Diagnostics, Kind, Limit, Limits};
use zumen_live::Display;
use zumen_live::display_wire::{Error, Packet, encode};

fn inspect(data: &[u8], cx: &mut Cx<'_>) -> Result<u64, Error> {
    let packet = Packet::parse(data, cx)?;
    let mut checksum = u64::from(packet.full());
    for column in packet.columns() {
        checksum = checksum.wrapping_add(u64::from(column.rows()));
        for block in column.blocks() {
            cx.check(Kind::Work, block.bytes.len() as u64)?;
            for byte in block.bytes {
                checksum = checksum.wrapping_mul(257).wrapping_add(u64::from(*byte));
            }
        }
    }
    Ok(checksum)
}

fn run(data: &[u8], limits: Limits) -> Vec<Vec<u8>> {
    let mut budget = Budget::new(limits);
    let mut diagnostics = Diagnostics::new();
    let cancel = CancelToken::new();
    let result = inspect(data, &mut Cx::new(&mut budget, &mut diagnostics, &cancel));
    let mut record =
        vec![format!("{result:?} {:?}", Kind::ALL.map(|k| budget.used(k))).into_bytes()];

    // 最大 256 行の合成。任意の f32 のビット列と、持ち主・スタイルを保つ。
    let mut display = Display::default();
    for (i, chunk) in data.as_chunks::<4>().0.iter().take(256).enumerate() {
        let bits = u32::from_le_bytes(*chunk);
        display.segments.push([f32::from_bits(bits); 8]);
        display.segment_owner.push(i as u32);
        display.segment_style.push([bits, !bits]);
    }
    display.clear_dirty();
    if let Some(&first) = data.first() {
        display.dirty.segments.mark_one(usize::from(first));
    }
    for full in [false, true] {
        let mut budget = Budget::new(limits);
        let mut diagnostics = Diagnostics::new();
        let result = encode(
            &display,
            full,
            &mut Cx::new(&mut budget, &mut diagnostics, &cancel),
        );
        match result {
            Ok(bytes) => {
                let mut read_budget = Budget::new(zumen_sdk::testkit::fuzz_call_limits());
                let mut cx = Cx::new(&mut read_budget, &mut diagnostics, &cancel);
                assert!(inspect(&bytes, &mut cx).is_ok());
                record.push(bytes);
            }
            Err(error) => record.push(format!("{error:?}").into_bytes()),
        }
        record.push(format!("{:?}", Kind::ALL.map(|k| budget.used(k))).into_bytes());
    }
    record
}

fuzz_target!(|data: &[u8]| {
    let result = zumen_sdk::testkit::survives_calls(|limits| {
        let limits = limits
            .with(Kind::DecodedBytes, Limit::Finite(4 << 20))
            .with(Kind::Work, Limit::Finite(1 << 18));
        let mut record = run(data, limits);
        let small = u64::from(data.first().copied().unwrap_or(0));
        for kind in [
            Kind::InputBytes,
            Kind::DecodedBytes,
            Kind::Records,
            Kind::Work,
        ] {
            record.extend(run(data, limits.with(kind, Limit::Finite(small))));
        }
        record
    });
    assert!(result.is_ok(), "同じ入力と予算で表示差分の結果が変わる");
});
