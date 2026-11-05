//! PDF の fuzz の target（#398・#501・#506）。読んで図面モデルに移すまでが、小さい予算の中で止まり、2 回で同じ
//! 結果になることを確かめる。ほかの図面の target と違い、移した図面（図面か止まった理由と、診断）も 2 回で同じに
//! なることを確かめる。ページは hayro-interpret が解釈するので、その中の順に頼らないことも見るため。2 回の結果は、
//! Debug の表記で比べる（浮動小数の NaN は、自分と等しくないので `==` では比べられない）。

#![no_main]

use libfuzzer_sys::fuzz_target;
use zumen_base::{Budget, CancelToken, Cx, Diagnostic, Diagnostics};
use zumen_drawing::Drawing2D;
use zumen_pdf::Pdf;
use zumen_sdk::{Codec, Error, Lift};

/// 読んで図面モデルに移した結果と診断。読めなければ `None`
fn lifted(data: &[u8]) -> Option<(Result<Drawing2D, Error>, Vec<Diagnostic>)> {
    let mut budget = Budget::new(zumen_sdk::testkit::fuzz_limits());
    let mut diag = Diagnostics::new();
    let cancel = CancelToken::new();
    let mut cx = Cx::new(&mut budget, &mut diag, &cancel);
    let file = Pdf::read(data, &mut cx).ok()?;
    let drawing = file.lift(&mut cx);
    Some((drawing, diag.into_vec()))
}

fuzz_target!(|data: &[u8]| {
    if let Err(message) = zumen_sdk::testkit::survives_lift::<Pdf, Drawing2D>(data) {
        panic!("{message}");
    }
    if format!("{:?}", lifted(data)) != format!("{:?}", lifted(data)) {
        panic!("同じバイト列を 2 回図面モデルに移すと、移した図面が違う");
    }
});
