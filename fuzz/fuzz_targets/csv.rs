//! CSV の fuzz の target（#501）。文字コードを見分けて `,` で区切る読み方と、Shift_JIS としてタブで区切る読み方の
//! 読み取りが、小さい予算の中で止まり、2 回で同じ結果になることを確かめる。

#![no_main]

use libfuzzer_sys::fuzz_target;
use zumen_sheet::Encoding;

fuzz_target!(|data: &[u8]| {
    let read = zumen_sdk::testkit::survives_with(data, |src, cx| {
        zumen_sheet::read_csv(src, None, ',', cx)?;
        zumen_sheet::read_csv(src, Some(Encoding::ShiftJis), '\t', cx)
    });
    if let Err(message) = read {
        panic!("{message}");
    }
});
