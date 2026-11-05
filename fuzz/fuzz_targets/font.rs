//! シーンの文字のフォントの fuzz の target（#517、決定 0180・0186・0188・0262）。受け取ったバイト列を
//! TrueType・OpenType（CFF・CFF2 を含む）のフォントとして `zumen` の `Session` に渡し、固定の図面の文字を、その
//! フォントの字形でシーンにする。コレクション（TTC）の 2 つ目と、文字の高さを字形の高さで合わせる指定も試す。
//!
//! どの要求の予算も、テストキットの fuzz の上限にする。2 回呼んで、応答のバイト列が同じことを確かめる。

#![no_main]

use libfuzzer_sys::fuzz_target;
use zumen_fuzz::{Calls, field};

/// フォントを指して、図面の空間をシーンにする。`index` はコレクションの中の番号、`height` は DXF の TEXT の高さ
/// （グループコード 40）を合わせる字形の高さ（`schema/messages.json` の `scene_request` の `heights`）
fn scene(calls: &mut Calls, index: u64, height: Option<&str>) {
    calls.call("scene", |out| {
        field(out, "doc", "a.dxf");
        field(out, "from", "space/0");
        out.key("tolerance");
        out.float(0.01);
        out.key("fonts");
        out.begin_array();
        out.begin_object();
        field(out, "input", "font");
        out.key("index");
        out.uint(index);
        out.end_object();
        out.end_array();
        if let Some(height) = height {
            out.key("heights");
            out.begin_object();
            field(out, "dxf.TEXT.40", height);
            out.end_object();
        }
    });
}

fuzz_target!(|data: &[u8]| {
    let survived = zumen_sdk::testkit::survives_calls(|limits| {
        let mut calls = Calls::new(limits);
        zumen_fuzz::open_drawing(&mut calls);
        calls.push("font", data);
        scene(&mut calls, 0, None);
        scene(&mut calls, 0, Some("frame"));
        scene(&mut calls, 1, Some("cap_height"));
        calls.finish()
    });
    if let Err(message) = survived {
        panic!("{message}");
    }
});
