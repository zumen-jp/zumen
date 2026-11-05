//! API が読む JSON（要求と色の表）の fuzz の target（#517、決定 0012・0016・0262）。受け取ったバイト列を、色の表と
//! して `zumen` の `Session` に渡して固定の図面をシーンにし、要求として呼ぶ。JSON の配列なら、要素を順に要求として
//! 呼ぶ（`open` から読む op までの並びを試せる）。要求は、固定の図面・照合のプロファイル・色の表・線形・設計面・
//! 点群を開いたセッションで呼ぶ。
//!
//! 要求の予算は、テストキットの API の呼び出しの fuzz の上限（`fuzz_call_limits`）に替える
//! （`zumen_fuzz::Calls::call_value`）。2 回呼んで、応答のバイト列が同じことを確かめる。

#![no_main]

use libfuzzer_sys::fuzz_target;
use zumen::json::Value;
use zumen_fuzz::{Calls, field};

/// 配列から呼ぶ要求の数の上限
const REQUESTS: usize = 16;

fuzz_target!(|data: &[u8]| {
    let survived = zumen_sdk::testkit::survives_calls(|limits| {
        let mut calls = Calls::new(limits);
        zumen_fuzz::open_fixtures(&mut calls);
        calls.push("colors.json", data);
        calls.call("scene", |out| {
            field(out, "doc", "a.dxf");
            field(out, "from", "space/0");
            field(out, "palette", "colors.json");
        });
        match zumen::json::parse(data) {
            Ok(Value::Array(requests)) => {
                for request in requests.iter().take(REQUESTS) {
                    calls.call_value(request);
                }
            }
            Ok(request) => {
                calls.call_value(&request);
            }
            // 読めない要求も、そのまま渡す
            Err(_) => {
                calls.call_bytes(data);
            }
        }
        calls.finish()
    });
    if let Err(message) = survived {
        panic!("{message}");
    }
});
