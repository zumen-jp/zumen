//! 描画と、壊れたシーン・予算・ID の契約を小さい合成入力で確かめる。
mod fixtures;

use fixtures::{Columns, mixed, options, run};
use zumen_base::{Kind, Limit, Limits, Stop, Value, codes};
use zumen_render::Error;
use zumen_scene::layout as l;

#[test]
fn order_outlines_clips_and_resource_diagnostics() {
    let bytes = mixed().pack();
    let (svg, diag, _) = run(&bytes, &options(), Limits::default(), false);
    let text = String::from_utf8(svg.unwrap()).unwrap();
    assert!(text.find("fill=\"#ff0000\"").unwrap() < text.find("fill=\"#0000ff\"").unwrap());
    assert!(text.contains("M0 50L100 50"));
    assert!(text.contains("fill-rule=\"nonzero\" d=\"M20 80L40 80L30 60Z"));
    assert!(text.contains("<g clip-path=\"url(#p0)\"><g clip-path=\"url(#p1)\">"));
    assert!(text.contains("M0 0H100V100H0ZM75 85L85 85L85 75L75 75Z"));
    assert!(!text.contains("script") && !text.contains("https:") && !text.contains("<text"));
    assert_eq!(
        diag.iter().map(|d| d.code()).collect::<Vec<_>>(),
        vec![
            codes::RENDER_TEXT_DROPPED,
            codes::RENDER_IMAGE_DROPPED,
            codes::RENDER_MARKER_DROPPED
        ]
    );
    assert_eq!(diag[0].places()[0].id(), Some("fixture-4"));
    assert_eq!(diag[0].param("unit"), Some(&Value::UInt(4)));
    assert_eq!(
        run(&bytes, &options(), Limits::default(), false),
        run(&bytes, &options(), Limits::default(), false)
    );
}

#[test]
fn budgets_stop_before_allocation_and_restore_clip_depth() {
    let bytes = mixed().pack();
    for kind in [
        Kind::InputBytes,
        Kind::Work,
        Kind::DecodedBytes,
        Kind::Elements,
        Kind::Segments,
        Kind::Depth,
    ] {
        let limits = Limits::default().with(kind, Limit::Finite(0));
        let first = run(&bytes, &options(), limits, false);
        assert!(
            matches!(first.0,Err(Error::Stopped(Stop::Exceeded(e))) if e.kind==kind),
            "{kind:?}: {first:?}"
        );
        assert_eq!(first, run(&bytes, &options(), limits, false));
    }
    let (result, _, used) = run(
        &bytes,
        &options(),
        Limits::default().with(Kind::DecodedBytes, Limit::Finite(4095)),
        false,
    );
    assert!(matches!(result, Err(Error::Stopped(_))));
    assert_eq!(
        used.iter()
            .find(|(k, _)| *k == Kind::DecodedBytes)
            .unwrap()
            .1,
        0
    );
    assert_eq!(
        run(&bytes, &options(), Limits::default(), true).0,
        Err(Error::Stopped(Stop::Cancelled))
    );
    let (_, _, used) = run(&bytes, &options(), Limits::default(), false);
    assert_eq!(used.iter().find(|(k, _)| *k == Kind::Depth).unwrap().1, 0);
}

#[test]
fn malformed_columns_references_and_floats_are_rejected() {
    let mut cases = vec![b"ZMSC".to_vec()];
    let mut c = mixed();
    c.u(l::LINE_UNITS, &[0]);
    cases.push(c.pack());
    let mut c = mixed();
    c.0[l::LINE_UNITS] = 999_u32.to_le_bytes().to_vec();
    cases.push(c.pack());
    let mut c = mixed();
    c.0[l::LINE_XY][..4].copy_from_slice(&f32::NAN.to_le_bytes());
    cases.push(c.pack());
    let mut c = mixed();
    c.0[l::TEXT_SECOND][..4].copy_from_slice(&0_f32.to_le_bytes());
    cases.push(c.pack());
    let mut c = mixed();
    c.0[l::ENTITY_IDS][4..8].copy_from_slice(&u32::MAX.to_le_bytes());
    cases.push(c.pack());
    let mut c = mixed().pack();
    c[16..20].copy_from_slice(&0_u32.to_le_bytes());
    cases.push(c);
    for input in cases {
        assert_eq!(
            run(&input, &options(), Limits::default(), false).0,
            Err(Error::Scene)
        );
    }
}

#[test]
fn nonascending_units_are_drawn_in_unit_order_without_changing_the_scene() {
    let bytes = fixtures::nonascending().pack();
    let original = bytes.clone();
    let (result, diag, _) = run(&bytes, &options(), Limits::default(), false);
    let text = String::from_utf8(result.unwrap()).unwrap();
    let red = text.find("fill=\"#ff0000\"").unwrap();
    let blue = text.find("fill=\"#0000ff\"").unwrap();
    let orange = text.find("fill=\"#ff8000\"").unwrap();
    assert!(red < blue && blue < orange, "{text}");
    assert_eq!(diag[0].places()[0].id(), Some("fixture-4"));
    assert_eq!(bytes, original);
}

#[test]
fn repeated_units_keep_kind_order_and_row_order() {
    let (result, _, _) = run(
        &fixtures::repeated_units().pack(),
        &options(),
        Limits::default(),
        false,
    );
    let text = String::from_utf8(result.unwrap()).unwrap();
    let line = text.find("M0 50L100 50").unwrap();
    let first_face = text.find("fill-rule=\"evenodd\"").unwrap();
    let second_face = text.rfind("fill-rule=\"evenodd\"").unwrap();
    let glyph = text.find("M20 80L40 80L30 60Z").unwrap();
    assert!(
        line < first_face && first_face < second_face && second_face < glyph,
        "{text}"
    );
    assert!(
        text.get(first_face..second_face)
            .unwrap()
            .contains("M40 60L60 60L60 40L40 40Z")
    );
}

#[test]
fn ordering_index_is_charged_before_allocation() {
    let bytes = fixtures::nonascending().pack();
    for (kind, limit) in [(Kind::Elements, 7), (Kind::DecodedBytes, 4096 + 8 * 12 - 1)] {
        let (result, _, used) = run(
            &bytes,
            &options(),
            Limits::default().with(kind, Limit::Finite(limit)),
            false,
        );
        assert!(matches!(result, Err(Error::Stopped(Stop::Exceeded(e))) if e.kind == kind));
        // 最初の SVG の容量だけを数え、8 要素分の索引には進まない。
        assert_eq!(
            used.iter()
                .find(|(k, _)| *k == Kind::DecodedBytes)
                .unwrap()
                .1,
            4096
        );
        assert_eq!(
            used.iter().find(|(k, _)| *k == Kind::Elements).unwrap().1,
            if kind == Kind::Elements { 0 } else { 8 }
        );
    }
    let (_, _, before) = run(
        &bytes,
        &options(),
        Limits::default().with(Kind::Elements, Limit::Finite(0)),
        false,
    );
    let work = before
        .iter()
        .find(|(kind, _)| *kind == Kind::Work)
        .unwrap()
        .1;
    let (result, diag, used) = run(
        &bytes,
        &options(),
        Limits::default().with(Kind::Work, Limit::Finite(work + 9)),
        false,
    );
    // 索引を埋めた後の並べ替えも、小さい残り work で止まる。
    assert!(matches!(result, Err(Error::Stopped(Stop::Exceeded(e))) if e.kind == Kind::Work));
    assert_eq!(
        used.iter()
            .find(|(k, _)| *k == Kind::DecodedBytes)
            .unwrap()
            .1,
        4096 + 8 * 12
    );
    assert_eq!(diag.len(), 1);
}

#[test]
fn legacy_glyph_columns_and_new_sections_keep_the_scene_contract() {
    let mut c = mixed();
    c.0.truncate(l::TEXT_GLYPHS);
    assert!(
        run(&c.pack(), &options(), Limits::default(), false)
            .0
            .is_ok()
    );
    let mut c = mixed();
    c.0.push(vec![1, 2, 3]);
    assert!(
        run(&c.pack(), &options(), Limits::default(), false)
            .0
            .is_ok()
    );
}

#[test]
fn extreme_coordinates_keep_small_chunk_offsets_and_reject_overflow() {
    let mut c = mixed();
    c.0[l::CHUNK_ORIGINS] = [1e16_f64.to_le_bytes(), 1e16_f64.to_le_bytes()].concat();
    let mut o = options();
    o.region = [1e16, 1e16, 1e16 + 100.0, 1e16 + 100.0];
    let svg = String::from_utf8(run(&c.pack(), &o, Limits::default(), false).0.unwrap()).unwrap();
    assert!(svg.contains("M20 80L40 80L30 60Z"));
    o.region = [-f64::MAX, 0.0, f64::MAX, 1.0];
    assert_eq!(
        run(&c.pack(), &o, Limits::default(), false).0,
        Err(Error::Options)
    );
    o = options();
    o.dpi = f64::MAX;
    c.0[l::UNIT_WEIGHTS] = [f32::MAX; 8].iter().flat_map(|f| f.to_le_bytes()).collect();
    assert_eq!(
        run(&c.pack(), &o, Limits::default(), false).0,
        Err(Error::Options)
    );
}

#[test]
fn empty_scene_and_explicit_options() {
    let input = Columns::new().pack();
    let mut o = options();
    o.width = 200;
    o.height = 100;
    o.background = [1, 2, 3, 128];
    let text = String::from_utf8(run(&input, &o, Limits::default(), false).0.unwrap()).unwrap();
    assert!(text.contains("width=\"200\" height=\"100\""));
    assert!(text.contains("fill=\"#010203\" fill-opacity=\"0.5019607843137255\""));
    for region in [[0.0; 4], [0.0, 0.0, f64::NAN, 1.0], [2.0, 0.0, 1.0, 1.0]] {
        o.region = region;
        assert_eq!(
            run(&input, &o, Limits::default(), false).0,
            Err(Error::Options)
        );
    }
}

#[test]
fn explicit_foreground_opacity_hairline_and_centered_region() {
    let mut c = mixed();
    c.0[l::COLOR_FLAGS][1] = l::color_flag::UNRESOLVED;
    c.0[l::UNIT_WEIGHTS][4..8].copy_from_slice(&0_f32.to_le_bytes());
    let mut o = options();
    o.width = 200;
    o.dpi = 254.0;
    o.foreground = [18, 52, 86, 128];
    let (result, diag, _) = run(&c.pack(), &o, Limits::default(), false);
    let svg = String::from_utf8(result.unwrap()).unwrap();
    let line = svg
        .lines()
        .find(|line| line.contains("M50 50L150 50"))
        .unwrap();
    assert!(
        line.contains("#123456")
            && line.contains("stroke-width=\"1\"")
            && line.contains("stroke-opacity=\"0.5019607843137255\"")
    );
    assert!(!svg.contains(" opacity="));
    assert!(svg.contains("<rect x=\"50\" y=\"0\" width=\"100\" height=\"100\"/></clipPath>"));
    assert!(
        svg.find("<rect width=\"200\" height=\"100\" fill=")
            .unwrap()
            < svg.find("<g clip-path=\"url(#viewport)\">").unwrap()
    );
    let color = diag
        .iter()
        .find(|d| d.code() == codes::RENDER_COLOR_ASSUMED)
        .unwrap();
    assert_eq!(color.places()[0].id(), Some("fixture-1"));
    assert_eq!(color.count(), 1);
}

#[test]
fn sdk_fuzz_and_id_contracts() {
    let bytes = mixed().pack();
    zumen_sdk::testkit::survives_with(&bytes, |input, cx| {
        zumen_render::svg(input, &options(), cx).map_err(|e| match e {
            Error::Stopped(s) => zumen_sdk::Error::Stopped(s),
            _ => zumen_sdk::Error::Rejected,
        })
    })
    .unwrap();
    zumen_sdk::testkit::ids_match(&bytes, "fixture-4\nfixture-5\nfixture-6", |input| {
        Ok(run(input, &options(), Limits::default(), false)
            .1
            .iter()
            .flat_map(|d| d.places())
            .filter_map(|p| p.id().map(str::to_owned))
            .collect())
    })
    .unwrap();
}

#[test]
fn accepts_scene_built_from_a_drawing_with_small_chunks() {
    use zumen_base::{Budget, CancelToken, Cx, Diagnostics};
    use zumen_sdk::Lift;
    let input=b"0\nSECTION\n2\nENTITIES\n0\nLINE\n5\n1\n8\n0\n420\n255\n10\n0\n20\n0\n30\n0\n11\n100\n21\n100\n31\n0\n0\nTEXT\n5\n2\n8\n0\n10\n20\n20\n20\n30\n0\n40\n5\n1\nlabel\n0\nENDSEC\n0\nEOF\n";
    let file = zumen_sdk::testkit::read::<zumen_acad::Dxf>(input)
        .0
        .unwrap();
    let mut budget = Budget::default();
    let mut diag = Diagnostics::new();
    let cancel = CancelToken::new();
    let mut cx = Cx::new(&mut budget, &mut diag, &cancel);
    let drawing: zumen_drawing::Drawing2D = file.lift(&mut cx).unwrap();
    assert!(!drawing.spaces[0].elements.is_empty(), "{drawing:?}");
    let scene = zumen_scene::build(
        &drawing,
        &zumen_scene::Options {
            origin: zumen_drawing::Origin::Space(zumen_drawing::SpaceId(0)),
            tolerance: Some(0.01),
            print: false,
            region: None,
            palette: zumen_scene::Palette::default(),
            chunking: zumen_scene::Chunking {
                items: 1,
                ..zumen_scene::Chunking::PROVISIONAL
            },
        },
        &mut cx,
    )
    .unwrap();
    assert_eq!(scene.counts.chunks, 2);
    let (result, diag, _) = run(&scene.bytes, &options(), Limits::default(), false);
    let text = String::from_utf8(result.unwrap()).unwrap();
    assert!(text.contains("M0 100L100 0"), "{text}");
    assert!(text.contains("#0000ff"));
    assert_eq!(
        diag.iter()
            .filter(|d| d.code() == codes::RENDER_TEXT_DROPPED)
            .count(),
        1
    );
    assert_eq!(
        diag.iter()
            .find(|d| d.code() == codes::RENDER_TEXT_DROPPED)
            .unwrap()
            .places()[0]
            .id(),
        Some("space/0/h2")
    );
}

#[test]
fn all_prefixes_and_changed_bytes_finish_with_small_budgets() {
    let source = mixed().pack();
    let limits = Limits::default()
        .with(Kind::Work, Limit::Finite(2048))
        .with(Kind::DecodedBytes, Limit::Finite(16384));
    for end in 0..source.len() {
        let _ = run(&source[..end], &options(), limits, false);
    }
    for index in 0..source.len() {
        let mut bytes = source.clone();
        bytes[index] ^= 0xff;
        assert_eq!(
            run(&bytes, &options(), limits, false),
            run(&bytes, &options(), limits, false)
        );
    }
}

#[cfg(test)]
fn drawing_from_dxf(entities: &str) -> zumen_drawing::Drawing2D {
    use zumen_base::{Budget, CancelToken, Cx, Diagnostics};
    use zumen_sdk::Lift;
    let input = format!("0\nSECTION\n2\nENTITIES\n{entities}0\nENDSEC\n0\nEOF\n");
    let file = zumen_sdk::testkit::read::<zumen_acad::Dxf>(input.as_bytes())
        .0
        .unwrap();
    file.lift(&mut Cx::new(
        &mut Budget::default(),
        &mut Diagnostics::new(),
        &CancelToken::new(),
    ))
    .unwrap()
}

#[cfg(test)]
fn build_scene(drawing: &zumen_drawing::Drawing2D, region: Option<[f64; 4]>) -> zumen_scene::Scene {
    use zumen_base::{Budget, CancelToken, Cx, Diagnostics};
    zumen_scene::build(
        drawing,
        &zumen_scene::Options {
            origin: zumen_drawing::Origin::Space(zumen_drawing::SpaceId(0)),
            tolerance: Some(0.01),
            print: false,
            region,
            palette: zumen_scene::Palette::default(),
            chunking: zumen_scene::Chunking::PROVISIONAL,
        },
        &mut Cx::new(
            &mut Budget::default(),
            &mut Diagnostics::new(),
            &CancelToken::new(),
        ),
    )
    .unwrap()
}

#[cfg(test)]
fn scene_u32s(bytes: &[u8], section: usize) -> Vec<u32> {
    let at = 16 + 8 * section;
    let start = u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap()) as usize;
    let len = u32::from_le_bytes(bytes[at + 4..at + 8].try_into().unwrap()) as usize;
    bytes[start..start + len]
        .as_chunks::<4>()
        .0
        .iter()
        .map(|b| u32::from_le_bytes(*b))
        .collect()
}

#[test]
fn mtext_reused_color_units_from_dxf_keep_ids_and_all_text_runs() {
    let drawing =
        drawing_from_dxf("0\nMTEXT\n5\n1\n8\n0\n10\n10\n20\n20\n30\n0\n40\n5\n1\nA{\\C1;B}C\n");
    let scene = build_scene(&drawing, None);
    assert_eq!(scene_u32s(&scene.bytes, l::TEXT_UNITS), [0, 1, 0]);
    let (result, diag, _) = run(&scene.bytes, &options(), Limits::default(), false);
    assert!(result.is_ok(), "{result:?}");
    // フォントを渡していない 3 区間を、元の ID と unit ごとの件数で診断する。
    assert_eq!(diag.len(), 2);
    for (diagnostic, unit, count) in [(&diag[0], 0, 2), (&diag[1], 1, 1)] {
        assert_eq!(diagnostic.code(), codes::RENDER_TEXT_DROPPED);
        assert_eq!(diagnostic.param("unit"), Some(&Value::UInt(unit)));
        assert_eq!(diagnostic.count(), count);
        assert_eq!(diagnostic.places()[0].id(), Some("space/0/h1"));
    }
}

#[test]
fn region_compaction_of_text_then_dimension_keeps_ids_and_draws_lines() {
    use zumen_drawing::{
        Dimension, DimensionKind, DimensionText, Format, Label, Length, ModelLength, ModelPoint,
        Piece, Scaled, Shape, SpaceId, Span,
    };
    let mut drawing = drawing_from_dxf(
        "0\nTEXT\n5\n1\n8\n0\n10\n20\n20\n20\n30\n0\n40\n5\n1\nlabel\n0\nLINE\n5\n2\n8\n0\n10\n0\n20\n0\n30\n0\n11\n10\n21\n0\n31\n0\n",
    );
    let at = |x, y| ModelPoint {
        space: SpaceId(0),
        x,
        y,
    };
    drawing.spaces[0].elements[1].shape = Shape::Dimension(Dimension {
        kind: DimensionKind::Linear {
            first: at(0.0, 0.0),
            second: at(10.0, 0.0),
            line: at(0.0, 5.0),
            rotation: 0.0,
            oblique: 0.0,
        },
        measurement: None,
        label: Label::Override {
            content: "10".into(),
            spans: vec![Span {
                piece: Piece::Text("10".into()),
                format: Format {
                    height: Some(Scaled::Value(Length::Model(ModelLength {
                        space: SpaceId(0),
                        value: 2.5,
                    }))),
                    ..Format::default()
                },
            }],
        },
        text: DimensionText {
            position: None,
            moved: false,
            rotation: None,
            attachment: None,
        },
        style: String::new(),
        block: None,
    });
    let scene = build_scene(&drawing, Some([-10.0, -10.0, 100.0, 100.0]));
    assert_eq!(scene_u32s(&scene.bytes, l::LINE_UNITS), [0, 0, 0]);
    assert_eq!(scene_u32s(&scene.bytes, l::TEXT_UNITS), [1, 0]);
    let (result, diag, _) = run(&scene.bytes, &options(), Limits::default(), false);
    let text = String::from_utf8(result.unwrap()).unwrap();
    assert_eq!(text.matches("stroke-width=").count(), 3);
    assert_eq!(diag.len(), 3, "{diag:?}");
    assert_eq!(diag[0].code(), codes::RENDER_COLOR_ASSUMED);
    assert_eq!(diag[0].count(), 3);
    assert_eq!(diag[0].param("unit"), Some(&Value::UInt(0)));
    assert_eq!(diag[0].places()[0].id(), Some("space/0/h2"));
    for (diagnostic, unit, id) in [(&diag[1], 0, "space/0/h2"), (&diag[2], 1, "space/0/h1")] {
        assert_eq!(diagnostic.code(), codes::RENDER_TEXT_DROPPED);
        assert_eq!(diagnostic.param("unit"), Some(&Value::UInt(unit)));
        assert_eq!(diagnostic.count(), 1);
        assert_eq!(diagnostic.places()[0].id(), Some(id));
    }
}

#[test]
fn repeated_shared_glyphs_charge_each_rendered_vertex() {
    let mut c = mixed();
    c.0[l::TEXT_GLYPHS] = [0_u32, 1000]
        .into_iter()
        .flat_map(u32::to_le_bytes)
        .collect();
    c.0[l::GLYPH_SHAPES] = vec![0; 4000];
    c.0[l::GLYPH_AXES] = [1_f32, 0.0, 0.0, 1.0]
        .into_iter()
        .flat_map(f32::to_le_bytes)
        .cycle()
        .take(16000)
        .collect();
    c.0[l::GLYPH_XY] = vec![0; 8000];
    let (result, _, used) = run(
        &c.pack(),
        &options(),
        Limits::default().with(Kind::Segments, Limit::Finite(30)),
        false,
    );
    assert!(matches!(result,Err(Error::Stopped(Stop::Exceeded(e))) if e.kind==Kind::Segments));
    assert_eq!(
        used.iter()
            .find(|(kind, _)| *kind == Kind::Depth)
            .unwrap()
            .1,
        0
    );
}
