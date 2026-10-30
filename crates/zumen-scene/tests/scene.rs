//! 試験の中で作った小さな DXF から、シーンのバッファーを作って確かめる（決定 0172・0178）。

use zumen_acad::Dxf;
use zumen_base::{Budget, CancelToken, Code, Cx, Diagnostics, Kind, Limit, Limits, Stop, codes};
use zumen_drawing::{
    Color, ColorSystem, Dimension, DimensionKind, DimensionText, Drawing2D, Fill, Format, Hatch,
    HatchLoop, Height, HeightMeaning, Inherit, Islands, Label, Length, LineType,
    LineTypeDefinition, LineTypeId, ModelLength, ModelPoint, Origin, PatternBasis, Piece, Scaled,
    Shape, SpaceId, Span, Vertex,
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
    let space = &mut drawing.spaces[0];
    space.elements.push(hatch);
    space.order.push(1);
    let (scene, found) = made(&drawing, &options());
    assert_eq!(scene.counts.faces, 1);
    let read = Read(&scene.bytes);
    assert_eq!(read.u32s(layout::RING_VERTICES), vec![0]);
    assert_eq!(read.f32s(layout::FACE_XY).len(), 6);
    assert!(found.is_empty(), "{found:?}");
}

/// 線分の要素を写して、形を変えて空間の後ろに足す
#[cfg(test)]
fn push(drawing: &mut Drawing2D, shape: Shape<ModelPoint>) {
    let mut element = drawing.spaces[0].elements[0].clone();
    element.shape = shape;
    let space = &mut drawing.spaces[0];
    space.elements.push(element);
    space.order.push(space.elements.len() - 1);
}

#[cfg(test)]
fn at(x: f64, y: f64) -> ModelPoint {
    ModelPoint { space: MODEL, x, y }
}

#[cfg(test)]
fn square(x0: f64, y0: f64, x1: f64, y1: f64, outermost: bool) -> HatchLoop<ModelPoint> {
    let v = |x: f64, y: f64| Vertex {
        point: at(x, y),
        bulge: 0.0,
    };
    HatchLoop {
        boundary: zumen_drawing::Boundary::Polyline {
            vertices: vec![v(x0, y0), v(x1, y0), v(x1, y1), v(x0, y1)],
            closed: true,
        },
        external: outermost,
        outermost,
        derived: false,
        text_box: false,
        sources: Vec::new(),
    }
}

#[cfg(test)]
fn model(value: f64) -> Length {
    Length::Model(ModelLength {
        space: MODEL,
        value,
    })
}

#[test]
fn draws_hatch_pattern_lines_inside_the_rings() {
    let mut drawing = lift(&SHAPES[..9]);
    drawing.spaces[0].order.clear();
    let pattern = |offset: f64, dashes: Vec<f64>| {
        Fill::Pattern(zumen_drawing::Pattern {
            name: "ANSI31".to_string(),
            origin: zumen_drawing::PatternOrigin::Predefined,
            angle: 0.0,
            scale: 1.0,
            double: false,
            lines: vec![zumen_drawing::PatternLine {
                angle: 0.0,
                base: [0.0, 0.5],
                offset: [0.0, offset],
                dashes,
            }],
        })
    };
    let hatch = |fill| {
        Shape::Hatch(Hatch {
            loops: vec![
                square(0.0, 0.0, 10.0, 10.0, true),
                square(4.0, 4.0, 6.0, 6.0, false),
            ],
            islands: Islands::Nested,
            fill,
            associative: false,
            seeds: Vec::new(),
        })
    };
    push(&mut drawing, hatch(pattern(1.0, Vec::new())));
    let (scene, found) = made(&drawing, &options());
    // y = 0.5〜9.5 の 10 本。穴を通る 4.5 と 5.5 の 2 本は 2 つに切れる
    assert_eq!(scene.counts.lines, 12);
    assert_eq!(scene.counts.faces, 0);
    assert!(found.is_empty(), "{found:?}");
    // 線と間の列も当てる
    drawing.spaces[0].order.clear();
    push(&mut drawing, hatch(pattern(1.0, vec![3.0, -2.0])));
    let (dashed, _) = made(&drawing, &options());
    assert!(dashed.counts.lines > 12);
    // 間隔が許容差以下なら塗る
    drawing.spaces[0].order.clear();
    push(&mut drawing, hatch(pattern(0.001, Vec::new())));
    let (filled, found) = made(&drawing, &options());
    assert_eq!((filled.counts.faces, filled.counts.lines), (1, 0));
    assert_eq!(found, vec![codes::SCENE_HATCH_PATTERN_APPROXIMATED]);
}

#[test]
fn outer_islands_keep_only_the_first_ring_inside() {
    let mut drawing = lift(&SHAPES[..9]);
    drawing.spaces[0].order.clear();
    let loops = vec![
        square(0.0, 0.0, 10.0, 10.0, true),
        square(2.0, 2.0, 8.0, 8.0, false),
        square(4.0, 4.0, 6.0, 6.0, false),
    ];
    for islands in [Islands::Nested, Islands::Outer] {
        push(
            &mut drawing,
            Shape::Hatch(Hatch {
                loops: loops.clone(),
                islands,
                fill: Fill::Solid,
                associative: false,
                seeds: Vec::new(),
            }),
        );
    }
    let (scene, _) = made(&drawing, &options());
    // 入れ子は 3 つの輪、外側だけなら 3 つ目の輪を落とす
    assert_eq!(Read(&scene.bytes).u32s(layout::FACE_RINGS), vec![0, 3]);
    assert_eq!(scene.counts.faces, 2);
    assert_eq!(Read(&scene.bytes).u32s(layout::RING_VERTICES).len(), 5);
}

#[test]
fn splines_with_only_fit_points_pass_through_them() {
    let mut drawing = lift(&SHAPES[..9]);
    drawing.spaces[0].order.clear();
    let fit = vec![at(0.0, 0.0), at(5.0, 5.0), at(10.0, 0.0)];
    push(
        &mut drawing,
        Shape::Curve(zumen_drawing::Edge::Spline(zumen_drawing::Spline {
            degree: 3,
            knots: Vec::new(),
            control: Vec::new(),
            weights: Vec::new(),
            fit,
            start_tangent: None,
            end_tangent: None,
            closed: false,
            periodic: false,
            fit_tolerance: Some(0.0),
            knot_parameterization: None,
        })),
    );
    let (scene, found) = made(&drawing, &options());
    assert_eq!(found, vec![codes::SCENE_CURVE_APPROXIMATED]);
    let xy = Read(&scene.bytes).f32s(layout::LINE_XY);
    // 折れ線ではなく曲線にする。点は 3 つより多く、頂上は (5, 5) を通る
    assert!(xy.len() > 6);
    let origin = Read(&scene.bytes).f64s(layout::CHUNK_ORIGINS);
    let top = xy
        .chunks(2)
        .map(|p| f64::from(p[1]) + origin[1])
        .fold(f64::MIN, f64::max);
    assert!((top - 5.0).abs() < 0.02, "{top}");
}

#[test]
fn dimensions_without_parts_draw_lines_and_their_value() {
    let mut drawing = lift(&SHAPES[..9]);
    drawing.spaces[0].order.clear();
    let span = Span {
        piece: Piece::Text("10".to_string()),
        format: Format {
            height: Some(Scaled::Value(model(2.5))),
            ..Format::default()
        },
    };
    push(
        &mut drawing,
        Shape::Dimension(Dimension {
            kind: DimensionKind::Linear {
                first: at(0.0, 0.0),
                second: at(10.0, 0.0),
                line: at(0.0, 5.0),
                rotation: 0.0,
                oblique: 0.0,
            },
            measurement: None,
            label: Label::Override {
                content: "10".to_string(),
                spans: vec![span],
            },
            text: DimensionText {
                position: None,
                moved: false,
                rotation: None,
                attachment: None,
            },
            style: String::new(),
            block: None,
        }),
    );
    let (scene, found) = made(&drawing, &options());
    // 2 本の補助線と寸法線
    assert_eq!(scene.counts.lines, 3);
    assert_eq!(scene.counts.texts, 1);
    assert_eq!(found, vec![codes::SCENE_DIMENSION_APPROXIMATED]);
    let read = Read(&scene.bytes);
    let kinds = read.u8s(layout::ENTITY_KINDS);
    assert_eq!(kinds, vec![layout::entity_kind::DIMENSION]);
}

#[test]
fn dimensions_with_parts_draw_their_block() {
    let text = [
        (0, "SECTION"),
        (2, "BLOCKS"),
        (0, "BLOCK"),
        (2, "*D1"),
        (70, "1"),
        (10, "0"),
        (20, "0"),
        (0, "LINE"),
        (8, "0"),
        (10, "0"),
        (20, "5"),
        (11, "10"),
        (21, "5"),
        (0, "ENDBLK"),
        (0, "ENDSEC"),
        (0, "SECTION"),
        (2, "ENTITIES"),
        (0, "DIMENSION"),
        (5, "D0"),
        (8, "0"),
        (2, "*D1"),
        (10, "0"),
        (20, "5"),
        (11, "5"),
        (21, "6"),
        (70, "32"),
        (13, "0"),
        (23, "0"),
        (14, "10"),
        (24, "0"),
        (50, "0"),
        (0, "ENDSEC"),
        (0, "EOF"),
    ]
    .iter()
    .map(|(code, value)| format!("{code:>3}\n{value}\n"))
    .collect::<String>();
    let file = testkit::read::<Dxf>(text.as_bytes()).0.unwrap();
    let mut budget = Budget::new(Limits::default());
    let mut diag = Diagnostics::new();
    let drawing = file
        .lift(&mut Cx::new(&mut budget, &mut diag, &CancelToken::new()))
        .unwrap();
    let model = drawing
        .spaces
        .iter()
        .find(|s| !s.elements.is_empty())
        .unwrap()
        .id;
    let mut options = options();
    options.origin = Origin::Space(model);
    let (scene, found) = made(&drawing, &options);
    assert_eq!(scene.counts.lines, 1);
    assert!(found.is_empty(), "{found:?}");
    let read = Read(&scene.bytes);
    // 部品の線は、部品の要素として返し、道筋に寸法を持つ
    assert_eq!(
        read.u8s(layout::ENTITY_KINDS),
        vec![layout::entity_kind::LINE]
    );
    assert_eq!(read.u32s(layout::STEP_IDS).len(), 2);
}

#[test]
fn leaders_draw_their_path_and_arrowhead() {
    let mut drawing = lift(&SHAPES[..9]);
    drawing.spaces[0].order.clear();
    drawing
        .dimension_styles
        .push(zumen_drawing::DimensionStyle {
            name: "STANDARD".to_string(),
            ends: [
                zumen_drawing::Arrowhead::Default,
                zumen_drawing::Arrowhead::Default,
            ],
            leader: zumen_drawing::Arrowhead::Default,
            size: 2.0,
            tick: None,
            scale: 1.0,
            source: drawing.spaces[0].elements[0].source.clone(),
        });
    push(
        &mut drawing,
        Shape::Leader(zumen_drawing::Leader {
            path: vec![at(0.0, 0.0), at(10.0, 0.0), at(15.0, 5.0)],
            spline: false,
            arrow: None,
            annotation: None,
            style: "STANDARD".to_string(),
        }),
    );
    let (scene, found) = made(&drawing, &options());
    assert_eq!((scene.counts.lines, scene.counts.faces), (1, 1));
    assert!(found.is_empty(), "{found:?}");
    // 矢印の先は経路の始まりで、長さは寸法スタイルの大きさ
    let xy = Read(&scene.bytes).f32s(layout::FACE_XY);
    let origin = Read(&scene.bytes).f64s(layout::CHUNK_ORIGINS);
    let xs: Vec<f64> = xy.chunks(2).map(|p| f64::from(p[0]) + origin[0]).collect();
    assert!(
        (xs[0] - 0.0).abs() < 1e-6 && (xs[1] - 2.0).abs() < 1e-6,
        "{xs:?}"
    );
    // 寸法スタイルがなければ、端部記号を描かない
    if let Shape::Leader(leader) = &mut drawing.spaces[0].elements[1].shape {
        leader.style = "NONE".to_string();
    }
    let (_, found) = made(&drawing, &options());
    assert_eq!(found, vec![codes::SCENE_ARROWHEAD_DROPPED]);
}

#[test]
fn tables_without_parts_draw_cells() {
    let mut drawing = lift(&SHAPES[..9]);
    drawing.spaces[0].order.clear();
    let cell = |text: &str| zumen_drawing::Cell {
        content: zumen_drawing::CellContent::Text {
            content: text.to_string(),
            spans: vec![Span {
                piece: Piece::Text(text.to_string()),
                format: Format::default(),
            }],
        },
        span: [1, 1],
        covered: false,
        value: None,
        rotation: 0.0,
        format: zumen_drawing::CellFormat {
            text_height: Some(model(1.0)),
            ..zumen_drawing::CellFormat::default()
        },
    };
    let mut wide = cell("A");
    wide.span = [2, 1];
    let mut covered = cell("");
    covered.covered = true;
    push(
        &mut drawing,
        Shape::Table(zumen_drawing::Table {
            position: at(0.0, 10.0),
            direction: [1.0, 0.0],
            row_heights: vec![model(5.0), model(5.0)],
            column_widths: vec![model(10.0), model(10.0)],
            cells: vec![wide, covered, cell("B"), cell("C")],
            style: None,
            formats: None,
            drawn: None,
        }),
    );
    let (scene, found) = made(&drawing, &options());
    // 外の右と下の 2 本と、覆われないセルごとに上と左の 2 本
    assert_eq!(scene.counts.lines, 2 + 3 * 2);
    assert_eq!(scene.counts.texts, 3);
    assert_eq!(found, vec![codes::SCENE_TABLE_APPROXIMATED]);
}

#[test]
fn regions_filter_items_inside_the_chunks() {
    let drawing = lift(SHAPES);
    let mut options = options();
    // 1 つの塊の中で、範囲に重なる描くものだけを残す。円と点は落とす。文字は字数と高さで大きめに囲むので残す
    options.region = Some([4.0, -1.0, 6.0, 1.0]);
    let (scene, _) = made(&drawing, &options);
    assert_eq!(scene.counts.chunks, 1);
    assert_eq!(scene.counts.lines, 1);
    assert_eq!(scene.counts.texts, 1);
    assert_eq!(scene.counts.points, 0);
    // 要素ごとの表も、残る描くものが引く行だけにする
    let read = Read(&scene.bytes);
    assert_eq!(
        read.u8s(layout::ENTITY_KINDS),
        vec![layout::entity_kind::LINE, layout::entity_kind::TEXT]
    );
    assert_eq!(read.u32s(layout::UNIT_ENTITIES), vec![0, 1]);
    let ids = read.u32s(layout::ENTITY_IDS);
    let full = made(&drawing, &self::options()).0;
    let all = Read(&full.bytes).u32s(layout::ENTITY_IDS);
    assert_eq!(
        read.string([ids[0], ids[1]]),
        Read(&full.bytes).string([all[0], all[1]])
    );
    assert!(read.u8s(layout::STRINGS).len() < Read(&full.bytes).u8s(layout::STRINGS).len());
}

#[test]
fn formatted_spans_keep_their_color_and_height_meaning() {
    let mut drawing = lift(&SHAPES[..9]);
    drawing.spaces[0].order.clear();
    let red = Color {
        rgb: Some([255, 0, 0]),
        code: None,
    };
    push(
        &mut drawing,
        Shape::Formatted(zumen_drawing::FormattedText {
            content: String::new(),
            spans: vec![
                Span {
                    piece: Piece::Text("a".to_string()),
                    format: Format::default(),
                },
                Span {
                    piece: Piece::Text("b".to_string()),
                    format: Format {
                        color: Some(Inherit::Own(red)),
                        ..Format::default()
                    },
                },
            ],
            height: Height {
                value: model(2.0),
                meaning: HeightMeaning::Em,
            },
            point: at(0.0, 0.0),
            horizontal: zumen_drawing::Horizontal::Left,
            vertical: zumen_drawing::Vertical::Top,
            rotation: zumen_drawing::Rotation::Angle(0.0),
            width: None,
            frame_height: None,
            flow: zumen_drawing::Flow::LeftToRight,
            line_spacing: zumen_drawing::LineSpacing {
                factor: 1.0,
                exact: false,
            },
            background: None,
            style: String::new(),
        }),
    );
    let (scene, _) = made(&drawing, &options());
    let read = Read(&scene.bytes);
    let units = read.u32s(layout::TEXT_UNITS);
    assert_ne!(units[0], units[1]);
    let colors = read.u32s(layout::UNIT_COLORS);
    let second = colors[units[1] as usize] as usize;
    assert_eq!(
        read.u8s(layout::COLOR_RGBA)[second * 4..second * 4 + 4],
        [255, 0, 0, 255]
    );
    // 同じ要素の単位
    let entities = read.u32s(layout::UNIT_ENTITIES);
    assert_eq!(entities[units[0] as usize], entities[units[1] as usize]);
    let flags = read.u8s(layout::TEXT_FLAGS);
    assert!(flags.iter().all(|f| f & layout::text_flag::HEIGHT_EM != 0));
}

#[test]
fn underlays_clip_and_include_dwf() {
    let mut drawing = lift(&SHAPES[..9]);
    drawing.spaces[0].order.clear();
    let resource = zumen_drawing::Resource {
        path: "a.dwf".to_string(),
        definition: drawing.spaces[0].elements[0].source.clone(),
    };
    push(
        &mut drawing,
        Shape::Underlay(zumen_drawing::Underlay {
            kind: zumen_drawing::UnderlayKind::Dwf,
            resource: resource.clone(),
            page: None,
            frame: zumen_drawing::UnderlayFrame::Scaled {
                origin: at(10.0, 0.0),
                scale: [model(2.0), model(2.0)],
                rotation: 0.0,
            },
            clip: Some(zumen_drawing::Clip {
                vertices: vec![[0.0, 0.0], [1.0, 1.0]],
                inverted: false,
            }),
            visible: true,
        }),
    );
    // 画素の格子の切り抜きは、左上の画素の中心を原点とし、下へ進む
    push(
        &mut drawing,
        Shape::Underlay(zumen_drawing::Underlay {
            kind: zumen_drawing::UnderlayKind::Image,
            resource,
            page: None,
            frame: zumen_drawing::UnderlayFrame::Pixels {
                size: [10.0, 10.0],
                origin: at(0.0, 0.0),
                right: at(10.0, 0.0),
                top: at(0.0, 10.0),
            },
            clip: Some(zumen_drawing::Clip {
                vertices: vec![[-0.5, -0.5], [4.5, 4.5]],
                inverted: false,
            }),
            visible: true,
        }),
    );
    let (scene, found) = made(&drawing, &options());
    assert!(found.is_empty(), "{found:?}");
    let read = Read(&scene.bytes);
    assert_eq!(
        read.u8s(layout::IMAGE_KINDS),
        vec![layout::image_kind::DWF, layout::image_kind::IMAGE]
    );
    let clip = read.f64s(layout::CLIP_XY);
    assert_eq!(&clip[..4], &[10.0, 0.0, 12.0, 0.0]);
    // 左上の 5×5 画素は、(0, 5)〜(5, 10)
    assert_eq!(&clip[8..12], &[0.0, 10.0, 5.0, 10.0]);
    assert_eq!(&clip[12..14], &[5.0, 5.0]);
}

#[test]
fn line_type_text_marks_are_placed_along_the_line() {
    let mut drawing = lift(&SHAPES[..9]);
    drawing.line_types.push(LineType {
        id: LineTypeId(90),
        name: "GAS".to_string(),
        definition: Some(LineTypeDefinition {
            description: String::new(),
            pattern: vec![3.0, -2.0],
            basis: PatternBasis::Element,
            marks: vec![
                zumen_drawing::LineTypeMark {
                    index: 1,
                    content: zumen_drawing::MarkContent::Text {
                        content: "G".to_string(),
                        style: None,
                    },
                    scale: 0.5,
                    rotation: 0.0,
                    rotation_basis: zumen_drawing::MarkRotation::Relative,
                    offset: [0.5, -0.25],
                },
                zumen_drawing::LineTypeMark {
                    index: 0,
                    content: zumen_drawing::MarkContent::Shape {
                        font: None,
                        number: 1,
                    },
                    scale: 1.0,
                    rotation: 0.0,
                    rotation_basis: zumen_drawing::MarkRotation::Relative,
                    offset: [0.0, 0.0],
                },
            ],
        }),
        source: drawing.spaces[0].elements[0].source.clone(),
    });
    drawing.spaces[0].elements[0].style.line_type = Inherit::Own(LineTypeId(90));
    let (scene, found) = made(&drawing, &options());
    // 長さ 10 の線に、間の始まり 3 と 8 で 2 つの文字
    assert_eq!(scene.counts.texts, 2);
    assert_eq!(found, vec![codes::SCENE_LINE_TYPE_SHAPE_DROPPED]);
    let read = Read(&scene.bytes);
    let origin = read.f64s(layout::CHUNK_ORIGINS);
    let xy = read.f32s(layout::TEXT_XY);
    assert!((f64::from(xy[0]) + origin[0] - 3.5).abs() < 1e-6);
    assert!((f64::from(xy[1]) + origin[1] + 0.25).abs() < 1e-6);
    assert_eq!(read.f32s(layout::TEXT_HEIGHTS)[0], 0.5);
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
