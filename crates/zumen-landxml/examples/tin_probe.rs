//! 公開 J-LandXML の TIN を照合するため、土木モデルの頂点と面を入力順に出す。

use std::io::{self, BufWriter, Read, Write};
use zumen_base::{Budget, CancelToken, Diagnostics};
use zumen_civil::CivilModel;
use zumen_landxml::LandXml;
use zumen_sdk::{Codec, Cx, Lift};

fn main() -> io::Result<()> {
    let mut src = Vec::new();
    io::stdin().lock().read_to_end(&mut src)?;
    let mut budget = Budget::default();
    let mut diagnostics = Diagnostics::new();
    let cancel = CancelToken::new();
    let mut cx = Cx::new(&mut budget, &mut diagnostics, &cancel);
    let file =
        LandXml::read(&src, &mut cx).map_err(|error| io::Error::other(format!("{error:?}")))?;
    let model: CivilModel = file
        .lift(&mut cx)
        .map_err(|error| io::Error::other(format!("{error:?}")))?;

    let mut out = BufWriter::new(io::stdout().lock());
    for tin in model.surfaces {
        write!(out, "S {} {} ", tin.points.len(), tin.triangles.len())?;
        for byte in tin.name.as_deref().unwrap_or_default().bytes() {
            write!(out, "{byte:02x}")?;
        }
        writeln!(out)?;
        for (point, height) in tin.points {
            writeln!(
                out,
                "P {:016x} {:016x} {:016x}",
                point.northing.to_bits(),
                point.easting.to_bits(),
                height.to_bits()
            )?;
        }
        for [a, b, c] in tin.triangles {
            writeln!(out, "F {a} {b} {c}")?;
        }
    }
    out.flush()
}
