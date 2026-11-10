//! レビューの図形を別のシーンにし、対象・起点・出所・予算を保つ（決定 0295）。

use std::collections::BTreeSet;

use zumen_base::{
    Budget, CancelToken, Cx, Diagnostic, Diagnostics, Key, Kind, Limit, Limits, Ref, Stop, Value,
};
use zumen_drawing::font::Font as Face;
use zumen_drawing::{
    Affine, Axes, Color, Drawing2D, Element, Inherit, Layer, LayerId, ModelPoint, ModelSpace,
    Origin, Shape, Sheet, SheetId, SpaceId, Style, Unit, View,
};
use zumen_review::{Comment, Location, Markup, MarkupKind, Place, Review, Status, Target, Topic};
use zumen_scene::layout;
use zumen_scene::{
    Chunking, FontSet, Options, Palette, ReviewArrows, ReviewError, ReviewOptions, ReviewScene,
    ReviewSourceKind, ReviewStyle, build, build_review, build_review_with,
};

const MODEL: SpaceId = SpaceId(0);
const TOPIC: &str = "11111111-1111-4111-8111-111111111111";

fn target() -> Target {
    Target {
        id: "plan".into(),
        format: "dxf".into(),
        hash: zumen_drawing::Hash::document(b"synthetic review drawing").to_string(),
        name: None,
        extra: Vec::new(),
    }
}

fn drawing() -> Drawing2D {
    let sheet = |id| Sheet {
        id: SheetId(id),
        name: None,
        size: None,
        elements: Vec::new(),
        views: vec![View {
            space: MODEL,
            sheet: SheetId(id),
            transform: Affine {
                m: [2.0, 0.0, 100.0, 0.0, 3.0, 200.0],
            },
            clip: Vec::new(),
            hidden_layers: Vec::new(),
            source: Ref::default(),
        }],
        order: Vec::new(),
        // 同じハンドルでも、確定した用紙の ID とそのビューは区別する。
        source: Ref::default().with_key(Key::Handle(0x22)),
    };
    Drawing2D {
        spaces: vec![
            ModelSpace {
                id: MODEL,
                unit: Unit::Meter,
                axes: Axes::EastNorth,
                elements: vec![Element {
                    source: Ref::at_record(0).with_key(Key::Handle(0xA1)),
                    layer: LayerId(0),
                    style: Style {
                        color: Inherit::Own(Color {
                            code: None,
                            rgb: Some([0, 0, 0]),
                        }),
                        ..Style::BLOCK
                    },
                    shape: Shape::Line {
                        start: ModelPoint {
                            space: MODEL,
                            x: 0.0,
                            y: 0.0,
                        },
                        end: ModelPoint {
                            space: MODEL,
                            x: 1.0,
                            y: 2.0,
                        },
                    },
                }],
                order: Vec::new(),
            },
            ModelSpace {
                id: SpaceId(1),
                unit: Unit::Unknown,
                axes: Axes::EastNorth,
                elements: Vec::new(),
                order: Vec::new(),
            },
        ],
        sheets: vec![sheet(0), sheet(1)],
        layers: vec![Layer {
            id: LayerId(0),
            name: "0".into(),
            state: None,
            style: None,
            follows_placement: false,
            description: None,
            source: Ref::default(),
        }],
        ..Drawing2D::default()
    }
}

fn options(origin: Origin) -> ReviewOptions {
    ReviewOptions {
        origin,
        tolerance: 0.01,
        region: None,
        chunking: Chunking::PROVISIONAL,
        style: ReviewStyle {
            color: [200, 20, 30],
            line_weight: 0.25,
            text_height: 2.0,
            text_padding: 0.5,
            arrow_length: 1.0,
            arrows: ReviewArrows::None,
            cloud_pitch: 5.0,
            marker_size: 4.0,
            font: None,
        },
    }
}

fn markup(id: &str, kind: MarkupKind, view: Option<&str>, points: &[[f64; 2]]) -> Markup {
    Markup {
        id: id.into(),
        kind,
        target: "plan".into(),
        view: view.map(str::to_owned),
        points: points.to_vec(),
        text: None,
        topic: None,
        carried: None,
        extra: Vec::new(),
    }
}

fn plane(id: &str, view: Option<&str>, pin: Option<[f64; 2]>) -> Location {
    Location {
        id: id.into(),
        target: "plan".into(),
        place: Place::Plane {
            view: view.map(str::to_owned),
            pin,
            extent: None,
            elements: Vec::new(),
        },
        carried: None,
        extra: Vec::new(),
    }
}

fn topic(locations: Vec<Location>) -> Topic {
    Topic {
        id: TOPIC.into(),
        title: "指摘の題を自動で図形にしない".into(),
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
        locations,
        origin: None,
        extra: Vec::new(),
    }
}

fn review_document(markups: Vec<Markup>) -> Review {
    Review {
        targets: vec![target()],
        markups,
        ..Review::default()
    }
}

fn hex(text: &str) -> String {
    text.bytes().map(|byte| format!("{byte:02x}")).collect()
}

fn markup_id(id: &str) -> String {
    format!("review/markup/{}", hex(id))
}

fn location_id(topic: &str, id: &str) -> String {
    format!("review/location/{}/{}", hex(topic), hex(id))
}

type Outcome = (
    Result<ReviewScene, (ReviewError, Ref)>,
    Budget,
    Vec<Diagnostic>,
);

fn run(
    drawing: &Drawing2D,
    review: &Review,
    target: &Target,
    options: &ReviewOptions,
    limits: Limits,
    cancelled: bool,
) -> Outcome {
    let mut budget = Budget::new(limits);
    let mut diagnostics = Diagnostics::new();
    let cancel = CancelToken::new();
    if cancelled {
        cancel.cancel();
    }
    let result = build_review(
        drawing,
        review,
        target,
        options,
        &mut Cx::new(&mut budget, &mut diagnostics, &cancel),
    );
    (result, budget, diagnostics.into_vec())
}

#[cfg(test)]
fn made(drawing: &Drawing2D, review: &Review, options: &ReviewOptions) -> ReviewScene {
    run(
        drawing,
        review,
        &target(),
        options,
        Limits::default(),
        false,
    )
    .0
    .unwrap()
}

fn code_count(diagnostics: &[Diagnostic], code: &str) -> u64 {
    diagnostics
        .iter()
        .filter(|d| d.code().name() == code)
        .map(Diagnostic::count)
        .sum()
}

fn has_place(diagnostics: &[Diagnostic], code: &str, id: &str) -> bool {
    diagnostics
        .iter()
        .any(|d| d.code().name() == code && d.places().iter().any(|place| place.id() == Some(id)))
}

#[cfg(test)]
struct Read<'a>(&'a [u8]);

#[cfg(test)]
impl Read<'_> {
    fn u32_at(&self, at: usize) -> u32 {
        u32::from_le_bytes(self.0[at..at + 4].try_into().unwrap())
    }

    fn section(&self, index: usize) -> &[u8] {
        let start = self.u32_at(16 + 8 * index) as usize;
        let length = self.u32_at(20 + 8 * index) as usize;
        &self.0[start..start + length]
    }

    fn u32s(&self, section: usize) -> Vec<u32> {
        self.section(section)
            .as_chunks::<4>()
            .0
            .iter()
            .map(|bytes| u32::from_le_bytes(*bytes))
            .collect()
    }

    fn f32s(&self, section: usize) -> Vec<f32> {
        self.section(section)
            .as_chunks::<4>()
            .0
            .iter()
            .map(|bytes| f32::from_le_bytes(*bytes))
            .collect()
    }

    fn f64s(&self, section: usize) -> Vec<f64> {
        self.section(section)
            .as_chunks::<8>()
            .0
            .iter()
            .map(|bytes| f64::from_le_bytes(*bytes))
            .collect()
    }

    fn strings(&self, section: usize) -> Vec<String> {
        self.u32s(section)
            .as_chunks::<2>()
            .0
            .iter()
            .map(|&[start, length]| {
                String::from_utf8(
                    self.section(layout::STRINGS)[start as usize..(start + length) as usize]
                        .to_vec(),
                )
                .unwrap()
            })
            .collect()
    }

    fn line_points(&self) -> Vec<[f64; 2]> {
        let origins = self.f64s(layout::CHUNK_ORIGINS);
        let chunks = self.u32s(layout::CHUNK_LINES);
        let starts = self.u32s(layout::LINE_VERTICES);
        let xy = self.f32s(layout::LINE_XY);
        let mut points = Vec::new();
        for (chunk, &start) in chunks.iter().enumerate() {
            let end = chunks
                .get(chunk + 1)
                .copied()
                .unwrap_or(starts.len() as u32);
            for line in start as usize..end as usize {
                let first = starts[line] as usize;
                let last = starts
                    .get(line + 1)
                    .copied()
                    .map_or(xy.len() / 2, |n| n as usize);
                for point in first..last {
                    points.push([
                        origins[2 * chunk] + f64::from(xy[2 * point]),
                        origins[2 * chunk + 1] + f64::from(xy[2 * point + 1]),
                    ]);
                }
            }
        }
        points
    }
}

#[test]
fn redlines_keep_their_coordinates_color_and_source() {
    let drawing = drawing();
    let review = review_document(vec![markup(
        "線/赤入れ",
        MarkupKind::Redline,
        Some("space/0"),
        &[[1.0, 2.0], [5.0, 4.0]],
    )]);
    let result = made(&drawing, &review, &options(Origin::Space(MODEL)));
    assert_eq!(result.from, "space/0");
    assert_eq!(result.view, None);
    assert_eq!(result.unit, Unit::Meter);
    assert_eq!(result.scene.counts.lines, 1);
    assert_eq!(result.scene.counts.vertices, 2);
    let read = Read(&result.scene.bytes);
    assert_eq!(read.line_points(), [[1.0, 2.0], [5.0, 4.0]]);
    assert_eq!(read.strings(layout::ENTITY_IDS), [markup_id("線/赤入れ")]);
    assert_eq!(read.section(layout::COLOR_RGBA), [200, 20, 30, 255]);
    assert_eq!(read.f32s(layout::UNIT_WEIGHTS), [0.25]);
    assert_eq!(result.sources.len(), 1);
    assert_eq!(result.sources[0].scene_id, markup_id("線/赤入れ"));
    assert!(
        matches!(&result.sources[0].source, ReviewSourceKind::Markup { id } if id == "線/赤入れ")
    );
}

#[test]
fn explicit_arrow_ends_add_faces_without_changing_the_source_id() {
    let drawing = drawing();
    let review = review_document(vec![markup(
        "line",
        MarkupKind::Redline,
        Some("space/0"),
        &[[0.0, 0.0], [0.0, 0.0], [10.0, 0.0], [10.0, 0.0]],
    )]);
    for (arrows, faces) in [
        (ReviewArrows::None, 0),
        (ReviewArrows::Start, 1),
        (ReviewArrows::End, 1),
        (ReviewArrows::Both, 2),
    ] {
        let mut options = options(Origin::Space(MODEL));
        options.style.arrows = arrows;
        let result = made(&drawing, &review, &options);
        assert_eq!(result.scene.counts.faces, faces);
        assert_eq!(result.sources.len(), 1);
        assert!(
            Read(&result.scene.bytes)
                .strings(layout::ENTITY_IDS)
                .iter()
                .all(|id| id == &markup_id("line"))
        );
    }
}

#[test]
fn clouds_bulge_outside_both_windings() {
    let drawing = drawing();
    for points in [
        vec![[0.0, 0.0], [10.0, 0.0], [10.0, 10.0], [0.0, 10.0]],
        vec![[0.0, 0.0], [0.0, 10.0], [10.0, 10.0], [10.0, 0.0]],
    ] {
        let review = review_document(vec![markup(
            "cloud",
            MarkupKind::Cloud,
            Some("space/0"),
            &points,
        )]);
        let result = made(&drawing, &review, &options(Origin::Space(MODEL)));
        let extent = result.scene.extent.unwrap();
        for (got, wanted) in extent.into_iter().zip([-2.5, -2.5, 12.5, 12.5]) {
            assert!((got - wanted).abs() <= 0.02);
        }
        assert!(result.scene.counts.lines > 0);
        assert!(result.scene.counts.vertices > 8);
        assert_eq!(result.sources[0].scene_id, markup_id("cloud"));
    }
}

#[test]
fn text_lines_and_photo_markers_do_not_fetch_or_invent_content() {
    let drawing = drawing();
    let mut text = markup("text", MarkupKind::Text, Some("space/0"), &[[2.0, 8.0]]);
    text.text = Some("一行\r\n二行".into());
    let mut photo = markup("photo", MarkupKind::Photo, Some("space/0"), &[[30.0, 30.0]]);
    photo.text = Some("https://invalid.example/photo.jpg".into());
    let review = review_document(vec![text, photo]);
    let options = options(Origin::Space(MODEL));
    let (result, _, diagnostics) = run(
        &drawing,
        &review,
        &target(),
        &options,
        Limits::default(),
        false,
    );
    let result = result.unwrap();
    assert_eq!(result.scene.counts.texts, 2);
    assert_eq!(result.scene.counts.images, 0);
    let read = Read(&result.scene.bytes);
    assert_eq!(read.strings(layout::TEXT_STRINGS), ["一行", "二行"]);
    for (got, expected) in read.line_points()[..5].iter().zip([
        [1.5, 3.1],
        [6.5, 3.1],
        [6.5, 8.5],
        [1.5, 8.5],
        [1.5, 3.1],
    ]) {
        for (got, expected) in got.iter().zip(expected) {
            assert!((got - expected).abs() < 0.00001);
        }
    }
    assert!(has_place(
        &diagnostics,
        "review.scene-text-frame.approximated",
        &markup_id("text")
    ));
    let mut lf = review.clone();
    lf.markups[0].text = Some("一行\n二行".into());
    assert_eq!(
        made(&drawing, &lf, &options).scene.bytes,
        result.scene.bytes
    );
    let mut budget = Budget::default();
    let mut diagnostics = Diagnostics::new();
    let with_empty_fonts = build_review_with(
        &drawing,
        &review,
        &target(),
        &options,
        &FontSet::default(),
        &mut Cx::new(&mut budget, &mut diagnostics, &CancelToken::new()),
    )
    .unwrap();
    assert_eq!(with_empty_fonts.scene.bytes, result.scene.bytes);
}

#[test]
fn location_pin_and_extent_share_one_source_but_not_a_markup_id() {
    let drawing = drawing();
    let mut review = review_document(vec![markup(
        "same/日本語",
        MarkupKind::Redline,
        Some("space/0"),
        &[[0.0, 0.0], [1.0, 1.0]],
    )]);
    let mut location = plane("same/日本語", Some("space/0"), Some([20.0, 30.0]));
    if let Place::Plane { extent, .. } = &mut location.place {
        *extent = Some([15.0, 25.0, 25.0, 35.0]);
    }
    review.topics.push(topic(vec![location]));
    let result = made(&drawing, &review, &options(Origin::Space(MODEL)));
    let ids: BTreeSet<_> = Read(&result.scene.bytes)
        .strings(layout::ENTITY_IDS)
        .into_iter()
        .collect();
    assert_eq!(
        ids,
        BTreeSet::from([markup_id("same/日本語"), location_id(TOPIC, "same/日本語")])
    );
    assert_eq!(result.sources.len(), 2);
    assert!(matches!(&result.sources[1].source,
        ReviewSourceKind::Location { topic, id } if topic == TOPIC && id == "same/日本語"));
    assert_eq!(result.scene.counts.texts, 0);
    assert_eq!(result.scene.extent.unwrap(), [0.0, 0.0, 25.0, 35.0]);
}

#[test]
fn source_ids_survive_reordering_and_geometry_edits() {
    let drawing = drawing();
    let mut review = review_document(
        ["a/b", "a", "b", "日本語\0"]
            .iter()
            .map(|id| {
                markup(
                    id,
                    MarkupKind::Redline,
                    Some("space/0"),
                    &[[0.0, 0.0], [1.0, 2.0]],
                )
            })
            .collect(),
    );
    let original = made(&drawing, &review, &options(Origin::Space(MODEL)));
    let ids = |scene: &ReviewScene| {
        scene
            .sources
            .iter()
            .map(|source| source.scene_id.clone())
            .collect::<BTreeSet<_>>()
    };
    assert_eq!(ids(&original).len(), 4);
    review.markups.reverse();
    review.markups[0].points[1] = [9.0, 8.0];
    assert_eq!(
        ids(&made(&drawing, &review, &options(Origin::Space(MODEL)))),
        ids(&original)
    );
}

#[test]
fn target_format_hash_and_presence_are_checked_before_drawing() {
    let drawing = drawing();
    let review = review_document(vec![markup(
        "line",
        MarkupKind::Redline,
        Some("space/0"),
        &[[0.0, 0.0], [1.0, 1.0]],
    )]);
    for (field, code) in [
        ("id", "review.target.missing"),
        ("format", "review.target.stale"),
        ("hash", "review.target.stale"),
    ] {
        let mut supplied = target();
        match field {
            "id" => supplied.id = "other".into(),
            "format" => supplied.format = "jww".into(),
            _ => supplied.hash = zumen_drawing::Hash::document(b"other").to_string(),
        }
        let (result, _, diagnostics) = run(
            &drawing,
            &review,
            &supplied,
            &options(Origin::Space(MODEL)),
            Limits::default(),
            false,
        );
        assert!(matches!(result, Err((ReviewError::Invalid, _))));
        assert_eq!(code_count(&diagnostics, code), 1);
    }
}

#[test]
fn sheet_view_space_and_duplicate_sheet_names_are_not_mixed() {
    let drawing = drawing();
    let origins = [
        (Origin::Space(MODEL), "space/0", None, Unit::Meter),
        (
            Origin::Sheet(SheetId(0)),
            "sheet/h22",
            None,
            Unit::Millimeter,
        ),
        (
            Origin::View {
                sheet: SheetId(0),
                view: 0,
            },
            "sheet/h22",
            Some(0),
            Unit::Millimeter,
        ),
        (
            Origin::Sheet(SheetId(1)),
            "sheet/h22/1",
            None,
            Unit::Millimeter,
        ),
        (
            Origin::View {
                sheet: SheetId(1),
                view: 0,
            },
            "sheet/h22/1",
            Some(0),
            Unit::Millimeter,
        ),
    ];
    let views = [
        "space/0",
        "sheet/h22",
        "sheet/h22/view/0",
        "sheet/h22/1",
        "sheet/h22/1/view/0",
    ];
    let mut review = review_document(
        views
            .iter()
            .enumerate()
            .map(|(index, view)| {
                markup(
                    &format!("line-{index}"),
                    MarkupKind::Redline,
                    Some(view),
                    &[[1.0, 2.0], [5.0, 4.0]],
                )
            })
            .collect(),
    );
    let mut other = target();
    other.id = "another-document".into();
    review.targets.push(other);
    let mut foreign = markup(
        "foreign",
        MarkupKind::Redline,
        Some("space/0"),
        &[[300.0, 400.0], [500.0, 600.0]],
    );
    foreign.target = "another-document".into();
    review.markups.push(foreign);
    for (index, (origin, from, view, unit)) in origins.into_iter().enumerate() {
        let result = made(&drawing, &review, &options(origin));
        assert_eq!(result.from, from);
        assert_eq!(result.view, view);
        assert_eq!(result.unit, unit);
        assert_eq!(result.sources.len(), 1);
        assert_eq!(
            result.sources[0].scene_id,
            markup_id(&format!("line-{index}"))
        );
        // 用紙のビューでも、点へ変換を二重に当てない。
        assert_eq!(
            Read(&result.scene.bytes).line_points(),
            [[1.0, 2.0], [5.0, 4.0]]
        );
    }
    let unknown = review_document(vec![markup(
        "unknown-unit",
        MarkupKind::Redline,
        Some("space/1"),
        &[[1.0, 2.0], [5.0, 4.0]],
    )]);
    assert_eq!(
        made(&drawing, &unknown, &options(Origin::Space(SpaceId(1)))).unit,
        Unit::Unknown
    );
}

#[test]
fn missing_origins_and_invalid_options_do_not_return_a_scene() {
    let drawing = drawing();
    let review = review_document(Vec::new());
    for origin in [
        Origin::Space(SpaceId(99)),
        Origin::Sheet(SheetId(99)),
        Origin::View {
            sheet: SheetId(0),
            view: 99,
        },
    ] {
        let (result, _, diagnostics) = run(
            &drawing,
            &review,
            &target(),
            &options(origin),
            Limits::default(),
            false,
        );
        assert!(matches!(result, Err((ReviewError::Invalid, _))));
        assert_eq!(code_count(&diagnostics, "review.scene-origin.invalid"), 1);
    }
    for invalid in 0..6 {
        let mut options = options(Origin::Space(MODEL));
        match invalid {
            0 => options.tolerance = f64::NAN,
            1 => options.style.text_height = 0.0,
            2 => options.style.line_weight = -1.0,
            3 => options.style.cloud_pitch = f64::INFINITY,
            4 => options.region = Some([2.0, 0.0, 1.0, 1.0]),
            _ => options.chunking.items = 0,
        }
        let (result, _, diagnostics) = run(
            &drawing,
            &review,
            &target(),
            &options,
            Limits::default(),
            false,
        );
        assert!(matches!(result, Err((ReviewError::Invalid, _))));
        assert_eq!(code_count(&diagnostics, "review.scene-options.invalid"), 1);
    }
}

#[test]
fn malformed_selected_geometry_is_rejected_but_filtered_geometry_is_not_examined() {
    let drawing = drawing();
    let mut review = review_document(vec![markup(
        "line",
        MarkupKind::Redline,
        Some("space/0"),
        &[[0.0, 0.0], [1.0, 1.0]],
    )]);
    for invalid in 0..4 {
        let mut broken = review.clone();
        match invalid {
            0 => broken.markups[0].points[0][0] = f64::NAN,
            1 => broken.markups[0].points.truncate(1),
            2 => {
                broken.markups[0].kind = MarkupKind::Cloud;
                broken.markups[0].points = vec![[0.0, 0.0], [1.0, 1.0], [2.0, 2.0]];
            }
            _ => {
                broken.markups[0].kind = MarkupKind::Text;
                broken.markups[0].text = Some(String::new());
            }
        }
        let (result, _, diagnostics) = run(
            &drawing,
            &broken,
            &target(),
            &options(Origin::Space(MODEL)),
            Limits::default(),
            false,
        );
        assert!(matches!(result, Err((ReviewError::Invalid, _))));
        assert!(has_place(
            &diagnostics,
            "review.scene-geometry.invalid",
            &markup_id("line")
        ));
    }
    review.markups.push(markup(
        "filtered",
        MarkupKind::Cloud,
        Some("unknown-view"),
        &[[f64::NAN, f64::INFINITY]],
    ));
    let result = made(&drawing, &review, &options(Origin::Space(MODEL)));
    assert_eq!(result.sources.len(), 1);
}

#[test]
fn document_id_and_reference_integrity_is_checked_outside_the_selected_view() {
    let drawing = drawing();
    let base = review_document(vec![markup(
        "line",
        MarkupKind::Redline,
        Some("space/0"),
        &[[0.0, 0.0], [1.0, 1.0]],
    )]);
    for invalid in 0..5 {
        let mut review = base.clone();
        match invalid {
            0 => review.markups.push(review.markups[0].clone()),
            1 => {
                let mut extra = markup("filtered", MarkupKind::Redline, Some("unknown"), &[]);
                extra.target = "missing-target".into();
                review.markups.push(extra);
            }
            2 => review.markups[0].topic = Some("missing-topic".into()),
            3 => {
                let mut item = topic(Vec::new());
                item.comments.push(Comment {
                    id: "comment".into(),
                    author: None,
                    date: None,
                    text: "本文".into(),
                    location: Some("missing-location".into()),
                    extra: Vec::new(),
                });
                review.topics.push(item);
            }
            _ => review.targets.push(review.targets[0].clone()),
        }
        let (result, _, diagnostics) = run(
            &drawing,
            &review,
            &target(),
            &options(Origin::Space(MODEL)),
            Limits::default(),
            false,
        );
        assert!(matches!(result, Err((ReviewError::Invalid, _))));
        let wanted = if invalid == 0 || invalid == 4 {
            "review.id.exists"
        } else {
            "review.reference.missing"
        };
        assert!(code_count(&diagnostics, wanted) > 0);
    }
}

#[test]
fn skipped_placements_extra_points_and_legacy_rendering_values_are_diagnosed() {
    let drawing = drawing();
    let source = r#"{"format":"zumen-review","version":1,"targets":[{"id":"plan","format":"dxf","hash":"fixture"}],"legacy_item_order":["nowhere","text","photo"],"markups":[{"id":"text","kind":"text","target":"plan","view":"space/0","points":[[0,0],[99,99]],"text":"注記","legacy.fontSize":8},{"id":"photo","kind":"photo","target":"plan","view":"space/0","points":[[20,20],[99,99]],"text":"photo-ref"},{"id":"nowhere","kind":"redline","target":"plan","points":[[0,0],[1,1]]}]}"#;
    let mut budget = Budget::default();
    let mut diagnostics = Diagnostics::new();
    let mut review = Review::read(
        source.as_bytes(),
        &mut Cx::new(&mut budget, &mut diagnostics, &CancelToken::new()),
    )
    .unwrap();
    review.targets[0] = target();
    // 先頭以外の点は幾何の検査に使わず、使わなかった点として診断する。
    review.markups[0].points[1] = [f64::NAN, f64::INFINITY];
    review.markups[1].points[1] = [f64::NEG_INFINITY, f64::NAN];
    let mut three_d = plane("three-d", None, None);
    three_d.place = Place::Space {
        camera: None,
        selected: Vec::new(),
        visible: None,
        exceptions: Vec::new(),
        colored: Vec::new(),
        clips: Vec::new(),
    };
    review.topics.push(topic(vec![
        three_d,
        plane("no-geometry", Some("space/0"), None),
    ]));
    let (result, _, diagnostics) = run(
        &drawing,
        &review,
        &target(),
        &options(Origin::Space(MODEL)),
        Limits::default(),
        false,
    );
    let result = result.unwrap();
    assert_eq!(result.sources.len(), 2);
    assert_eq!(
        code_count(&diagnostics, "review.scene-placement.dropped"),
        3
    );
    let points: u64 = diagnostics
        .iter()
        .filter(|d| d.code().name() == "review.scene-points.dropped")
        .map(|d| match d.param("points") {
            Some(Value::UInt(n)) => n.saturating_mul(d.count()),
            _ => 0,
        })
        .sum();
    assert_eq!(points, 2);
    assert!(has_place(
        &diagnostics,
        "review.scene-points.dropped",
        &markup_id("text")
    ));
    assert!(has_place(
        &diagnostics,
        "review.scene-points.dropped",
        &markup_id("photo")
    ));
    assert!(code_count(&diagnostics, "review.scene-property.dropped") >= 2);
}

#[test]
fn inputs_and_normal_drawing_scenes_are_unchanged() {
    let drawing = drawing();
    let mut review = review_document(vec![markup(
        "note",
        MarkupKind::Text,
        Some("space/0"),
        &[[0.0, 0.0]],
    )]);
    review.markups[0].text = Some("描画前の文書を保つ".into());
    let original_drawing = drawing.clone();
    let original_review = review.clone();
    let original_json = review.to_bytes();
    let drawing_scene = || {
        let mut budget = Budget::default();
        let mut diagnostics = Diagnostics::new();
        build(
            &drawing,
            &Options {
                origin: Origin::Space(MODEL),
                tolerance: Some(0.01),
                print: false,
                region: None,
                palette: Palette::default(),
                chunking: Chunking::PROVISIONAL,
            },
            &mut Cx::new(&mut budget, &mut diagnostics, &CancelToken::new()),
        )
        .unwrap()
    };
    let before = drawing_scene();
    let a = run(
        &drawing,
        &review,
        &target(),
        &options(Origin::Space(MODEL)),
        Limits::default(),
        false,
    );
    let b = run(
        &drawing,
        &review,
        &target(),
        &options(Origin::Space(MODEL)),
        Limits::default(),
        false,
    );
    assert_eq!(a.0.unwrap().scene.bytes, b.0.unwrap().scene.bytes);
    assert_eq!(a.1, b.1);
    assert_eq!(a.2, b.2);
    assert_eq!(review, original_review);
    assert_eq!(review.to_bytes(), original_json);
    assert_eq!(drawing, original_drawing);
    assert_eq!(drawing_scene().bytes, before.bytes);
}

#[test]
fn budgets_stop_without_partial_results_and_exact_limits_reproduce_the_scene() {
    let drawing = drawing();
    let review = review_document(vec![markup(
        "cloud",
        MarkupKind::Cloud,
        Some("space/0"),
        &[[0.0, 0.0], [2.0, 0.0], [2.0, 2.0], [0.0, 2.0]],
    )]);
    let mut options = options(Origin::Space(MODEL));
    options.style.cloud_pitch = 0.125;
    let (result, budget, diagnostics) = run(
        &drawing,
        &review,
        &target(),
        &options,
        Limits::default(),
        false,
    );
    let result = result.unwrap();
    for kind in [
        Kind::Work,
        Kind::DecodedBytes,
        Kind::Elements,
        Kind::Segments,
    ] {
        let used = budget.used(kind);
        assert!(used > 0);
        let exact = Limits::default().with(kind, Limit::Finite(used));
        let (same, _, found) = run(&drawing, &review, &target(), &options, exact, false);
        assert_eq!(same.unwrap().scene.bytes, result.scene.bytes);
        assert_eq!(found, diagnostics);
        let too_small = Limits::default().with(kind, Limit::Finite(used - 1));
        let (stopped, _, found) = run(&drawing, &review, &target(), &options, too_small, false);
        assert!(
            matches!(stopped, Err((ReviewError::Stopped(Stop::Exceeded(e)), _)) if e.kind == kind)
        );
        assert_eq!(code_count(&found, kind.exceeded_code().name()), 1);
    }
    let (cancelled, _, diagnostics) = run(
        &drawing,
        &review,
        &target(),
        &options,
        Limits::default(),
        true,
    );
    assert!(matches!(
        cancelled,
        Err((ReviewError::Stopped(Stop::Cancelled), _))
    ));
    assert_eq!(code_count(&diagnostics, "call.request.cancelled"), 1);
}

#[test]
fn region_filtering_does_not_move_review_geometry() {
    let drawing = drawing();
    let review = review_document(vec![markup(
        "line",
        MarkupKind::Redline,
        Some("space/0"),
        &[[1.0, 2.0], [5.0, 4.0]],
    )]);
    let mut options = options(Origin::Space(MODEL));
    options.region = Some([0.0, 0.0, 6.0, 5.0]);
    assert_eq!(
        Read(&made(&drawing, &review, &options).scene.bytes).line_points(),
        [[1.0, 2.0], [5.0, 4.0]]
    );
    options.region = Some([100.0, 100.0, 110.0, 110.0]);
    let result = made(&drawing, &review, &options);
    assert_eq!(result.scene.counts.lines, 0);
    assert_eq!(result.scene.counts.vertices, 0);
    assert_eq!(result.scene.extent, None);
}

#[test]
fn text_overlapping_a_region_survives_when_its_anchor_is_outside() {
    let drawing = drawing();
    let bytes = font_bytes();
    for (use_font, caption) in [(false, false), (true, false), (false, true), (true, true)] {
        let mut review = review_document(vec![markup(
            "region-note",
            if caption {
                MarkupKind::Redline
            } else {
                MarkupKind::Text
            },
            Some("space/0"),
            if caption {
                &[[0.0, 7.5], [1.0, 7.5]]
            } else {
                &[[0.0, 10.0]]
            },
        )]);
        review.markups[0].text = Some(if use_font { "HH" } else { "ABCDEFGH" }.into());
        let mut fonts = FontSet::default();
        let mut options = options(Origin::Space(MODEL));
        options.chunking.items = 1;
        if use_font {
            fonts.add(Some("synthetic"), Face::parse(&bytes, 0).unwrap());
            options.style.font = Some("synthetic".into());
        }
        let make = |options: &ReviewOptions| {
            let mut budget = Budget::default();
            let mut diagnostics = Diagnostics::new();
            build_review_with(
                &drawing,
                &review,
                &target(),
                options,
                &fonts,
                &mut Cx::new(&mut budget, &mut diagnostics, &CancelToken::new()),
            )
            .unwrap()
        };
        assert_eq!(make(&options).scene.counts.texts, 1);
        options.region = Some(if use_font {
            [1.0, 9.0, 1.1, 9.1]
        } else {
            [10.0, 8.0, 11.0, 9.0]
        });
        let clipped = make(&options);
        assert_eq!(clipped.scene.counts.texts, 1);
        assert_eq!(clipped.scene.counts.lines, if caption { 0 } else { 1 });
        options.region = Some([100.0, 100.0, 101.0, 101.0]);
        assert_eq!(make(&options).scene.counts.texts, 0);
    }
}

#[test]
fn glyph_captions_outside_the_f32_relative_range_are_rejected() {
    let drawing = drawing();
    let mut review = review_document(vec![markup(
        "large-caption",
        MarkupKind::Redline,
        Some("space/0"),
        &[[0.0, 0.0], [1.0, 0.0]],
    )]);
    review.markups[0].text = Some("HHHHHHHHHH".into());
    let bytes = font_bytes();
    let mut fonts = FontSet::default();
    fonts.add(Some("synthetic"), Face::parse(&bytes, 0).unwrap());
    let mut options = options(Origin::Space(MODEL));
    options.style.font = Some("synthetic".into());
    options.style.text_height = 1e38;
    let mut budget = Budget::default();
    let mut diagnostics = Diagnostics::new();
    let result = build_review_with(
        &drawing,
        &review,
        &target(),
        &options,
        &fonts,
        &mut Cx::new(&mut budget, &mut diagnostics, &CancelToken::new()),
    );
    assert!(matches!(result, Err((ReviewError::Invalid, _))));
    assert!(has_place(
        &diagnostics.into_vec(),
        "review.scene-geometry.invalid",
        &markup_id("large-caption")
    ));
}

/// 既存の scene.rs と同じ合成 TrueType。.notdef・H・g・空白・I だけを持つ。
const FONT: &[&str] = &[
    "0001000000080000000000004f532f32000000000000008c00000060636d617000000000000000ec0000004c676c7966",
    "00000000000001380000007c6865616400000000000001b4000000366868656100000000000001ec00000024686d7478",
    "0000000000000210000000106c6f63610000000000000220000000186d61787000000000000002380000000600040000",
    "000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000",
    "000000000000000000000000000000000320ff380000000000000000000000000000000000002bc00000000000000001",
    "0003000a0000000c000c0000000000400000000000000004000000200000002000000003000000480000004800000001",
    "00000049000000490000000400000067000000670000000200010032000001c202bc0003000033211121320190fe7002",
    "bc00000000020000000002bc02d0000300070000313311230133112364640258646402d0fd3002d00001001eff2e0208",
    "01e00003000004120a010113f5f5f5d201590159fea70000ffff0000000002bc02d0002b0001000a0014200000020002",
    "fb0300000001000000000000000000005f0f3cf5000003e8000000000000000000000000000000000000000000000000",
    "000000000000000100000000000100000384ff0600000000000000000000000000000000000000000000000000000004",
    "01f4000002bc00000226000000fa0000000000000000001c0000004000000060000000600000007c0000500000050000",
];

#[cfg(test)]
fn font_bytes() -> Vec<u8> {
    FONT.concat()
        .as_bytes()
        .chunks(2)
        .map(|pair| u8::from_str_radix(core::str::from_utf8(pair).unwrap(), 16).unwrap())
        .collect()
}

#[test]
fn supplied_glyphs_define_multiline_frames_and_obey_preallocation_budgets() {
    let drawing = drawing();
    let mut review = review_document(vec![markup(
        "glyph-note",
        MarkupKind::Text,
        Some("space/0"),
        &[[5.0, 10.0]],
    )]);
    review.markups[0].text = Some("Hg\r\nHI".into());
    let font = font_bytes();
    let mut fonts = FontSet::default();
    assert!(fonts.add(Some("synthetic"), Face::parse(&font, 0).unwrap()));
    let mut options = options(Origin::Space(MODEL));
    options.style.font = Some("synthetic".into());
    let make = |limits| {
        let mut budget = Budget::new(limits);
        let mut diagnostics = Diagnostics::new();
        let cancel = CancelToken::new();
        let result = build_review_with(
            &drawing,
            &review,
            &target(),
            &options,
            &fonts,
            &mut Cx::new(&mut budget, &mut diagnostics, &cancel),
        );
        (result, budget, diagnostics.into_vec())
    };
    let (result, budget, diagnostics) = make(Limits::default());
    let result = result.unwrap();
    let (_, without_fonts, _) = run(
        &drawing,
        &review,
        &target(),
        &options,
        Limits::default(),
        false,
    );
    assert_eq!(result.scene.counts.texts, 2);
    assert_eq!(result.scene.counts.glyphs, 4);
    assert!(result.scene.counts.lines >= 1);
    assert_eq!(result.sources.len(), 1);
    for code in [
        "review.scene-text-frame.approximated",
        "scene.font.missing",
        "scene.text-layout.unsupported",
        "scene.glyph.missing",
    ] {
        assert_eq!(code_count(&diagnostics, code), 0);
    }
    for kind in [Kind::Segments, Kind::DecodedBytes] {
        let used = budget.used(kind);
        assert!(used > 0);
        let exact = Limits::default().with(kind, Limit::Finite(used));
        assert_eq!(make(exact).0.unwrap().scene.bytes, result.scene.bytes);
        // 同じ注記のフォントなしの量では作れない。最初の行の配置は許し、
        // 輪郭を作る前の予約で止まることを確かめる。
        let limit = without_fonts.used(kind);
        let (stopped, used, diagnostics) = make(Limits::default().with(kind, Limit::Finite(limit)));
        assert!(
            matches!(stopped, Err((ReviewError::Stopped(Stop::Exceeded(e)), ref at))
                if e.kind == kind && at.id() == Some(markup_id("glyph-note").as_str()))
        );
        assert!(used.used(kind) <= limit);
        assert_eq!(code_count(&diagnostics, kind.exceeded_code().name()), 1);
    }
}

#[test]
fn font_names_are_charged_before_interning_without_font_files() {
    let drawing = drawing();
    let mut review = review_document(vec![markup(
        "named-font-note",
        MarkupKind::Text,
        Some("space/0"),
        &[[0.0, 0.0]],
    )]);
    review.markups[0].text = Some("H".into());
    let make = |font: Option<&str>, limits| {
        let mut options = options(Origin::Space(MODEL));
        options.style.font = font.map(str::to_owned);
        run(&drawing, &review, &target(), &options, limits, false)
    };
    let (without_name, no_name_budget, _) = make(None, Limits::default());
    assert!(without_name.is_ok());
    let (short, short_budget, _) = make(Some("x"), Limits::default());
    assert!(short.is_ok());
    let long_name = "x".repeat(8192);
    let (long, long_budget, _) = make(Some(&long_name), Limits::default());
    let long = long.unwrap();
    assert_eq!(
        Read(&long.scene.bytes).strings(layout::FONT_NAMES),
        [long_name.as_str()]
    );
    assert!(
        long_budget.used(Kind::DecodedBytes) - short_budget.used(Kind::DecodedBytes)
            >= long_name.len() as u64 - 1
    );
    let limit = no_name_budget.used(Kind::DecodedBytes);
    let low = Limits::default().with(Kind::DecodedBytes, Limit::Finite(limit));
    assert!(make(None, low).0.is_ok());
    let (stopped, spent, diagnostics) = make(Some(&long_name), low);
    assert!(
        matches!(stopped, Err((ReviewError::Stopped(Stop::Exceeded(e)), ref at))
            if e.kind == Kind::DecodedBytes
                && at.id() == Some(markup_id("named-font-note").as_str()))
    );
    assert!(spent.used(Kind::DecodedBytes) <= limit);
    assert_eq!(code_count(&diagnostics, "budget.decoded-bytes.exceeded"), 1);
}
