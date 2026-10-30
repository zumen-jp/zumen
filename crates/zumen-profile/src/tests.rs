//! 試験の中で作った小さな図面を、`mlit-cad` の 2025-12 版と、それを `extends` で指すプロファイルで照合する。

use zumen_base::{Budget, CancelToken, Diagnostics, Limit, Limits};
use zumen_drawing::{
    ColorCode, Horizontal, Inherit, Layer, LineType, LineTypeId, LineWeight, ModelLength,
    ModelPoint, ModelSpace, PaperLength, PaperPoint, Placement, Sheet, SheetId, SheetItem, SpaceId,
    Style, Text, Unit, Vertical, View,
};

use super::*;

const MLIT: &[u8] = include_bytes!("../../../profiles/mlit-cad/2025-12/profile.json");
const MODEL: SpaceId = SpaceId(0);
const SHEET: SheetId = SheetId(0);

fn style(color: Color, line_type: u32, width: f64) -> Style {
    Style {
        color: Inherit::Own(color),
        line_type: Inherit::Own(LineTypeId(line_type)),
        line_type_scale: 1.0,
        line_weight: Some(Inherit::Own(LineWeight::Width(PaperLength(width)))),
        transparency: Inherit::Layer,
    }
}

fn sxf(value: u32) -> Color {
    Color {
        code: Some(ColorCode {
            system: ColorSystem::Sxf,
            value,
        }),
        rgb: None,
    }
}

fn point(x: f64) -> ModelPoint {
    ModelPoint {
        space: MODEL,
        x,
        y: 0.0,
    }
}

fn line(layer: u32, style: Style) -> zumen_drawing::Element<ModelPoint> {
    zumen_drawing::Element {
        source: Ref::default(),
        layer: LayerId(layer),
        style,
        shape: Shape::Line {
            start: point(0.0),
            end: point(1.0),
        },
    }
}

fn text(content: &str, height: f64) -> zumen_drawing::Element<ModelPoint> {
    zumen_drawing::Element {
        source: Ref::default(),
        layer: LayerId(0),
        style: style(sxf(1), 0, 0.35),
        shape: Shape::Text(Text {
            content: content.to_string(),
            height: Height {
                value: Length::Model(ModelLength {
                    space: MODEL,
                    value: height,
                }),
                meaning: HeightMeaning::Frame,
            },
            placement: Placement::Anchor {
                point: point(0.0),
                horizontal: Horizontal::Left,
                vertical: Vertical::Baseline,
            },
            rotation: 0.0,
            width_factor: 1.0,
            oblique: 0.0,
            backward: false,
            upside_down: false,
            style: String::new(),
        }),
    }
}

fn layer(id: u32, name: &str) -> Layer {
    Layer {
        id: LayerId(id),
        name: name.to_string(),
        state: None,
        style: None,
        follows_placement: false,
        description: None,
        source: Ref::default(),
    }
}

fn view(scale: f64) -> View {
    View {
        space: MODEL,
        sheet: SHEET,
        transform: Affine {
            m: [scale, 0.0, 0.0, 0.0, scale, 0.0],
        },
        clip: Vec::new(),
        hidden_layers: Vec::new(),
        source: Ref::default(),
    }
}

/// A3 の横の用紙に、1:100 と 1:7 のビューで実寸の空間を置いた図面
fn drawing() -> Drawing2D {
    let elements = vec![
        // 色・線種・線幅が語彙にある
        line(0, style(sxf(2), 0, 0.35)),
        // 色の符号が語彙にない
        line(0, style(sxf(20), 0, 0.35)),
        // RGB だけの色が語彙にない・線種が語彙にない・線幅が語彙にない
        line(
            1,
            style(
                Color {
                    code: None,
                    rgb: Some([0x12, 0x34, 0x56]),
                },
                1,
                0.3,
            ),
        ),
        // 用紙の 3.5 mm と 3 mm（1:100）、機種依存文字を含む
        text("壁", 350.0),
        text("①", 300.0),
    ];
    Drawing2D {
        spaces: vec![ModelSpace {
            id: MODEL,
            unit: Unit::Millimeter,
            order: (0..elements.len()).collect(),
            elements,
        }],
        sheets: vec![Sheet {
            id: SHEET,
            name: None,
            size: Some([PaperLength(420.0), PaperLength(297.0)]),
            elements: Vec::<zumen_drawing::Element<PaperPoint>>::new(),
            views: vec![view(0.01)],
            order: vec![SheetItem::View(0)],
            source: Ref::default(),
        }],
        layers: vec![layer(0, "C-STR-壁"), layer(1, "BAD"), layer(2, "UNUSED")],
        line_types: vec![
            LineType {
                id: LineTypeId(0),
                name: "continuous".to_string(),
                definition: None,
                source: Ref::default(),
            },
            LineType {
                id: LineTypeId(1),
                name: "foo".to_string(),
                definition: None,
                source: Ref::default(),
            },
        ],
        ..Drawing2D::default()
    }
}

fn run(
    drawing: &Drawing2D,
    profiles: &[(&str, &[u8])],
    id: &str,
    family: &str,
    origins: Vec<Origin>,
    limits: Limits,
) -> (Result<Report, Error>, Vec<Diagnostic>) {
    let mut budget = Budget::new(limits);
    let mut diag = Diagnostics::new();
    let cancel = CancelToken::new();
    let palette = Palette::default();
    let options = Options {
        origins,
        palette: &palette,
        family,
    };
    let result = check(
        drawing,
        profiles,
        id,
        "2025-12",
        &options,
        &mut Cx::new(&mut budget, &mut diag, &cancel),
    );
    (result, diag.into_vec())
}

fn codes_of(report: &Report) -> Vec<(&str, &str, Severity)> {
    report
        .findings
        .iter()
        .map(|f| (f.rule.as_str(), f.diagnostic.code().name(), f.level))
        .collect()
}

#[test]
fn checks_a_drawing_against_mlit_cad() {
    let drawing = drawing();
    let origins = origins(&drawing, None, None).unwrap();
    let (report, diagnostics) = run(
        &drawing,
        &[("mlit.json", MLIT)],
        "mlit-cad",
        "sxf",
        origins,
        Limits::default(),
    );
    let report = report.unwrap();
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    assert_eq!(
        report.chain,
        vec![Identity {
            id: "mlit-cad".to_string(),
            edition: "2025-12".to_string(),
            revision: 1,
        }]
    );
    assert_eq!(
        codes_of(&report),
        vec![
            ("layer-name", "profile.layer-name.invalid", Severity::Error),
            ("color", "profile.color.invalid", Severity::Warning),
            ("color", "profile.color.invalid", Severity::Warning),
            ("line-type", "profile.line-type.invalid", Severity::Warning),
            (
                "line-width",
                "profile.line-width.invalid",
                Severity::Warning
            ),
            (
                "text-height",
                "profile.text-height.invalid",
                Severity::Warning
            ),
            (
                "characters",
                "profile.characters.invalid",
                Severity::Warning
            ),
        ]
    );
    let params = |at: usize| report.findings[at].diagnostic.params().to_vec();
    assert!(params(0).contains(&("layer", Param::Text("BAD".to_string()))));
    assert!(params(1).contains(&("color", Param::Text("sxf:20".to_string()))));
    assert!(params(2).contains(&("color", Param::Text("#123456".to_string()))));
    assert!(params(5).contains(&("height", Param::Float(3.0))));
    assert!(params(6).contains(&("character", Param::Text("U+2460".to_string()))));
    // 要素・実効のレイヤ・起点の用紙の ID
    let places: Vec<Option<&str>> = report.findings[1]
        .diagnostic
        .places()
        .iter()
        .map(|place| place.id())
        .collect();
    assert_eq!(places.len(), 3);
    assert_eq!(places[2], Some("sheet/0"));
    let rules: Vec<(&str, u64, u64)> = report
        .rules
        .iter()
        .map(|r| (r.id.as_str(), r.findings, r.unsupported))
        .collect();
    assert_eq!(
        rules,
        vec![
            ("layer-name", 1, 0),
            ("color", 2, 0),
            ("line-type", 1, 0),
            ("line-width", 1, 0),
            ("text-height", 1, 0),
            ("characters", 1, 0),
            ("sheet-size", 0, 0),
            ("orientation", 0, 0),
            ("scale", 0, 0),
        ]
    );
}

#[test]
fn reports_values_it_cannot_check_as_info() {
    let mut drawing = drawing();
    drawing.sheets[0].size = None;
    drawing.sheets[0].views.push(view(1.0 / 7.0));
    drawing.sheets[0].order.push(SheetItem::View(1));
    drawing.spaces[0].elements.truncate(1);
    drawing.spaces[0].order = vec![0];
    // DXF の線種の名前は、語彙の sxf の列と比べない
    let (report, _) = run(
        &drawing,
        &[("mlit.json", MLIT)],
        "mlit-cad",
        "acad",
        origins(&drawing, Some("sheet/0"), None).unwrap(),
        Limits::default(),
    );
    let report = report.unwrap();
    let found = codes_of(&report);
    // 2 つのビューを通して、同じ線を 2 度たどる
    assert_eq!(
        found,
        vec![
            ("line-type", "profile.line-type.unsupported", Severity::Info),
            ("line-type", "profile.line-type.unsupported", Severity::Info),
            (
                "sheet-size",
                "profile.sheet-size.unsupported",
                Severity::Info
            ),
            (
                "orientation",
                "profile.orientation.unsupported",
                Severity::Info
            ),
            ("scale", "profile.scale.invalid", Severity::Info),
        ]
    );
    assert!(
        report.findings[4]
            .diagnostic
            .params()
            .contains(&("view", Param::UInt(1)))
    );
    // 実寸の空間からたどった実寸の文字は、用紙の高さが決まらない
    let drawing = self::drawing();
    let (report, _) = run(
        &drawing,
        &[("mlit.json", MLIT)],
        "mlit-cad",
        "sxf",
        origins(&drawing, Some("space/0"), None).unwrap(),
        Limits::default(),
    );
    let report = report.unwrap();
    let heights: Vec<&str> = report
        .findings
        .iter()
        .filter(|f| f.rule == "text-height")
        .map(|f| f.diagnostic.code().name())
        .collect();
    assert_eq!(heights, vec!["profile.text-height.unsupported"; 2]);
}

const CHILD: &str = r#"{
  "format": "0.2", "id": "office", "edition": "2025-12", "revision": 3,
  "extends": { "id": "mlit-cad", "edition": "2025-12" },
  "unknown": true,
  "vocabularies": { "sheet": { "values": [{ "width": 100, "height": 50 }] } },
  "rules": [
    { "id": "color", "check": "color", "level": "error", "vocabulary": "color" },
    { "id": "portrait", "check": "orientation", "level": "warning", "orientation": "portrait" },
    { "id": "future", "check": "title-block", "level": "info" }
  ]
}"#;

#[test]
fn resolves_extends_by_replacing_rules_and_vocabularies() {
    let drawing = drawing();
    let (report, diagnostics) = run(
        &drawing,
        &[("child.json", CHILD.as_bytes()), ("mlit.json", MLIT)],
        "office",
        "sxf",
        origins(&drawing, None, None).unwrap(),
        Limits::default(),
    );
    let report = report.unwrap();
    let chain: Vec<(&str, u64)> = report
        .chain
        .iter()
        .map(|p| (p.id.as_str(), p.revision))
        .collect();
    assert_eq!(chain, vec![("mlit-cad", 1), ("office", 3)]);
    // 知らない種類の規則は照合せず、診断で返す
    let codes: Vec<&str> = diagnostics.iter().map(|d| d.code().name()).collect();
    assert_eq!(codes, vec!["profile.rule.unsupported"]);
    let rules: Vec<&str> = report.rules.iter().map(|r| r.id.as_str()).collect();
    assert_eq!(
        rules,
        vec![
            "layer-name",
            "color",
            "line-type",
            "line-width",
            "text-height",
            "characters",
            "sheet-size",
            "orientation",
            "scale",
            "portrait",
        ]
    );
    let found = codes_of(&report);
    // 子が規則の重要度と、語彙を置き換えた
    assert!(found.contains(&("color", "profile.color.invalid", Severity::Error)));
    assert!(found.contains(&(
        "sheet-size",
        "profile.sheet-size.invalid",
        Severity::Warning
    )));
    assert!(found.contains(&("portrait", "profile.orientation.invalid", Severity::Warning)));
}

#[test]
fn stops_before_checking_when_the_profiles_cannot_be_used() {
    let drawing = drawing();
    let all = || origins(&drawing, None, None).unwrap();
    let error = |profiles: &[(&str, &[u8])], id: &str| {
        run(&drawing, profiles, id, "sxf", all(), Limits::default())
            .0
            .unwrap_err()
    };
    let code = |error: Error| match error {
        Error::Profile(diagnostic) => diagnostic.code().name(),
        other => panic!("{other:?}"),
    };
    assert_eq!(
        code(error(&[("child.json", CHILD.as_bytes())], "office")),
        "profile.extends.missing"
    );
    let a = r#"{"format":"0.1","id":"a","edition":"2025-12","revision":1,"extends":{"id":"b","edition":"2025-12"}}"#;
    let b = r#"{"format":"0.1","id":"b","edition":"2025-12","revision":1,"extends":{"id":"a","edition":"2025-12"}}"#;
    assert_eq!(
        code(error(&[("a", a.as_bytes()), ("b", b.as_bytes())], "a")),
        "profile.extends.invalid"
    );
    let newer = r#"{"format":"1.0","id":"a","edition":"2025-12","revision":1}"#;
    assert_eq!(
        code(error(&[("a", newer.as_bytes())], "a")),
        "profile.format.unsupported"
    );
    assert_eq!(
        code(error(&[("x", b"{")], "a")),
        "profile.format.unsupported"
    );
    assert_eq!(error(&[("mlit.json", MLIT)], "other"), Error::NotFound);
}

#[test]
fn counts_work_and_depth_and_stops_at_the_limits() {
    let drawing = drawing();
    let all = || origins(&drawing, None, None).unwrap();
    for (kind, limit) in [(Kind::Work, 20), (Kind::Depth, 1)] {
        let limits = Limits::default().with(kind, Limit::Finite(limit));
        let (result, _) = run(
            &drawing,
            &[("child.json", CHILD.as_bytes()), ("mlit.json", MLIT)],
            "office",
            "sxf",
            all(),
            limits,
        );
        assert!(
            matches!(result, Err(Error::Stop(Stop::Exceeded(_), _))),
            "{kind:?}"
        );
    }
}

#[test]
fn chooses_origins_and_rejects_unknown_ones() {
    let drawing = drawing();
    assert_eq!(
        origins(&drawing, None, None),
        Ok(vec![Origin::Sheet(SHEET)])
    );
    assert_eq!(
        origins(&drawing, Some("sheet/0"), Some(0)),
        Ok(vec![Origin::View {
            sheet: SHEET,
            view: 0
        }])
    );
    assert_eq!(origins(&drawing, Some("sheet/0"), Some(1)), Err("/view"));
    assert_eq!(origins(&drawing, Some("space/0"), Some(0)), Err("/view"));
    assert_eq!(origins(&drawing, Some("sheet/9"), None), Err("/from"));
    assert_eq!(origins(&drawing, None, Some(0)), Err("/view"));
    let empty = Drawing2D {
        spaces: drawing.spaces.clone(),
        ..Drawing2D::default()
    };
    assert_eq!(origins(&empty, None, None), Ok(vec![Origin::Space(MODEL)]));
}

#[test]
fn splits_scales_and_layer_names() {
    for (denominator, expected) in [(100.0, 1.0), (250.0, 2.5), (0.5, 5.0), (1.5, 1.5)] {
        assert!(
            (mantissa(denominator) - expected).abs() < 1e-9,
            "{denominator}"
        );
    }
    assert!((mantissa(99.999_999_9) - 1.0).abs() < 1e-6);
    let part = |codes: &[&str]| -> Vec<PartValue> {
        codes
            .iter()
            .map(|code| PartValue {
                code: code.to_string(),
                limits: if *code == "ORD" {
                    vec![(0, vec!["C".to_string()])]
                } else {
                    Vec::new()
                },
            })
            .collect()
    };
    let parts = vec![part(&["C", "D"]), part(&["STR", "ORD"])];
    let ok = |name: &str, user: bool| layer_name_ok(name, &parts, user, "-", Some(8));
    assert!(ok("C-STR", false));
    assert!(ok("C-ORD-x", true));
    assert!(!ok("D-ORD", true));
    assert!(!ok("C-STR-x", false));
    assert!(!ok("C", true));
    assert!(!ok("C-STR-ああ", true));
    assert!(ok("C-STR-ｱｱ", true));
}
