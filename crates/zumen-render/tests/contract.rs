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
fn malformed_columns_references_floats_and_order_are_rejected() {
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
    c.0[l::FACE_UNITS] = [7_u32.to_le_bytes(), 0_u32.to_le_bytes()].concat();
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
