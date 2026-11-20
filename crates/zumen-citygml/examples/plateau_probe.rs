//! 公開 PLATEAU 原本の面を、地物 ID・LOD・輪・座標の順に出す。
use std::io::{self, BufWriter, Read, Write};
use zumen_base::{Budget, CancelToken, Diagnostics, codes};
use zumen_citygml::CityGml;
use zumen_geo::{Form, GeoModel};
use zumen_geom3d::Shape;
use zumen_sdk::{Codec, Cx, Lift};

fn main() -> io::Result<()> {
    let mut data = Vec::new();
    io::stdin().lock().read_to_end(&mut data)?;
    let mut budget = Budget::default();
    let mut diag = Diagnostics::new();
    let cancel = CancelToken::new();
    let mut cx = Cx::new(&mut budget, &mut diag, &cancel);
    let mut model: GeoModel = CityGml::read(&data, &mut cx)
        .and_then(|file| file.lift(&mut cx))
        .map_err(|e| io::Error::other(format!("{e:?}")))?;
    let mut out = BufWriter::new(io::stdout().lock());
    writeln!(
        out,
        "CRS {:?} {:?}",
        model.crs.map(|v| v.epsg()),
        model.height.map(|v| v.epsg())
    )?;
    model.features.sort_by(|a, b| a.id.cmp(&b.id));
    for feature in model
        .features
        .into_iter()
        .filter(|f| !f.geometries.is_empty())
    {
        writeln!(out, "ID {}", feature.id)?;
        for geometry in feature.geometries {
            let Form::Solid(Shape::FaceSet { faces, closed }) = geometry.form else {
                return Err(io::Error::other("面の集合を想定した原本です"));
            };
            let lod = geometry
                .lod
                .ok_or_else(|| io::Error::other("LOD がありません"))?;
            writeln!(
                out,
                "LOD {lod} {}",
                if closed { "Solid" } else { "MultiSurface" }
            )?;
            for face in faces {
                for (kind, ring) in std::iter::once(("outer", face.outer))
                    .chain(face.holes.into_iter().map(|r| ("hole", r)))
                {
                    writeln!(out, "{kind} {}", ring.len())?;
                    for [lat, lon, z] in ring {
                        writeln!(
                            out,
                            "{:016x} {:016x} {:016x}",
                            lat.to_bits(),
                            lon.to_bits(),
                            z.to_bits()
                        )?;
                    }
                }
            }
        }
    }
    let invalid: u64 = diag
        .iter()
        .filter(|d| d.code() == codes::CITYGML_VALUE_INVALID)
        .map(|d| d.count())
        .sum();
    writeln!(out, "invalid {invalid}")?;
    out.flush()
}
