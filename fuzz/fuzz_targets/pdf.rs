//! PDF の読み取りと、図面モデルへの移しの fuzz の target（#506）。小さい予算の中で止まり、2 回読んで同じ結果に
//! なることと、2 回図面モデルに移して同じ結果（図面か止まった理由と、診断）になることを確かめる。2 回の結果は、
//! Debug の表記で比べる（浮動小数の NaN は、自分と等しくないので `==` では比べられない）。

#![no_main]

use libfuzzer_sys::fuzz_target;
use zumen_base::{Budget, CancelToken, Cx, Diagnostic, Diagnostics, Kind, Limit, Limits};
use zumen_drawing::Drawing2D;
use zumen_pdf::Pdf;
use zumen_sdk::{Codec, Error, Lift};

/// 移しの上限。テストキットの `survives` と同じく、どの入力でもすぐに止まるようにする
fn limits() -> Limits {
    Limits::default()
        .with(Kind::DecodedBytes, Limit::Finite(64 << 20))
        .with(Kind::Records, Limit::Finite(1 << 20))
        .with(Kind::Elements, Limit::Finite(1 << 20))
        .with(Kind::Segments, Limit::Finite(1 << 20))
        .with(Kind::Work, Limit::Finite(1 << 24))
}

/// 読んで図面モデルに移した結果と診断。読めなければ `None`
fn lifted(data: &[u8]) -> Option<(Result<Drawing2D, Error>, Vec<Diagnostic>)> {
    let mut budget = Budget::new(limits());
    let mut diag = Diagnostics::new();
    let cancel = CancelToken::new();
    let mut cx = Cx::new(&mut budget, &mut diag, &cancel);
    let file = Pdf::read(data, &mut cx).ok()?;
    let drawing = file.lift(&mut cx);
    Some((drawing, diag.into_vec()))
}

fuzz_target!(|data: &[u8]| {
    if let Err(message) = zumen_sdk::testkit::survives::<Pdf>(data) {
        panic!("{message}");
    }
    if format!("{:?}", lifted(data)) != format!("{:?}", lifted(data)) {
        panic!("同じバイト列を 2 回図面モデルに移すと、結果が違う");
    }
});
