//! PNG の画素、途中停止、容量・ID の契約。外部の読み取りは browser.mjs でも照らす。
mod fixtures;

use fixtures::{Columns, mixed, options, pixels, run_png};
use zumen_base::{Kind, Limit, Limits, Stop, codes};
use zumen_render::Error;
use zumen_scene::layout as l;

#[allow(clippy::unwrap_used)] // 契約テストで、期待する位置の画素がなければ失敗させる。
fn pixel(bytes: &[u8], x: usize, y: usize) -> [u8; 4] {
    let (width, _, rgba) = pixels(bytes);
    rgba.get((y * width + x) * 4..(y * width + x + 1) * 4)
        .unwrap()
        .try_into()
        .unwrap()
}

#[test]
fn order_holes_glyphs_clips_and_diagnostics_match_svg_contract() {
    let input = mixed().pack();
    let (result, diag, _) = run_png(&input, &options(), Limits::default(), false);
    let png = result.unwrap();
    for (x, y, expected) in [
        (5, 5, [255, 0, 0, 255]),
        (50, 45, [255; 4]),
        (50, 50, [0, 0, 255, 255]),
        (30, 75, [0, 160, 0, 255]),
        (70, 70, [255, 128, 0, 255]),
        (80, 80, [255, 0, 0, 255]),
        (97, 80, [255, 0, 0, 255]),
    ] {
        assert_eq!(pixel(&png, x, y), expected, "{x},{y}");
    }
    assert_eq!(
        diag,
        fixtures::run(&input, &options(), Limits::default(), false).1
    );
    assert_eq!(diag[0].places()[0].id(), Some("fixture-4"));
    assert_eq!(
        run_png(&input, &options(), Limits::default(), false)
            .0
            .unwrap(),
        png
    );
    let (result, _, _) = run_png(
        &fixtures::nonascending().pack(),
        &options(),
        Limits::default(),
        false,
    );
    assert_eq!(pixel(&result.unwrap(), 50, 45), [255, 0, 0, 255]);
}

#[test]
fn region_margin_and_large_origins_use_the_same_camera() {
    let mut o = options();
    o.width = 200;
    o.region = [20.0, 20.0, 80.0, 80.0];
    let png = run_png(&mixed().pack(), &o, Limits::default(), false)
        .0
        .unwrap();
    for (x, c) in [(25, [255; 4]), (100, [0, 0, 255, 255]), (175, [255; 4])] {
        assert_eq!(pixel(&png, x, 50), c);
    }
    let mut c = mixed();
    c.0[l::UNIT_CLIPS].fill(255);
    let plain = run_png(&c.pack(), &options(), Limits::default(), false)
        .0
        .unwrap();
    c.0[l::CHUNK_ORIGINS].clear();
    c.d(l::CHUNK_ORIGINS, &[1e15, 1e15]);
    let mut o = options();
    o.region = [1e15, 1e15, 1e15 + 100.0, 1e15 + 100.0];
    assert_eq!(
        run_png(&c.pack(), &o, Limits::default(), false).0.unwrap(),
        plain
    );
}

#[test]
fn extreme_dimensions_stop_before_allocating_pixels() {
    let bytes = Columns::new().pack();
    for (width, height) in [
        (0, 1),
        (1, 0),
        (u32::MAX, u32::MAX),
        (16_777_217, 1),
        (4096, 4097),
    ] {
        let mut o = options();
        o.width = width;
        o.height = height;
        let (r, _, used) = run_png(&bytes, &o, Limits::default(), false);
        assert_eq!(r, Err(Error::Options));
        assert_eq!(
            used.iter()
                .find(|(k, _)| *k == Kind::DecodedBytes)
                .unwrap()
                .1,
            0
        );
    }
    let mut o = options();
    o.width = 4096;
    o.height = 4096;
    let (r, diag, used) = run_png(
        &bytes,
        &o,
        Limits::default().with(Kind::DecodedBytes, Limit::Finite(1024)),
        false,
    );
    assert!(matches!(r,Err(Error::Stopped(Stop::Exceeded(e))) if e.kind==Kind::DecodedBytes));
    assert_eq!(
        diag.last().unwrap().code(),
        codes::BUDGET_DECODED_BYTES_EXCEEDED
    );
    assert_eq!(
        used.iter()
            .find(|(k, _)| *k == Kind::DecodedBytes)
            .unwrap()
            .1,
        0
    );
}

#[test]
fn every_budget_and_mid_render_stop_is_atomic() {
    let bytes = mixed().pack();
    let (_, _, used) = run_png(&bytes, &options(), Limits::default(), false);
    for kind in [
        Kind::InputBytes,
        Kind::DecodedBytes,
        Kind::Work,
        Kind::Elements,
        Kind::Segments,
        Kind::Depth,
    ] {
        let amount = used.iter().find(|(k, _)| *k == kind).unwrap().1;
        for limit in [0, amount / 2, amount.saturating_sub(1)] {
            if kind == Kind::Depth && limit != 0 {
                continue;
            }
            let limits = Limits::default().with(kind, Limit::Finite(limit));
            let first = run_png(&bytes, &options(), limits, false);
            assert!(
                matches!(first.0,Err(Error::Stopped(Stop::Exceeded(e))) if e.kind==kind),
                "{kind:?}:{limit}"
            );
            assert_eq!(first, run_png(&bytes, &options(), limits, false));
            assert_eq!(
                first.2.iter().find(|(k, _)| *k == Kind::Depth).unwrap().1,
                0
            );
        }
    }
    assert_eq!(
        run_png(&bytes, &options(), Limits::default(), true).0,
        Err(Error::Stopped(Stop::Cancelled))
    );
    let limits = used.iter().fold(Limits::default(), |l, (k, v)| {
        if *k == Kind::Depth {
            l
        } else {
            l.with(*k, Limit::Finite(*v))
        }
    });
    assert!(run_png(&bytes, &options(), limits, false).0.is_ok());
}

#[test]
fn transparent_background_blocks_and_resource_only_scenes() {
    let mut o = options();
    o.width = 257;
    o.height = 80;
    o.background = [40, 80, 160, 128];
    let png = run_png(&Columns::new().pack(), &o, Limits::default(), false)
        .0
        .unwrap();
    let (w, h, p) = pixels(&png);
    assert_eq!((w, h), (257, 80));
    assert!(p.as_chunks::<4>().0.iter().all(|v| *v == o.background));
    o.width = 1;
    o.height = 1;
    o.background = [255, 128, 64, 0];
    assert_eq!(
        pixel(
            &run_png(&Columns::new().pack(), &o, Limits::default(), false)
                .0
                .unwrap(),
            0,
            0
        ),
        [0; 4]
    );
}

#[test]
fn malformed_scene_and_out_of_range_geometry_fail_with_diagnostics() {
    let input = mixed().pack();
    for end in [0, 4, 16, input.len() - 1] {
        assert!(
            run_png(
                input.get(..end).unwrap(),
                &options(),
                Limits::default(),
                false
            )
            .0
            .is_err()
        );
    }
    let mut c = mixed();
    c.0[l::LINE_XY].clear();
    c.f(l::LINE_XY, &[0.0, 0.0, f32::MAX, 0.0]);
    let result = run_png(&c.pack(), &options(), Limits::default(), false);
    assert_eq!(result.0, Err(Error::Options));
    assert_eq!(
        result.1.last().unwrap().code(),
        codes::RENDER_OPTIONS_INVALID
    );
}

#[test]
fn sdk_fuzz_contract_covers_valid_and_mutated_scenes() {
    let input = mixed().pack();
    let mut o = options();
    o.width = 16;
    o.height = 16;
    let previous = std::cell::RefCell::new(None);
    zumen_sdk::testkit::survives_with(&input, |src, cx| {
        let result = zumen_render::png(src, &o, cx);
        if let Some(first) = previous.borrow().as_ref() {
            assert_eq!(first, &result);
        }
        let status = result.as_ref().map(|_| ()).map_err(|e| match e {
            Error::Stopped(stop) => zumen_sdk::Error::Stopped(*stop),
            _ => zumen_sdk::Error::Rejected,
        });
        *previous.borrow_mut() = Some(result);
        status
    })
    .unwrap();
    assert!(previous.into_inner().unwrap().is_ok());
    let limits = Limits::default()
        .with(Kind::Work, Limit::Finite(4096))
        .with(Kind::DecodedBytes, Limit::Finite(65536));
    for at in (0..input.len()).step_by(17) {
        let _ = run_png(input.get(..at).unwrap(), &o, limits, false);
        let mut changed = input.clone();
        *changed.get_mut(at).unwrap() ^= 0xff;
        assert_eq!(
            run_png(&changed, &o, limits, false),
            run_png(&changed, &o, limits, false)
        );
    }
}
