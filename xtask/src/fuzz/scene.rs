//! SVG を書く target に、線と列の参照を持つ合成シーンを渡す（#602）。

use zumen_scene::layout as l;

pub(super) fn seed() -> Vec<u8> {
    let mut columns = vec![Vec::new(); l::SECTIONS.len()];
    columns[l::CHUNK_ORIGINS] = [0_f64; 2].into_iter().flat_map(f64::to_le_bytes).collect();
    columns[l::CHUNK_BOUNDS] = [0_f32, 0.0, 20.0, 20.0]
        .into_iter()
        .flat_map(f32::to_le_bytes)
        .collect();
    for section in [
        l::CHUNK_LINES,
        l::CHUNK_FACES,
        l::CHUNK_TEXTS,
        l::CHUNK_POINTS,
        l::CHUNK_IMAGES,
        l::LINE_VERTICES,
        l::LINE_UNITS,
        l::UNIT_ENTITIES,
        l::UNIT_COLORS,
        l::ENTITY_PATHS,
        l::COLOR_CODES,
    ] {
        columns[section] = 0_u32.to_le_bytes().to_vec();
    }
    for section in [l::UNIT_LAYERS, l::UNIT_CLIPS] {
        columns[section] = l::NONE.to_le_bytes().to_vec();
    }
    columns[l::UNIT_WEIGHTS] = 1_f32.to_le_bytes().to_vec();
    columns[l::LINE_XY] = [0_f32, 0.0, 20.0, 20.0]
        .into_iter()
        .flat_map(f32::to_le_bytes)
        .collect();
    columns[l::ENTITY_IDS] = [0_u32, 9].into_iter().flat_map(u32::to_le_bytes).collect();
    for section in [l::ENTITY_KINDS, l::COLOR_SYSTEMS, l::COLOR_FLAGS] {
        columns[section] = vec![0];
    }
    columns[l::COLOR_FILES] = vec![0; 4];
    columns[l::COLOR_RGBA] = vec![0, 0, 0, 255];
    columns[l::STRINGS] = b"fuzz-line".to_vec();
    let mut bytes = l::MAGIC.to_vec();
    bytes.extend(l::MAJOR.to_le_bytes());
    bytes.extend(l::MINOR.to_le_bytes());
    bytes.extend(1_u32.to_le_bytes());
    bytes.extend((columns.len() as u32).to_le_bytes());
    bytes.resize(16 + columns.len() * 8, 0);
    for (index, column) in columns.iter().enumerate() {
        bytes.resize(bytes.len().next_multiple_of(8), 0);
        let start = (bytes.len() as u32).to_le_bytes();
        bytes[16 + index * 8..20 + index * 8].copy_from_slice(&start);
        bytes[20 + index * 8..24 + index * 8].copy_from_slice(&(column.len() as u32).to_le_bytes());
        bytes.extend(column);
    }
    bytes
}

#[cfg(test)]
mod tests {
    #[test]
    fn seed_reaches_svg_paths() {
        use zumen_base::{Budget, CancelToken, Cx, Diagnostics};
        let mut budget = Budget::default();
        let mut diag = Diagnostics::new();
        let cancel = CancelToken::new();
        let options = zumen_render::Options {
            width: 100,
            height: 100,
            region: [0.0, 0.0, 20.0, 20.0],
            dpi: 96.0,
            background: [255; 4],
            foreground: [0, 0, 0, 255],
        };
        let svg = zumen_render::svg(
            &super::seed(),
            &options,
            &mut Cx::new(&mut budget, &mut diag, &cancel),
        )
        .unwrap();
        assert!(String::from_utf8(svg).unwrap().contains("M0 100L100 0"));
        assert!(diag.is_empty());
    }
}
