//! レビューの JSON と直接作った Rust の値から別のシーンを作る（#589、決定 0295）。
//! 有効な 4 種類の図形を種として毎回作り、座標・引数・ID・参照を入力で変える。
//! シーンのバイト・出所・診断・使用量の再現、低い予算と中断、渡したフォントも確かめる。

#![no_main]

use std::cell::RefCell;
use std::collections::BTreeSet;

use libfuzzer_sys::fuzz_target;
use zumen_base::{
    Budget, CancelToken, Cx, Diagnostic, Diagnostics, Kind, Limit, Limits, Ref, Stop,
};
use zumen_text::font::Font;
use zumen_drawing::{
    Affine, Axes, Drawing2D, ModelSpace, Origin, Sheet, SheetId, SpaceId, Unit, View,
};
use zumen_review::{Location, Markup, MarkupKind, Place, Review, Status, Target, Topic};
use zumen_scene::{
    Chunking, FontSet, ReviewArrows, ReviewError, ReviewOptions, ReviewScene, ReviewStyle,
    build_review, build_review_with,
};
use zumen_sdk::{Error, testkit};

type Outcome = (
    Result<ReviewScene, (ReviewError, Ref)>,
    Budget,
    Vec<Diagnostic>,
);

fn drawing() -> Drawing2D {
    Drawing2D {
        spaces: [Unit::Meter, Unit::Unknown]
            .into_iter()
            .enumerate()
            .map(|(i, unit)| ModelSpace {
                id: SpaceId(i as u32),
                unit,
                axes: Axes::EastNorth,
                elements: Vec::new(),
                order: Vec::new(),
            })
            .collect(),
        sheets: vec![Sheet {
            id: SheetId(0),
            name: None,
            size: None,
            elements: Vec::new(),
            views: vec![View {
                space: SpaceId(0),
                sheet: SheetId(0),
                transform: Affine {
                    m: [2.0, 0.0, 10.0, 0.0, 3.0, 20.0],
                },
                clip: Vec::new(),
                hidden_layers: Vec::new(),
                source: Ref::default(),
            }],
            order: Vec::new(),
            source: Ref::default(),
        }],
        ..Drawing2D::default()
    }
}

fn options(origin: Origin) -> ReviewOptions {
    ReviewOptions {
        origin,
        tolerance: 0.02,
        region: None,
        chunking: Chunking {
            items: 16,
            span: 65536.0,
        },
        style: ReviewStyle {
            color: [220, 10, 30],
            line_weight: 0.25,
            text_height: 2.0,
            text_padding: 0.5,
            arrow_length: 1.0,
            arrows: ReviewArrows::Both,
            cloud_pitch: 4.0,
            marker_size: 3.0,
            font: Some("fuzz".into()),
        },
    }
}

fn target() -> Target {
    Target {
        id: "plan".into(),
        format: "dxf".into(),
        hash: format!("v2:{}", "0".repeat(64)),
        name: None,
        extra: Vec::new(),
    }
}

fn byte(data: &[u8], at: usize) -> u8 {
    data.get(at).copied().unwrap_or(0)
}

fn number(data: &[u8], at: usize) -> f64 {
    let mut bytes = [0; 8];
    for (i, output) in bytes.iter_mut().enumerate() {
        *output = byte(data, at + i);
    }
    f64::from_bits(u64::from_le_bytes(bytes))
}

/// JSON では表せない値も渡す。作る項目と文字の量は、入力長によらず小さく抑える。
fn generated(data: &[u8], mutate: bool) -> (Review, Target, ReviewOptions) {
    let mut options = options(match if mutate { byte(data, 0) % 4 } else { 0 } {
        1 => Origin::Space(SpaceId(1)),
        2 => Origin::Sheet(SheetId(0)),
        3 => Origin::View {
            sheet: SheetId(0),
            view: 0,
        },
        _ => Origin::Space(SpaceId(0)),
    });
    let view = match options.origin {
        Origin::Space(SpaceId(1)) => "space/1",
        Origin::Sheet(_) => "sheet/0",
        Origin::View { .. } => "sheet/0/view/0",
        _ => "space/0",
    };
    let text = format!(
        "H\n{}",
        String::from_utf8_lossy(&data[..data.len().min(128)])
    );
    let mut review = Review {
        targets: vec![target()],
        markups: [
            (MarkupKind::Redline, vec![[0.0, 0.0], [12.0, 8.0]]),
            (
                MarkupKind::Cloud,
                vec![[0.0, 0.0], [10.0, 0.0], [10.0, 8.0], [0.0, 8.0]],
            ),
            (MarkupKind::Text, vec![[3.0, 4.0]]),
            (MarkupKind::Photo, vec![[9.0, 10.0]]),
        ]
        .into_iter()
        .enumerate()
        .map(|(i, (kind, points))| Markup {
            // 図形と場所に同じ ID を使い、種類の名前空間による区別も通す。
            id: format!("item/{i}"),
            kind,
            target: "plan".into(),
            view: Some(view.into()),
            points,
            text: Some(text.clone()),
            topic: None,
            carried: None,
            extra: Vec::new(),
        })
        .collect(),
        topics: vec![Topic {
            id: "11111111-1111-4111-8111-111111111111".into(),
            title: "synthetic".into(),
            description: None,
            status: Status::Open,
            status_label: None,
            kind: None,
            priority: None,
            labels: Vec::new(),
            assignee: None,
            due: None,
            author: None,
            created: None,
            modified_by: None,
            modified: None,
            related: Vec::new(),
            comments: Vec::new(),
            locations: vec![Location {
                id: "item/0".into(),
                target: "plan".into(),
                place: Place::Plane {
                    view: Some(view.into()),
                    pin: Some([4.0, 5.0]),
                    extent: Some([1.0, 2.0, 8.0, 9.0]),
                    elements: Vec::new(),
                },
                carried: None,
                extra: Vec::new(),
            }],
            origin: None,
            extra: Vec::new(),
        }],
        extra: Vec::new(),
    };
    let mut target = target();
    if mutate {
        let value = match byte(data, 3) % 8 {
            1 => f64::NAN,
            2 => f64::INFINITY,
            3 => f64::NEG_INFINITY,
            4 => f64::MAX,
            5 => f64::MIN_POSITIVE,
            6 => -1.0,
            _ => number(data, 8),
        };
        match byte(data, 1) % 8 {
            1 => {
                let points = &mut review.markups[usize::from(byte(data, 2) % 4)].points;
                points[0] = [value, number(data, 16)];
                if byte(data, 3) & 1 != 0 {
                    points.clear();
                }
            }
            2 => match byte(data, 2) % 10 {
                0 => options.tolerance = value,
                1 => options.style.line_weight = value,
                2 => options.style.text_height = value,
                3 => options.style.text_padding = value,
                4 => options.style.arrow_length = value,
                5 => options.style.cloud_pitch = value,
                6 => options.style.marker_size = value,
                7 => options.chunking.span = value,
                8 => options.chunking.items = 0,
                _ => options.region = Some([value, number(data, 16), 1.0, 2.0]),
            },
            3 => review.markups[1].id = review.markups[0].id.clone(),
            4 => review.markups[0].view = Some("missing/view".into()),
            5 => target.hash.push('x'),
            6 => review.markups[0].topic = Some("missing/topic".into()),
            7 => options.origin = Origin::Space(SpaceId(99)),
            _ => {}
        }
    }
    (review, target, options)
}

fn checked(result: &Result<ReviewScene, (ReviewError, Ref)>) {
    if let Ok(result) = result {
        zumen::scene::read_scene(&result.scene.bytes).expect("生成したシーンを読み直せない");
        let ids: BTreeSet<_> = result
            .sources
            .iter()
            .map(|source| &source.scene_id)
            .collect();
        assert_eq!(ids.len(), result.sources.len(), "出所の ID が重なった");
    }
}

fn run(
    drawing: &Drawing2D,
    review: &Review,
    target: &Target,
    options: &ReviewOptions,
    fonts: Option<&FontSet<'_>>,
    limits: Limits,
    cancelled: bool,
) -> Outcome {
    let mut budget = Budget::new(limits);
    let mut diagnostics = Diagnostics::new();
    let cancel = CancelToken::new();
    if cancelled {
        cancel.cancel();
    }
    let mut cx = Cx::new(&mut budget, &mut diagnostics, &cancel);
    let result = match fonts {
        Some(fonts) => build_review_with(drawing, review, target, options, fonts, &mut cx),
        None => build_review(drawing, review, target, options, &mut cx),
    };
    checked(&result);
    (result, budget, diagnostics.into_vec())
}

fuzz_target!(|data: &[u8]| {
    let drawing = drawing();
    let previous = RefCell::new(None);
    // JSON の読みとシーンの生成を、同じ予算で 2 回通す。
    zumen_sdk::testkit::survives_with(data, |src, cx| {
        let review = Review::read(src, cx)?;
        let target = review.targets.first().cloned().unwrap_or_else(target);
        let result = build_review(
            &drawing,
            &review,
            &target,
            &options(Origin::Space(SpaceId(0))),
            cx,
        );
        checked(&result);
        if let Some(first) = previous.borrow().as_ref() {
            assert_eq!(first, &result);
        }
        let status = result
            .as_ref()
            .map(|_| ())
            .map_err(|(error, _)| match error {
                ReviewError::Stopped(stop) => Error::Stopped(*stop),
                ReviewError::Invalid => Error::Rejected,
            });
        *previous.borrow_mut() = Some(result);
        status
    })
    .unwrap();

    let (review, target, options) = generated(data, true);
    let limits = testkit::fuzz_call_limits();
    let ordinary = run(&drawing, &review, &target, &options, None, limits, false);
    // 空のフォントの明示と省略は、バイト・出所・診断・使用量まで一致する。
    assert_eq!(
        ordinary,
        run(
            &drawing,
            &review,
            &target,
            &options,
            Some(&FontSet::default()),
            limits,
            false
        )
    );
    let kind = [
        Kind::Work,
        Kind::DecodedBytes,
        Kind::Elements,
        Kind::Segments,
    ][usize::from(byte(data, 4) % 4)];
    let small = limits.with(kind, Limit::Finite(u64::from(byte(data, 5))));
    assert_eq!(
        run(&drawing, &review, &target, &options, None, small, false),
        run(&drawing, &review, &target, &options, None, small, false),
    );
    let cancelled = run(&drawing, &review, &target, &options, None, limits, true);
    assert!(matches!(
        cancelled.0,
        Err((ReviewError::Stopped(Stop::Cancelled), _))
    ));
    assert_eq!(cancelled.1.used(Kind::Work), 0);

    // フォントの種は JSON として読めなくても、H を描く有効なレビューで字形の生成へ進む。
    if let Ok(face) = Font::parse(data, 0) {
        let mut fonts = FontSet::default();
        fonts.add(Some("fuzz"), face);
        let (mut review, target, options) = generated(data, false);
        review.topics.clear();
        review
            .markups
            .retain(|markup| markup.kind == MarkupKind::Text);
        review.markups[0].text = Some("H".into());
        // 1 文字の輪郭を作る事前の予約（2^24）を通す。文字数は 1 に固定し、
        // メモリと幾何の分割の上限は上げない。
        let font_limits = limits.with(Kind::Work, Limit::Finite(1 << 26));
        assert_eq!(
            run(
                &drawing,
                &review,
                &target,
                &options,
                Some(&fonts),
                font_limits,
                false
            ),
            run(
                &drawing,
                &review,
                &target,
                &options,
                Some(&fonts),
                font_limits,
                false
            ),
        );
    }
});
