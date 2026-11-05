//! レビューの文書の fuzz の target（#517、決定 0218・0262）。受け取ったバイト列をレビューの文書として `zumen` の
//! `Session` に渡し、固定の図面の版へ対象を移す（`review_carry`）。移した文書と受け取った文書に、照合の指摘を
//! 取り込む（`review_adopt`。対象の版が古い文書には、取り込まずに断る）。移す対象の ID は、受け取った文書の最初の
//! 対象のものを使う。
//!
//! どの要求の予算も、テストキットの境界の呼び出しの fuzz の上限（`fuzz_call_limits`）にする。2 回呼んで、応答の
//! バイト列が同じことを確かめる。

#![no_main]

use libfuzzer_sys::fuzz_target;
use zumen::json::Value;
use zumen_fuzz::{Calls, field, list, profile, text};

fuzz_target!(|data: &[u8]| {
    let value = zumen::json::parse(data).unwrap_or(Value::Null);
    let target = text(&value, "/targets/0/id").unwrap_or("plan").to_string();
    let survived = zumen_sdk::testkit::survives_calls(|limits| {
        let mut calls = Calls::new(limits);
        zumen_fuzz::open_drawing(&mut calls);
        zumen_fuzz::push_profiles(&mut calls);
        calls.push("review.json", data);
        let carried = calls.call("review_carry", |out| {
            field(out, "doc", "a.dxf");
            field(out, "review", "review.json");
            field(out, "target", &target);
        });
        let mut reviews = vec!["review.json"];
        if let Some(file) = carried.blob("file") {
            calls.push("carried.json", file);
            reviews.push("carried.json");
        }
        for review in reviews {
            calls.call("review_adopt", |out| {
                field(out, "doc", "a.dxf");
                list(out, "profiles", &["base.json", "office.json"]);
                profile(out, "office", "1");
                field(out, "palette", "palette.json");
                field(out, "review", review);
                field(out, "target", &target);
                out.key("findings");
                out.begin_array();
                out.uint(0);
                out.uint(1);
                out.end_array();
            });
        }
        calls.finish()
    });
    if let Err(message) = survived {
        panic!("{message}");
    }
});
