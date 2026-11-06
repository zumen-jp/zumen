//! 外部との照合用に、小さい合成シーン・SVG・停止の記録を指定フォルダーへ書く。

#[path = "../tests/fixtures/mod.rs"]
mod fixtures;

use std::{error::Error, fs, path::PathBuf};

fn main() -> Result<(), Box<dyn Error>> {
    let folder = PathBuf::from(std::env::args_os().nth(1).ok_or("出力フォルダーが要る")?);
    fs::create_dir_all(&folder)?;
    let input = fixtures::mixed().pack();
    let (result, _, _) = fixtures::run(
        &input,
        &fixtures::options(),
        zumen_base::Limits::default(),
        false,
    );
    let svg = result.map_err(|e| format!("SVG を作れない: {e:?}"))?;
    let mut options = fixtures::options();
    options.width = 200;
    options.region = [20.0, 20.0, 80.0, 80.0];
    let (region, _, _) = fixtures::run(&input, &options, zumen_base::Limits::default(), false);
    fs::write(
        folder.join("region.svg"),
        region.map_err(|e| format!("範囲の SVG を作れない: {e:?}"))?,
    )?;
    for (name, options, input) in [
        ("drawing", fixtures::options(), input.clone()),
        ("region", options, input.clone()),
        (
            "alpha",
            zumen_render::Options {
                width: 257,
                height: 80,
                background: [40, 80, 160, 128],
                ..fixtures::options()
            },
            fixtures::Columns::new().pack(),
        ),
        (
            "alpha-drawing",
            zumen_render::Options {
                background: [40, 80, 160, 128],
                ..fixtures::options()
            },
            fixtures::translucent().pack(),
        ),
    ] {
        let (result, _, _) =
            fixtures::run_png(&input, &options, zumen_base::Limits::default(), false);
        let png = result.map_err(|e| format!("PNG を作れない: {e:?}"))?;
        fs::write(
            folder.join(format!("{name}.rgba")),
            fixtures::pixels(&png).2,
        )?;
        fs::write(folder.join(format!("{name}.png")), png)?;
    }
    fs::write(folder.join("scene.zmsc"), input)?;
    fs::write(folder.join("drawing.svg"), svg)?;
    fs::write(folder.join("transcript.bin"), fixtures::transcript())?;
    Ok(())
}
