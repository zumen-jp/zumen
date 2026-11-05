//! プロファイルの fuzz の target（#517、決定 0138・0262）。受け取ったバイト列をプロファイルとして `zumen` の
//! `Session` に渡し、固定の文書を照らす op（図面の照合の `check`、線形の図の `civil_drawing`、点群と設計面の差の
//! `deviation`）を呼ぶ。プロファイルは、固定の照合のプロファイル（`base.json`）と一緒に渡し、`extends` でたどれる
//! ようにする。照らすプロファイルの識別と工種は、受け取ったプロファイルが書くものを使う。線形の図は、シーンにする。
//!
//! どの要求の予算も、テストキットの境界の呼び出しの fuzz の上限（`fuzz_call_limits`）にする。2 回呼んで、応答の
//! バイト列が同じことを確かめる。

#![no_main]

use libfuzzer_sys::fuzz_target;
use zumen::json::Value;
use zumen_fuzz::{Calls, PLAN_SURFACE, field, list, profile, text};

/// 受け取ったプロファイルが書く識別と版と工種。読めなければ、どれにも当たらない値にする
fn identity(data: &[u8]) -> (String, String, String) {
    let value = zumen::json::parse(data).unwrap_or(Value::Null);
    let get = |pointer: &str, fallback: &str| text(&value, pointer).unwrap_or(fallback).to_string();
    let work = match value.pointer("/rules") {
        Some(Value::Array(rules)) => rules.iter().find_map(|rule| text(rule, "/work")),
        _ => None,
    };
    (
        get("/id", "none"),
        get("/edition", "none"),
        work.unwrap_or("none").to_string(),
    )
}

fuzz_target!(|data: &[u8]| {
    let (id, edition, work) = identity(data);
    let survived = zumen_sdk::testkit::survives_calls(|limits| {
        let mut calls = Calls::new(limits);
        zumen_fuzz::open_fixtures(&mut calls);
        calls.push("profile.json", data);
        calls.call("check", |out| {
            field(out, "doc", "a.dxf");
            list(out, "profiles", &["base.json", "profile.json"]);
            profile(out, &id, &edition);
            field(out, "palette", "palette.json");
        });
        let drawn = calls.call("civil_drawing", |out| {
            field(out, "doc", "road.xml");
            field(out, "to", "plan.dxf");
            list(out, "profiles", &["base.json", "profile.json"]);
            profile(out, &id, &edition);
        });
        if drawn.ok {
            for from in ["sheet/0", "space/0"] {
                calls.call("scene", |out| {
                    field(out, "doc", "plan.dxf");
                    field(out, "from", from);
                });
            }
        }
        calls.call("deviation", |out| {
            field(out, "doc", "a.xyz");
            out.key("surface");
            out.begin_object();
            field(out, "doc", "plan.xml");
            field(out, "id", PLAN_SURFACE);
            out.end_object();
            list(out, "profiles", &["base.json", "profile.json"]);
            profile(out, &id, &edition);
            field(out, "work", &work);
        });
        calls.finish()
    });
    if let Err(message) = survived {
        panic!("{message}");
    }
});
