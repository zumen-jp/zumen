//! Shift_JIS の全1バイトと、全 lead/trail の組を、SDK・JS とネイティブ・WASI で比べる（#824）。

use std::io::{self, Write};
use zumen_base::{Budget, CancelToken, Diagnostics, Kind};
use zumen_sdk::{Cx, Signature};

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn main() -> io::Result<()> {
    let mut out = io::BufWriter::new(io::stdout().lock());
    let singles = (0..=255).map(|byte| vec![byte]);
    let pairs = (0x81..=0x9f)
        .chain(0xe0..=0xfc)
        .flat_map(|lead| (0..=255).map(move |trail| vec![lead, trail]));
    for raw in singles.chain(pairs) {
        let mut source = b"<?xml version='1.0' encoding='Shift_JIS'?><n".to_vec();
        source.extend_from_slice(&raw);
        source.extend_from_slice(b"/>");
        let expected = encoding_rs::SHIFT_JIS
            .decode_without_bom_handling_and_without_replacement(&raw)
            .map(|text| format!("n{text}"))
            .unwrap_or_else(|| String::from("invalid"));
        // この検査用プロセスだけで使う、小さな期待名。製品の署名には割り当てを足さない。
        let expected = Box::leak(expected.into_boxed_str());
        let signature = Signature::XmlRoot {
            name: expected.as_bytes(),
            namespace: None,
        }
        .matches(&source);
        let mut budget = Budget::default();
        let mut diag = Diagnostics::new();
        let cancel = CancelToken::new();
        let document = zumen_xml::read(&source, &mut Cx::new(&mut budget, &mut diag, &cancel));
        let used: Vec<_> = Kind::ALL.iter().map(|kind| budget.used(*kind)).collect();
        writeln!(
            out,
            "{} {} {} {} {diag:?} {used:?}",
            hex(&raw),
            hex(expected.as_bytes()),
            u8::from(signature),
            document.is_ok(),
        )?;
    }
    Ok(())
}
