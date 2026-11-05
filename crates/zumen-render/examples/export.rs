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
    fs::write(folder.join("scene.zmsc"), input)?;
    fs::write(folder.join("drawing.svg"), svg)?;
    fs::write(folder.join("transcript.bin"), fixtures::transcript())?;
    Ok(())
}
