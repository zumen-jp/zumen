//! 検証キットの lift 結果から SAF 拡張を取り出す。製品の実装には使わない（#494）。

use std::fmt::Write as _;
use std::io::{Read, Write};

use zumen_base::{Budget, CancelToken, Cx, Diagnostics};
use zumen_drawing::{FieldValue, Target, binary};

fn hex(text: &str) -> String {
    let mut out = String::with_capacity(text.len().saturating_mul(2));
    for byte in text.as_bytes() {
        let _ = write!(out, "{byte:02x}");
    }
    out
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args_os().skip(1);
    let path = args.next().ok_or("lift の結果を指定してください")?;
    if args.next().is_some() {
        return Err("引数は 1 つです".into());
    }
    const MAX_INPUT: usize = 4 * 1024 * 1024;
    let mut bytes = Vec::new();
    std::fs::File::open(path)?
        .take((MAX_INPUT + 1) as u64)
        .read_to_end(&mut bytes)?;
    if bytes.len() > MAX_INPUT {
        return Err("入力は 4 MiB 以下です".into());
    }
    let mut budget = Budget::default();
    let mut diagnostics = Diagnostics::new();
    let cancel = CancelToken::new();
    let (_, drawing) = binary::read(&bytes, &mut Cx::new(&mut budget, &mut diagnostics, &cancel))
        .map_err(|_| "図面モデルの読みが失敗しました")?;
    if !diagnostics.is_empty() {
        return Err("図面モデルの読みに診断があります".into());
    }
    let mut out = std::io::stdout().lock();
    writeln!(out, "SAF\t1")?;
    let mut count = 0;
    for extension in &drawing.extensions {
        if extension.namespace != "sxf.saf" {
            continue;
        }
        let Target::Element(id) = &extension.target else {
            return Err("SAF が要素以外に結ばれています".into());
        };
        writeln!(
            out,
            "A\t{}\t{}\t{}\t{}",
            hex(id),
            hex(&extension.name),
            hex(extension.version.as_deref().unwrap_or("")),
            extension.is_current(&drawing)
        )?;
        for field in &extension.fields {
            let FieldValue::Text(text) = &field.value else {
                return Err("SAF の値が文字列以外です".into());
            };
            writeln!(out, "F\t{}\t{}", hex(&field.key), hex(text))?;
        }
        count += 1;
    }
    writeln!(out, "END\t{count}")?;
    Ok(())
}

fn main() {
    if run().is_err() {
        // 入力の絶対パスや内容を、公開する検証ログへ書かない。
        let _ = writeln!(std::io::stderr(), "SAF の検証用読取が失敗しました");
        std::process::exit(1);
    }
}
