//! 切り出した形・ID を保つ表現、属性だけによる記号、穴・縮尺・継承・停止の検査。
#![cfg(feature = "building-drawing")]
#![allow(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    clippy::panic,
    reason = "固定入力と期待値の試験"
)]
#[path = "support/drawing.rs"]
mod input;
use input::*;
use zumen_base::{Budget, CancelToken, Cx, Diagnostics, Kind, Limit, Limits};
use zumen_building::{drawing as b, *};
use zumen_drawing as d;
use zumen_profile::{building::Error, building_drawing::DrawingProfile};

#[test]
fn styles_hatches_sources_and_profile_survive_cut() {
    let p = profile();
    let b = building();
    let before = Hash::element(&b, ElementRef(0));
    let view = with(|cx| p.default_plan(&b, origin(), cx)).unwrap();
    let raw = with(|cx| {
        b::cut(
            &b,
            b::Options {
                view,
                tolerance: 0.00001,
                chord: 0.001,
                paper: p.defaults().paper(),
                symbols: &[],
            },
            cx,
        )
    })
    .unwrap();
    let lines = raw.lines.clone();
    let out = with(|cx| p.apply(&b, raw, cx)).unwrap();
    assert_eq!(out.output.lines, lines);
    assert_eq!(Hash::element(&b, ElementRef(0)), before);
    assert_eq!(out.chain[0].id, "zumen-building-drawing");
    assert!(
        out.output
            .drawing
            .extensions
            .iter()
            .all(|e| e.is_current(&out.output.drawing))
    );
    for line in &out.output.lines {
        let element = &out.output.drawing.spaces[0].elements[line.index];
        let width = match line.role {
            b::Role::Cut => 0.5,
            b::Role::Visible => 0.25,
            _ => 0.13,
        };
        assert_eq!(
            element.style.line_weight,
            Some(d::Inherit::Own(d::LineWeight::Width(d::PaperLength(width))))
        );
    }
    let hatch = out.decorations.iter().find(|d| d.kind == "hatch").unwrap();
    let d::Shape::Hatch(h) = &out.output.drawing.spaces[0].elements[hatch.index].shape else {
        panic!("hatch")
    };
    assert_eq!(h.loops.len(), 1);
    let d::Fill::Pattern(pattern) = &h.fill else {
        panic!("pattern")
    };
    assert_eq!(pattern.name, "zumen-concrete");
    assert!(
        (libm::hypot(pattern.lines[0].offset[0], pattern.lines[0].offset[1]) - 0.3).abs() < 1e-12
    );
    assert!(hatch.id.contains("/profile/hatch/0"));
}

#[test]
fn doors_use_explicit_width_operation_and_type_precedence() {
    let p = profile();
    for operation in [
        "SINGLE_SWING_LEFT",
        "SINGLE_SWING_RIGHT",
        "SLIDING_TO_LEFT",
        "SLIDING_TO_RIGHT",
    ] {
        let mut b = building();
        let at = door(&mut b, operation);
        let output = cut(&p, &b);
        let annotations: Vec<_> = output
            .output
            .lines
            .iter()
            .filter(|l| {
                output.output.sources[l.source].element == at && l.role == b::Role::Annotation
            })
            .collect();
        assert!(!annotations.is_empty(), "{operation}");
        let mut max_y = 0.0_f64;
        for line in annotations {
            let d::Shape::Line { start, end } =
                output.output.drawing.spaces[0].elements[line.index].shape
            else {
                panic!("line")
            };
            max_y = max_y.max(start.y).max(end.y);
            assert!(start.x >= 4.0 - 1e-9 && end.x <= 4.9 + 1e-9);
        }
        assert!(
            (max_y
                - if operation.starts_with("SINGLE") {
                    0.9
                } else {
                    0.27
                })
            .abs()
                < 1e-5
        );
        let ty = with(|cx| b.push(None, 100, cx)).unwrap();
        b.components.class.insert(
            ty,
            Class {
                name: "IfcDoorType".into(),
                original: "IfcDoorType".into(),
                predefined: None,
                user_defined: None,
            },
        );
        b.components.r#type.insert(at, TypeOf { target: ty });
        // occurrence に値があっても、割り当てた型の値が不明なら記号を作らない。
        assert!(
            !cut(&p, &b)
                .output
                .lines
                .iter()
                .any(|l| l.role == b::Role::Annotation)
        );
        b.components.extension.insert(
            ty,
            Extension {
                values: vec![attribute(
                    "OperationType",
                    "IfcDoorTypeOperationEnum",
                    Literal::Enumeration(operation.into()),
                )],
            },
        );
        assert!(
            cut(&p, &b)
                .output
                .lines
                .iter()
                .any(|l| l.role == b::Role::Annotation)
        );
    }
    for missing in ["OverallWidth", "OperationType"] {
        let mut b = building();
        let at = door(&mut b, "SINGLE_SWING_LEFT");
        let mut extension = b.components.extension.remove(at).unwrap();
        extension.values.retain(|v| v.name != missing);
        b.components.extension.insert(at, extension);
        assert!(
            !cut(&p, &b)
                .output
                .lines
                .iter()
                .any(|l| l.role == b::Role::Annotation)
        );
    }
}

#[test]
fn missing_or_ambiguous_door_operations_report_the_source_element() {
    let p = profile();
    for (operations, empty_type, reason) in [
        (0, false, "operation_missing"),
        (2, false, "operation_ambiguous"),
        (1, true, "operation_missing"),
    ] {
        let mut b = Building::new(Document {
            units: vec![unit()],
            ..Document::default()
        });
        let at = with(|cx| b.push(None, 1, cx)).unwrap();
        let class = |name: &str| Class {
            name: name.into(),
            original: name.into(),
            predefined: None,
            user_defined: None,
        };
        b.components.class.insert(at, class("IfcDoor"));
        let mut values = vec![attribute(
            "OverallWidth",
            "IfcPositiveLengthMeasure",
            Literal::Real(0.9),
        )];
        for _ in 0..operations {
            values.push(attribute(
                "OperationType",
                "IfcDoorTypeOperationEnum",
                Literal::Enumeration("SINGLE_SWING_LEFT".into()),
            ));
        }
        b.components.extension.insert(at, Extension { values });
        if empty_type {
            let ty = with(|cx| b.push(None, 2, cx)).unwrap();
            b.components.class.insert(ty, class("IfcDoorType"));
            b.components.r#type.insert(at, TypeOf { target: ty });
        }
        let view = with(|cx| p.default_plan(&b, origin(), cx)).unwrap();
        let mut diagnostics = Diagnostics::new();
        let out = p
            .cut(
                &b,
                view,
                0.00001,
                0.001,
                None,
                &mut Cx::new(
                    &mut Budget::default(),
                    &mut diagnostics,
                    &CancelToken::new(),
                ),
            )
            .unwrap();
        assert!(out.output.lines.is_empty());
        let reports: Vec<_> = diagnostics
            .iter()
            .filter(|d| d.code() == zumen_base::codes::PROFILE_BUILDING_DRAWING_UNSUPPORTED)
            .collect();
        assert_eq!(reports.len(), 1, "{diagnostics:?}");
        assert!(reports[0].places().iter().any(|p| p.id() == Some("i1")));
        assert!(
            reports[0]
                .params()
                .iter()
                .any(|(k, v)| *k == "reason" && *v == zumen_base::Value::Text(reason.into()))
        );
    }
}

#[test]
fn instances_of_one_shape_are_unioned_without_making_false_holes() {
    let p = profile();
    for (scale, translation, area) in [(0.5, [0.0; 3], 2.0), (1.0, [0.5, 0.25, 0.0], 2.875)] {
        let mut b = building();
        mapped(&mut b, scale, translation);
        let out = cut(&p, &b);
        assert!(out.output.complete);
        let hatches: Vec<_> = out
            .decorations
            .iter()
            .filter(|d| d.kind == "hatch")
            .collect();
        assert_eq!(hatches.len(), 1);
        let d::Shape::Hatch(h) = &out.output.drawing.spaces[0].elements[hatches[0].index].shape
        else {
            panic!("hatch")
        };
        assert_eq!(h.loops.len(), 1);
        let d::Boundary::Polyline { vertices, .. } = &h.loops[0].boundary else {
            panic!("loop")
        };
        let measured = vertices
            .iter()
            .zip(vertices.iter().cycle().skip(1))
            .take(vertices.len())
            .map(|(a, b)| a.point.x * b.point.y - b.point.x * a.point.y)
            .sum::<f64>()
            / 2.0;
        assert!((measured - area).abs() < 1e-8, "{scale}: {measured}");
    }
}

#[test]
fn holes_remain_empty_and_grid_labels_keep_their_axis() {
    let p = profile();
    let mut b = building();
    hole(&mut b);
    let axis = grid(&mut b);
    let out = cut(&p, &b);
    let hatch = out.decorations.iter().find(|d| d.kind == "hatch").unwrap();
    let d::Shape::Hatch(h) = &out.output.drawing.spaces[0].elements[hatch.index].shape else {
        panic!("hatch")
    };
    assert_eq!(h.loops.len(), 2);
    assert_eq!(h.islands, d::Islands::Nested);
    let signed_area = |l: &d::HatchLoop<d::ModelPoint>| {
        let d::Boundary::Polyline { vertices, .. } = &l.boundary else {
            panic!("loop")
        };
        vertices
            .iter()
            .zip(vertices.iter().cycle().skip(1))
            .take(vertices.len())
            .map(|(a, b)| a.point.x * b.point.y - b.point.x * a.point.y)
            .sum::<f64>()
            / 2.0
    };
    assert!((h.loops.iter().map(signed_area).sum::<f64>() - 1.8).abs() < 1e-8);
    let labels: Vec<_> = out
        .decorations
        .iter()
        .filter(|d| d.kind == "grid-label")
        .collect();
    assert_eq!(labels.len(), 2);
    for label in labels {
        assert_eq!(label.element, axis);
        let d::Shape::Text(text) = &out.output.drawing.spaces[0].elements[label.index].shape else {
            panic!("label")
        };
        assert_eq!(text.content, "X1");
        assert_eq!(text.height.value, d::Length::Paper(d::PaperLength(4.0)));
    }
    assert_eq!(
        out.decorations
            .iter()
            .filter(|d| d.kind == "grid-circle")
            .count(),
        2
    );
    assert!(
        out.output
            .drawing
            .extensions
            .iter()
            .all(|e| e.is_current(&out.output.drawing))
    );
}

#[test]
fn level_names_keep_source_values_and_do_not_make_heights() {
    let p = profile();
    for (name, kind) in [
        ("設計GL", "ground"),
        ("1FL", "floor"),
        ("2FL", "floor"),
        ("3FL", "floor"),
        ("RFL", "roof"),
        ("最高高さ", "maximum-height"),
        ("UG", "floor"),
        ("EG", "floor"),
        ("1. OG", "floor"),
        ("Ground Floor", "floor"),
        ("1. Story", "floor"),
        ("Level 1", "floor"),
    ] {
        assert_eq!(with(|cx| p.level_kind(name, cx)).unwrap(), Some(kind));
    }
    for unknown in ["FL", "RF", "Level ?", "1e1FL", "1.5FL", ""] {
        assert_eq!(with(|cx| p.level_kind(unknown, cx)).unwrap(), None);
    }
}

#[test]
fn defaults_convert_meters_without_changing_the_document() {
    let p = profile();
    let mut b = building();
    b.document.units[0].prefix = Some("MILLI".into());
    let view = with(|cx| p.default_plan(&b, origin(), cx)).unwrap();
    assert!(matches!(
        view,
        b::View::Plan {
            height: 1200.0,
            depth: 3000.0,
            ..
        }
    ));
    b.document.units.clear();
    assert!(with(|cx| p.default_plan(&b, origin(), cx)).is_err());
}

#[test]
fn extends_changes_values_but_not_source_ids() {
    let child = br#"{"format":"0.1","id":"test","edition":"1","revision":1,"extends":{"id":"zumen-building-drawing","edition":"2026-11"},"vocabularies":{"drawing-hatches":{"values":[{"name":"test","materials":["Concrete"],"angles":[30],"pitch":5}]}}}"#;
    let p = with(|cx| {
        DrawingProfile::load(
            &[("base", BUILDING_DRAWING), ("child", child)],
            "test",
            "1",
            cx,
        )
    })
    .unwrap();
    let b = building();
    let base = cut(&profile(), &b);
    let child = cut(&p, &b);
    assert_ne!(base.hash, child.hash);
    assert_eq!(base.output.lines, child.output.lines);
    assert_eq!(base.decorations, child.decorations);
}

#[test]
fn malformed_settings_and_low_budgets_never_return_partial_drawings() {
    let input = std::str::from_utf8(BUILDING_DRAWING).unwrap();
    for input in [
        input.replace("\"pitch\": 3", "\"pitch\": 0"),
        input.replace("\"solid\"", "\"bad\""),
        input.replace("\"radius\": 4", "\"radius\": -4"),
    ] {
        // JSON は生成器が整形する。変更が実際に入っていることも確かめる。
        assert_ne!(input.as_bytes(), BUILDING_DRAWING);
        let result = with(|cx| {
            DrawingProfile::load(
                &[("bad", input.as_bytes())],
                "zumen-building-drawing",
                "2026-11",
                cx,
            )
        });
        // 線種の一斉改名は合法なので、その場合は読み込める。
        if !input.contains("\"bad\"") {
            assert!(result.is_err());
        }
    }
    let p = profile();
    let b = building();
    let view = with(|cx| p.default_plan(&b, origin(), cx)).unwrap();
    for kind in [Kind::Work, Kind::DecodedBytes, Kind::Segments] {
        let mut budget = Budget::new(Limits::default().with(kind, Limit::Finite(0)));
        let result = p.cut(
            &b,
            view,
            0.00001,
            0.001,
            None,
            &mut Cx::new(&mut budget, &mut Diagnostics::new(), &CancelToken::new()),
        );
        assert!(matches!(
            result,
            Err(Error::Stopped(_)) | Err(Error::Geometry(zumen_geom3d::Failure::Stopped(_)))
        ));
    }
    let cancel = CancelToken::new();
    cancel.cancel();
    assert!(
        p.cut(
            &b,
            view,
            0.00001,
            0.001,
            None,
            &mut Cx::new(&mut Budget::default(), &mut Diagnostics::new(), &cancel)
        )
        .is_err()
    );
}
