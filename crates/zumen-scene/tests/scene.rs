//! 試験の中で作った小さな DXF から、シーンのバッファーを作って確かめる（決定 0172・0178）。

use zumen_acad::Dxf;
use zumen_base::{Budget, CancelToken, Code, Cx, Diagnostics, Kind, Limit, Limits, Stop, codes};
use zumen_drawing::{
    ColorSystem, Drawing2D, Fill, Hatch, HatchLoop, Inherit, Islands, LineType, LineTypeDefinition,
    LineTypeId, ModelPoint, Origin, PatternBasis, Shape, SpaceId, Vertex,
};
use zumen_scene::layout::{self, SECTIONS};
use zumen_scene::{Chunking, Options, Palette, Scene, build};
use zumen_sdk::{Lift, testkit};

const MODEL: SpaceId = SpaceId(0);

/// 図形のタグの組から、ENTITIES だけの DXF を作る
#[cfg(test)]
fn dxf(entities: &[(i16, &str)]) -> Vec<u8> {
    let mut tags = vec![(0, "SECTION"), (2, "ENTITIES")];
    tags.extend_from_slice(entities);
    tags.extend_from_slice(&[(0, "ENDSEC"), (0, "EOF")]);
    tags.iter()
        .map(|(code, value)| format!("{code:>3}\n{value}\n"))
        .collect::<String>()
        .into_bytes()
}

#[cfg(test)]
fn lift(entities: &[(i16, &str)]) -> Drawing2D {
    let file = testkit::read::<Dxf>(&dxf(entities)).0.unwrap();
    let mut budget = Budget::new(Limits::default());
    let mut diag = Diagnostics::new();
    file.lift(&mut Cx::new(&mut budget, &mut diag, &CancelToken::new()))
        .unwrap()
}

#[cfg(test)]
fn options() -> Options {
    Options {
        origin: Origin::Space(MODEL),
        tolerance: Some(0.01),
        print: false,
        region: None,
        palette: Palette::default(),
        chunking: Chunking::PROVISIONAL,
    }
}

#[cfg(test)]
fn scene_with(
    drawing: &Drawing2D,
    options: &Options,
    limits: Limits,
) -> (Result<Scene, Stop>, Vec<Code>) {
    let mut budget = Budget::new(limits);
    let mut diag = Diagnostics::new();
    let result = build(
        drawing,
        options,
        &mut Cx::new(&mut budget, &mut diag, &CancelToken::new()),
    )
    .map_err(|(stop, _)| stop);
    (result, diag.iter().map(|d| d.code()).collect())
}

#[cfg(test)]
fn made(drawing: &Drawing2D, options: &Options) -> (Scene, Vec<Code>) {
    let (result, codes) = scene_with(drawing, options, Limits::default());
    (result.unwrap(), codes)
}

/// 見出しを読んだバッファー
#[cfg(test)]
struct Read<'a>(&'a [u8]);

#[cfg(test)]
impl Read<'_> {
    fn u32_at(&self, at: usize) -> u32 {
        u32::from_le_bytes(self.0[at..at + 4].try_into().unwrap())
    }

    fn section(&self, index: usize) -> &[u8] {
        let start = self.u32_at(16 + 8 * index) as usize;
        let len = self.u32_at(20 + 8 * index) as usize;
        assert_eq!(
            start % 8,
            0,
            "{} は 8 バイトの境界に置く",
            SECTIONS[index].name
        );
        &self.0[start..start + len]
    }

    fn u8s(&self, index: usize) -> Vec<u8> {
        self.section(index).to_vec()
    }

    fn u32s(&self, index: usize) -> Vec<u32> {
        self.section(index)
            .as_chunks::<4>()
            .0
            .iter()
            .map(|b| u32::from_le_bytes(*b))
            .collect()
    }

    fn f32s(&self, index: usize) -> Vec<f32> {
        self.section(index)
            .as_chunks::<4>()
            .0
            .iter()
            .map(|b| f32::from_le_bytes(*b))
            .collect()
    }

    fn f64s(&self, index: usize) -> Vec<f64> {
        self.section(index)
            .as_chunks::<8>()
            .0
            .iter()
            .map(|b| f64::from_le_bytes(*b))
            .collect()
    }

    fn string(&self, [start, len]: [u32; 2]) -> String {
        let strings = self.section(layout::STRINGS);
        String::from_utf8(strings[start as usize..(start + len) as usize].to_vec()).unwrap()
    }
}

const SHAPES: &[(i16, &str)] = &[
    (0, "LINE"),
    (5, "A1"),
    (8, "0"),
    (62, "1"),
    (10, "0"),
    (20, "0"),
    (11, "10"),
    (21, "0"),
    (0, "CIRCLE"),
    (5, "A2"),
    (8, "0"),
    (10, "100"),
    (20, "100"),
    (40, "5"),
    (0, "TEXT"),
    (5, "A3"),
    (8, "0"),
    (10, "1"),
    (20, "2"),
    (40, "2.5"),
    (1, "ABC"),
    (0, "POINT"),
    (5, "A4"),
    (8, "0"),
    (10, "3"),
    (20, "4"),
];

#[test]
fn writes_the_header_and_sections_by_the_schema() {
    let drawing = lift(SHAPES);
    let (scene, found) = made(&drawing, &options());
    let read = Read(&scene.bytes);
    assert_eq!(&scene.bytes[0..4], b"ZMSC");
    assert_eq!(
        u16::from_le_bytes([scene.bytes[4], scene.bytes[5]]),
        layout::MAJOR
    );
    assert_eq!(read.u32_at(8), 1, "塊の数");
    assert_eq!(read.u32_at(12) as usize, SECTIONS.len());
    assert_eq!(scene.bytes.len() % 8, 0);
    assert_eq!(scene.counts.lines, 2);
    assert_eq!(scene.counts.texts, 1);
    assert_eq!(scene.counts.points, 1);
    assert_eq!(scene.tolerance, 0.01);
    assert_eq!(found, vec![codes::SCENE_CURVE_APPROXIMATED]);
    // 文字は内容と高さと単位を持つ
    let strings = read.u32s(layout::TEXT_STRINGS);
    assert_eq!(read.string([strings[0], strings[1]]), "ABC");
    assert_eq!(read.f32s(layout::TEXT_HEIGHTS), vec![2.5]);
    // 描く単位は、要素の ID と色を引く
    let units = read.u32s(layout::LINE_UNITS);
    let entity = read.u32s(layout::UNIT_ENTITIES)[units[0] as usize] as usize;
    let ids = read.u32s(layout::ENTITY_IDS);
    assert_eq!(
        read.string([ids[2 * entity], ids[2 * entity + 1]]),
        "space/0/hA1"
    );
    let color = read.u32s(layout::UNIT_COLORS)[units[0] as usize] as usize;
    assert_eq!(
        read.u8s(layout::COLOR_SYSTEMS)[color],
        layout::color_system::ACI
    );
    assert_eq!(read.u32s(layout::COLOR_CODES)[color], 1);
    // 色の表がなければ、未解決の印を立てる
    assert_eq!(
        read.u8s(layout::COLOR_FLAGS)[color],
        layout::color_flag::UNRESOLVED
    );
}

#[test]
fn circles_stay_within_the_tolerance_around_the_chunk_origin() {
    let drawing = lift(SHAPES);
    let (scene, _) = made(&drawing, &options());
    let read = Read(&scene.bytes);
    let origin = read.f64s(layout::CHUNK_ORIGINS);
    let starts = read.u32s(layout::LINE_VERTICES);
    let xy = read.f32s(layout::LINE_XY);
    // 2 本目の線が円。原点からの相対の座標を戻して、中心からの距離を確かめる
    let circle: Vec<[f64; 2]> = xy[starts[1] as usize * 2..]
        .as_chunks::<2>()
        .0
        .iter()
        .map(|p| [origin[0] + f64::from(p[0]), origin[1] + f64::from(p[1])])
        .collect();
    assert!(circle.len() > 8);
    assert_eq!(circle.first(), circle.last());
    for p in &circle {
        let r = libm_hypot(p[0] - 100.0, p[1] - 100.0);
        assert!((r - 5.0).abs() < 1e-4, "{r}");
    }
    for w in circle.windows(2) {
        let mid = [(w[0][0] + w[1][0]) / 2.0, (w[0][1] + w[1][1]) / 2.0];
        assert!(5.0 - libm_hypot(mid[0] - 100.0, mid[1] - 100.0) <= 0.01 + 1e-4);
    }
    // 範囲は、描くものの点と文字の基点を囲む
    let [x0, y0, x1, y1] = scene.extent.unwrap();
    assert!(x0 <= 0.0 && y0 <= 0.0 && x1 >= 104.99 && y1 >= 104.99);
}

#[cfg(test)]
fn libm_hypot(x: f64, y: f64) -> f64 {
    (x * x + y * y).sqrt()
}

#[test]
fn splits_chunks_and_filters_them_by_region() {
    let drawing = lift(SHAPES);
    let mut options = options();
    options.chunking = Chunking {
        items: 1,
        span: 65536.0,
    };
    let (scene, _) = made(&drawing, &options);
    assert_eq!(scene.counts.chunks, 4);
    let read = Read(&scene.bytes);
    assert_eq!(read.u32s(layout::CHUNK_LINES), vec![0, 1, 2, 2]);
    assert_eq!(read.u32s(layout::CHUNK_TEXTS), vec![0, 0, 0, 1]);
    // 範囲に重なる塊だけを返す。円の塊だけが重なる
    options.region = Some([90.0, 90.0, 110.0, 110.0]);
    let (scene, _) = made(&drawing, &options);
    assert_eq!(scene.counts.chunks, 1);
    assert_eq!(scene.counts.lines, 1);
    let origin = Read(&scene.bytes).f64s(layout::CHUNK_ORIGINS);
    assert!((origin[0] - 95.0).abs() < 0.02 && (origin[1] - 95.0).abs() < 0.02);
    // 塊の範囲が許容差の倍率を超えるなら、数が上限に届かなくても分ける
    options.region = None;
    options.chunking = Chunking {
        items: 4096,
        span: 50.0 / 0.01,
    };
    let (scene, _) = made(&drawing, &options);
    // 線・円・文字と点の 3 つ。描く順を保つので、離れた円の後の文字は、線の塊に戻らない
    assert_eq!(scene.counts.chunks, 3);
}

#[test]
fn looks_up_colors_in_the_palette() {
    let drawing = lift(SHAPES);
    let mut options = options();
    options.palette.insert(ColorSystem::Aci, 1, [255, 0, 0]);
    let (scene, _) = made(&drawing, &options);
    let read = Read(&scene.bytes);
    let unit = read.u32s(layout::LINE_UNITS)[0] as usize;
    let color = read.u32s(layout::UNIT_COLORS)[unit] as usize;
    assert_eq!(
        read.u8s(layout::COLOR_RGBA)[color * 4..color * 4 + 4],
        [255, 0, 0, 255]
    );
    assert_eq!(read.u8s(layout::COLOR_FLAGS)[color], 0);
}

#[test]
fn expands_line_types_into_dashes() {
    let mut drawing = lift(&SHAPES[..9]);
    drawing.line_types.push(LineType {
        id: LineTypeId(90),
        name: "DASHED".to_string(),
        definition: Some(LineTypeDefinition {
            description: String::new(),
            pattern: vec![2.0, -1.0],
            basis: PatternBasis::Element,
            marks: Vec::new(),
        }),
        source: drawing.spaces[0].elements[0].source.clone(),
    });
    drawing.spaces[0].elements[0].style.line_type = Inherit::Own(LineTypeId(90));
    let (scene, _) = made(&drawing, &options());
    // 長さ 10 の線を、線 2・間 1 で分ける: 0-2, 3-5, 6-8, 9-10
    assert_eq!(scene.counts.lines, 4);
    let xy = Read(&scene.bytes).f32s(layout::LINE_XY);
    assert_eq!(
        xy,
        vec![
            0.0, 0.0, 2.0, 0.0, 3.0, 0.0, 5.0, 0.0, 6.0, 0.0, 8.0, 0.0, 9.0, 0.0, 10.0, 0.0
        ]
    );
}

#[test]
fn fills_solid_hatches_and_reports_what_it_does_not_draw() {
    let mut drawing = lift(&SHAPES[..9]);
    let mut hatch = drawing.spaces[0].elements[0].clone();
    let at = |x: f64, y: f64| Vertex {
        point: ModelPoint { space: MODEL, x, y },
        bulge: 0.0,
    };
    hatch.shape = Shape::Hatch(Hatch {
        loops: vec![HatchLoop {
            boundary: zumen_drawing::Boundary::Polyline {
                vertices: vec![at(0.0, 0.0), at(4.0, 0.0), at(4.0, 4.0)],
                closed: true,
            },
            external: true,
            outermost: true,
            derived: false,
            text_box: false,
            sources: Vec::new(),
        }],
        islands: Islands::Nested,
        fill: Fill::Solid,
        associative: false,
        seeds: Vec::new(),
    });
    let mut pattern = hatch.clone();
    if let Shape::Hatch(h) = &mut pattern.shape {
        h.fill = Fill::Pattern(zumen_drawing::Pattern {
            name: "ANSI31".to_string(),
            origin: zumen_drawing::PatternOrigin::Predefined,
            angle: 0.0,
            scale: 1.0,
            double: false,
            lines: Vec::new(),
        });
    }
    let space = &mut drawing.spaces[0];
    space.elements.push(hatch);
    space.elements.push(pattern);
    space.order.extend([1, 2]);
    let (scene, found) = made(&drawing, &options());
    assert_eq!(scene.counts.faces, 1);
    let read = Read(&scene.bytes);
    assert_eq!(read.u32s(layout::RING_VERTICES), vec![0]);
    assert_eq!(read.f32s(layout::FACE_XY).len(), 6);
    assert_eq!(found, vec![codes::SCENE_HATCH_PATTERN_DROPPED]);
}

#[test]
fn counts_vertices_against_the_segment_budget() {
    let drawing = lift(SHAPES);
    let mut limits = Limits::default();
    limits.set(Kind::Segments, Limit::Finite(5));
    let (result, _) = scene_with(&drawing, &options(), limits);
    assert!(matches!(result, Err(Stop::Exceeded(_))));
}

#[test]
fn same_drawing_gives_the_same_bytes() {
    let drawing = lift(SHAPES);
    let mut options = options();
    options.tolerance = None;
    let (a, _) = made(&drawing, &options);
    let (b, _) = made(&drawing, &options);
    assert_eq!(a.bytes, b.bytes);
    // 空間からの既定の許容差は、範囲の対角線の 1/50000
    assert!(a.tolerance > 0.0 && a.tolerance < 0.01);
}
