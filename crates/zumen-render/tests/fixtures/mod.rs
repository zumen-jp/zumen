//! 固定の列から作る、小さい合成シーン。テスト・照合・fuzz の種で共有する。
#![allow(clippy::indexing_slicing)]
#![allow(dead_code)]

use zumen_base::{Budget, CancelToken, Cx, Diagnostics, Kind, Limits};
use zumen_scene::layout as l;

pub struct Columns(pub Vec<Vec<u8>>);

impl Columns {
    pub fn new() -> Self {
        Self(vec![Vec::new(); l::SECTIONS.len()])
    }
    pub fn u(&mut self, s: usize, v: &[u32]) {
        self.0[s].extend(v.iter().flat_map(|x| x.to_le_bytes()));
    }
    pub fn f(&mut self, s: usize, v: &[f32]) {
        self.0[s].extend(v.iter().flat_map(|x| x.to_le_bytes()));
    }
    pub fn d(&mut self, s: usize, v: &[f64]) {
        self.0[s].extend(v.iter().flat_map(|x| x.to_le_bytes()));
    }
    pub fn b(&mut self, s: usize, v: &[u8]) {
        self.0[s].extend(v);
    }
    pub fn string(&mut self, s: usize, text: &str) {
        let start = self.0[l::STRINGS].len() as u32;
        self.b(l::STRINGS, text.as_bytes());
        self.u(s, &[start, text.len() as u32]);
    }
    pub fn pack(&self) -> Vec<u8> {
        let mut out = b"ZMSC\x01\x00\x02\x00".to_vec();
        out.extend((self.0[l::CHUNK_ORIGINS].len() as u32 / 16).to_le_bytes());
        out.extend((self.0.len() as u32).to_le_bytes());
        out.resize(16 + self.0.len() * 8, 0);
        for (index, bytes) in self.0.iter().enumerate() {
            out.resize(out.len().next_multiple_of(8), 0);
            let offset = (out.len() as u32).to_le_bytes();
            out[16 + index * 8..20 + index * 8].copy_from_slice(&offset);
            out[20 + index * 8..24 + index * 8]
                .copy_from_slice(&(bytes.len() as u32).to_le_bytes());
            out.extend(bytes);
        }
        out
    }
}

pub fn options() -> zumen_render::Options {
    zumen_render::Options {
        width: 100,
        height: 100,
        region: [0.0, 0.0, 100.0, 100.0],
        dpi: 25.4,
        background: [255; 4],
        foreground: [0, 0, 0, 255],
    }
}

/// 赤い面（中央に穴）、青い線、緑の字形、点、未描画の参照と反転を含むクリップ。
pub fn mixed() -> Columns {
    let mut c = Columns::new();
    c.d(l::CHUNK_ORIGINS, &[0.0, 0.0]);
    c.f(l::CHUNK_BOUNDS, &[0.0, 0.0, 100.0, 100.0]);
    for s in l::CHUNK_LINES..=l::CHUNK_IMAGES {
        c.u(s, &[0]);
    }
    for unit in 0..8 {
        c.u(l::UNIT_ENTITIES, &[unit]);
        c.u(l::UNIT_COLORS, &[unit]);
        c.f(l::UNIT_WEIGHTS, &[2.0]);
        c.u(l::UNIT_CLIPS, &[if unit == 7 { 0 } else { l::NONE }]);
        c.u(l::UNIT_LAYERS, &[l::NONE]);
        c.string(l::ENTITY_IDS, &format!("fixture-{unit}"));
        c.b(l::ENTITY_KINDS, &[0]);
        c.u(l::ENTITY_PATHS, &[0]);
        c.b(l::COLOR_SYSTEMS, &[0]);
        c.u(l::COLOR_CODES, &[0]);
        c.b(l::COLOR_FILES, &[0; 4]);
        c.b(
            l::COLOR_RGBA,
            &match unit {
                0 => [255, 0, 0, 255],
                1 => [0, 0, 255, 255],
                2 => [0, 160, 0, 255],
                7 => [255, 128, 0, 255],
                _ => [0, 0, 0, 255],
            },
        );
        c.b(
            l::COLOR_FLAGS,
            &[if unit == 3 {
                l::color_flag::FOREGROUND
            } else {
                0
            }],
        );
    }
    c.u(l::LINE_VERTICES, &[0]);
    c.u(l::LINE_UNITS, &[1]);
    c.f(l::LINE_XY, &[0.0, 50.0, 100.0, 50.0]);
    c.u(l::FACE_RINGS, &[0, 2]);
    c.u(l::FACE_UNITS, &[0, 7]);
    c.u(l::RING_VERTICES, &[0, 4, 8]);
    c.f(
        l::FACE_XY,
        &[
            0.0, 0.0, 100.0, 0.0, 100.0, 100.0, 0.0, 100.0, 40.0, 40.0, 60.0, 40.0, 60.0, 60.0,
            40.0, 60.0, 0.0, 0.0, 100.0, 0.0, 100.0, 100.0, 0.0, 100.0,
        ],
    );
    for unit in [2, 4] {
        c.string(
            l::TEXT_STRINGS,
            if unit == 2 {
                "A"
            } else {
                "<script>missing</script>"
            },
        );
        c.f(l::TEXT_XY, &[20.0, 20.0]);
        c.f(l::TEXT_SECOND, &[f32::NAN; 2]);
        for s in [
            l::TEXT_HEIGHTS,
            l::TEXT_WIDTHS,
            l::TEXT_ROTATIONS,
            l::TEXT_OBLIQUES,
            l::TEXT_BOXES,
        ] {
            c.f(s, &[1.0]);
        }
        c.b(l::TEXT_ALIGNS, &[0; 2]);
        c.b(l::TEXT_FLAGS, &[0]);
        c.u(l::TEXT_LINES, &[0]);
        c.u(l::TEXT_FONTS, &[l::NONE]);
        c.u(l::TEXT_UNITS, &[unit]);
    }
    c.u(l::TEXT_GLYPHS, &[0, 2]);
    c.u(l::GLYPH_SHAPES, &[0, 1]);
    c.f(l::GLYPH_AXES, &[20.0, 0.0, 0.0, 20.0, 20.0, 0.0, 0.0, 20.0]);
    c.f(l::GLYPH_XY, &[20.0, 20.0, 45.0, 20.0]);
    // 字形 1 は空白で、輪を持たない。
    c.u(l::SHAPE_RINGS, &[0, 1]);
    c.u(l::OUTLINE_VERTICES, &[0]);
    c.f(l::OUTLINE_XY, &[0.0, 0.0, 1.0, 0.0, 0.5, 1.0]);
    for unit in [3, 6] {
        c.f(l::POINT_XY, &[80.0, 80.0]);
        c.f(l::POINT_ROTATIONS, &[30.0]);
        c.f(l::POINT_SCALES, &[1.0]);
        c.u(l::POINT_UNITS, &[unit]);
        c.string(l::POINT_MARKERS, if unit == 6 { "named" } else { "" });
        c.string(l::POINT_MARKERS, "");
    }
    c.f(l::POINT_STYLE, &[2.0, 3.0, 1.0, 10.0]);
    c.string(l::IMAGE_NAMES, "https://example.invalid/private-image");
    c.string(l::IMAGE_PAGES, "");
    c.f(l::IMAGE_XY, &[0.0, 0.0]);
    c.f(l::IMAGE_EDGES, &[10.0, 0.0, 0.0, 10.0]);
    c.b(l::IMAGE_KINDS, &[0]);
    c.b(l::IMAGE_FRAMES, &[0]);
    c.u(l::IMAGE_UNITS, &[5]);
    c.u(l::CLIP_POLYGONS, &[0]);
    c.u(l::POLYGON_VERTICES, &[0, 4]);
    c.b(l::POLYGON_FLAGS, &[0, 1]);
    c.d(
        l::CLIP_XY,
        &[
            65.0, 5.0, 95.0, 5.0, 95.0, 35.0, 65.0, 35.0, 75.0, 15.0, 85.0, 15.0, 85.0, 25.0, 75.0,
            25.0,
        ],
    );
    c
}

pub type Outcome = (
    Result<Vec<u8>, zumen_render::Error>,
    Vec<zumen_base::Diagnostic>,
    Vec<(Kind, u64)>,
);

pub fn run(
    bytes: &[u8],
    options: &zumen_render::Options,
    limits: Limits,
    cancelled: bool,
) -> Outcome {
    let mut budget = Budget::new(limits);
    let mut diag = Diagnostics::new();
    let cancel = CancelToken::new();
    if cancelled {
        cancel.cancel();
    }
    let result = zumen_render::svg(
        bytes,
        options,
        &mut Cx::new(&mut budget, &mut diag, &cancel),
    );
    (
        result,
        diag.into_vec(),
        Kind::ALL
            .iter()
            .map(|&kind| (kind, budget.used(kind)))
            .collect(),
    )
}

/// 同じ組をネイティブと wasm で走らせ、出力全体・診断・予算・停止を比較するためのバイト列。
pub fn transcript() -> Vec<u8> {
    use zumen_base::Limit;
    let mut out = Vec::new();
    let mut inputs = vec![mixed().pack(), Columns::new().pack(), b"ZMSC".to_vec()];
    let mut huge = mixed();
    huge.d(l::CHUNK_ORIGINS, &[f64::MAX, 0.0]);
    inputs.push(huge.pack());
    for input in inputs {
        for limits in [
            Limits::default(),
            Limits::default().with(Kind::Work, Limit::Finite(180)),
            Limits::default().with(Kind::DecodedBytes, Limit::Finite(1)),
            Limits::default().with(Kind::Depth, Limit::Finite(1)),
            Limits::default().with(Kind::InputBytes, Limit::Finite(1)),
            Limits::default().with(Kind::Elements, Limit::Finite(1)),
            Limits::default().with(Kind::Segments, Limit::Finite(10)),
        ] {
            for cancelled in [false, true] {
                let (result, diag, used) = run(&input, &options(), limits, cancelled);
                match result {
                    Ok(bytes) => out.extend(bytes),
                    Err(error) => out.extend(format!("{error:?}").as_bytes()),
                }
                out.extend(format!("{diag:?}{used:?}\n").as_bytes());
            }
        }
    }
    out
}
