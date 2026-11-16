//! 組み込み色表を外部実装と照らすため、解ける全符号を出力する。
use std::io::{self, Write};
use zumen_drawing::{ColorSystem, Palette};

fn main() -> io::Result<()> {
    let palette = Palette::cad();
    let mut output = io::BufWriter::new(io::stdout().lock());
    for (name, system) in [
        ("aci", ColorSystem::Aci),
        ("sxf", ColorSystem::Sxf),
        ("jww", ColorSystem::Jww),
    ] {
        for code in 0..=356 {
            if let Some([r, g, b]) = palette.get(system, code) {
                writeln!(output, "{name} {code} #{r:02x}{g:02x}{b:02x}")?;
            }
        }
    }
    output.flush()
}
