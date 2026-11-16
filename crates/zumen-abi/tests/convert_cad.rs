//! 別形式への変換で保つ値・欠落・予算と、失敗後のセッションの契約。
#![cfg(feature = "drawing")]
#![allow(clippy::unwrap_used, clippy::panic)] // 自作入力と固定の期待値。
#[path = "../examples/convert_cad_case.rs"]
mod case;
use zumen_abi1::Status;
use zumen_base::{Budget, CancelToken, Cx, Diagnostics, Ref};
use zumen_drawing::{Drawing2D, LayerId, Shape, SpaceId, Unit};
use zumen_sdk::{Codec, Lift, conversion::ConversionReport};

fn drawing() -> Drawing2D {
    let mut budget = Budget::default();
    let mut diag = Diagnostics::new();
    let cancel = CancelToken::new();
    let mut cx = Cx::new(&mut budget, &mut diag, &cancel);
    zumen_acad::Dxf::read(&case::source(), &mut cx)
        .unwrap()
        .lift(&mut cx)
        .unwrap()
}
fn lower(drawing: &Drawing2D, format: &str) -> (Vec<u8>, Drawing2D, ConversionReport, u64) {
    let mut budget = Budget::default();
    let mut diag = Diagnostics::new();
    let cancel = CancelToken::new();
    let mut cx = Cx::new(&mut budget, &mut diag, &cancel);
    if format == "sxf" {
        let out = zumen_sxf::lower_to_p21(drawing, 1000, &mut cx).unwrap();
        let reloaded = zumen_sxf::Sxf::read(&out.bytes, &mut cx)
            .unwrap()
            .lift(&mut cx)
            .unwrap();
        (out.bytes, reloaded, out.report, out.elements)
    } else {
        let out = zumen_jww::lower_to_jww(drawing, 1000, &mut cx).unwrap();
        let reloaded = zumen_jww::Jww::read(&out.bytes, &mut cx)
            .unwrap()
            .lift(&mut cx)
            .unwrap();
        (out.bytes, reloaded, out.report, out.elements)
    }
}
#[test]
fn public_estimates_match_written_bytes_and_reopened_values() {
    let steps = case::steps(None);
    assert!(!steps.is_empty());
}
#[test]
fn geometry_colors_weights_and_line_patterns_survive() {
    for format in ["sxf", "jww"] {
        let (_, read, report, count) = lower(&drawing(), format);
        assert_eq!(count, if format == "sxf" { 6 } else { 7 });
        assert!(
            report
                .counts
                .iter()
                .any(|c| c.kind == "spline" && c.count == 1 && c.output_elements == 0)
        );
        assert!(read.line_types.iter().any(|line| {
            line.definition
                .as_ref()
                .is_some_and(|d| d.pattern == [2.0, -1.0])
        }));
        if format == "sxf" {
            let elements = &read.sheets[0].elements;
            assert!(elements.iter().any(|element| matches!(&element.shape,
                Shape::Line {start,end} if [start.x,start.y,end.x,end.y]==[1.0,2.0,11.0,12.0])));
            assert!(elements.iter().any(|element| matches!(&element.shape,
                Shape::Circle {center,..} if [center.x,center.y]==[20.0,30.0])));
            assert!(elements.iter().all(|element| matches!(element.style.color,
                zumen_drawing::Inherit::Own(color) if color.rgb==Some([255,0,0]))));
        } else {
            let elements = &read.spaces[0].elements;
            assert!(elements.iter().any(|element| matches!(&element.shape,
                Shape::Line {start,end} if [start.x,start.y,end.x,end.y]==[1.0,2.0,11.0,12.0])));
            assert!(elements.iter().all(|element| matches!(element.style.color,
                zumen_drawing::Inherit::Own(color) if color.rgb==Some([255,0,0]))));
        }
    }
}
#[test]
fn known_units_are_converted_to_millimeters_and_new_spaces_stay_separate() {
    let mut model = drawing();
    model.spaces[0].unit = Unit::Meter;
    model.spaces[0].elements.truncate(1);
    model.spaces[0].order = vec![0];
    for format in ["sxf", "jww"] {
        let (_, read, _, count) = lower(&model, format);
        assert_eq!(count, 1);
        let xy = if format == "sxf" {
            let Shape::Line { start, .. } = read.sheets[0].elements[0].shape else {
                panic!()
            };
            [start.x, start.y]
        } else {
            let Shape::Line { start, .. } = read.spaces[0].elements[0].shape else {
                panic!()
            };
            [start.x, start.y]
        };
        assert_eq!(xy, [1000.0, 2000.0]);
    }
    let mut second = model.spaces[0].clone();
    second.id = SpaceId(99);
    if let Shape::Line { start, end } = &mut second.elements[0].shape {
        start.space = second.id;
        end.space = second.id;
    }
    model.spaces.push(second);
    let (_, read, _, count) = lower(&model, "jww");
    assert_eq!(count, 2);
    assert_eq!(read.spaces.len(), 2);
    assert_ne!(read.spaces[0].id, read.spaces[1].id);
}
#[test]
fn long_unicode_layer_names_roundtrip_and_seeds_are_removed() {
    let mut model = drawing();
    let name = format!("層'\\{}", "あ".repeat(300));
    model.layers[0].name = name.clone();
    for format in ["sxf", "jww"] {
        let (_, read, _, count) = lower(&model, format);
        assert!(read.layers.iter().any(|l| l.name == name));
        let total = read.spaces.iter().map(|s| s.elements.len()).sum::<usize>()
            + read.sheets.iter().map(|s| s.elements.len()).sum::<usize>();
        assert_eq!(count, total as u64);
    }
}
#[test]
fn jww_layer_limit_drops_only_the_overflow_and_reports_its_source() {
    let mut model = drawing();
    let element = model.spaces[0].elements[0].clone();
    let layer = model.layers[0].clone();
    model.layers.clear();
    model.spaces[0].elements.clear();
    model.spaces[0].order.clear();
    for i in 0..257 {
        let id = LayerId(i + 1);
        let mut layer = layer.clone();
        layer.id = id;
        layer.name = format!("L{i}");
        model.layers.push(layer);
        let mut element = element.clone();
        element.layer = id;
        element.source = Ref::at_record(1000 + u64::from(i));
        model.spaces[0].elements.push(element);
        model.spaces[0].order.push(i as usize);
    }
    let (_, read, report, count) = lower(&model, "jww");
    assert_eq!(count, 256);
    assert_eq!(read.spaces.len(), 16);
    let dropped = report
        .details
        .iter()
        .find(|d| d.kind == "line" && d.output_elements == 0)
        .unwrap();
    assert_eq!(dropped.source.record(), Some(1256));
}
#[test]
fn rejected_text_does_not_discard_the_other_supported_geometry() {
    let mut model = drawing();
    for element in &mut model.spaces[0].elements {
        if let Shape::Text(text) = &mut element.shape {
            text.oblique = 20.0;
        }
    }
    let (_, _, report, count) = lower(&model, "jww");
    assert_eq!(count, 6);
    assert!(
        report
            .counts
            .iter()
            .any(|c| c.kind == "text" && c.output_elements == 0 && c.count == 1)
    );
}
#[test]
fn empty_drawings_produce_readable_empty_files() {
    for format in ["sxf", "jww"] {
        let (_, read, _, count) = lower(&Drawing2D::default(), format);
        assert_eq!(count, 0);
        assert!(read.spaces.iter().all(|s| s.elements.is_empty()));
        assert!(read.sheets.iter().all(|s| s.elements.is_empty()));
    }
}

#[test]
fn cancellation_does_not_register_output() {
    for format in ["sxf", "jww"] {
        let mut session = case::open();
        let cancel = CancelToken::new();
        cancel.cancel();
        assert_ne!(
            session
                .call(case::request(format, false, 0).as_bytes(), &cancel)
                .status,
            Status::Ok
        );
        assert!(session.take(0).is_none());
        case::call(&mut session, &case::request(format, false, 0));
    }
}

#[test]
fn model_draw_order_is_kept() {
    let mut model = drawing();
    model.spaces[0].elements.truncate(2);
    model.spaces[0].order = vec![1, 0];
    for format in ["sxf", "jww"] {
        let (_, read, _, count) = lower(&model, format);
        assert_eq!(count, 2);
        let first = if format == "sxf" {
            zumen_drawing::transfer::kind(&read.sheets[0].elements[0].shape)
        } else {
            zumen_drawing::transfer::kind(&read.spaces[0].elements[0].shape)
        };
        assert_eq!(first, "circle");
    }
}
#[test]
fn failed_conversion_publishes_neither_document_nor_blob_and_can_be_retried() {
    for format in ["sxf", "jww"] {
        let mut session = case::open();
        let request = case::request(format, false, 0);
        let failed = request.replace("\"to\":\"out\"", "\"to\":\"out\",\"budget\":{\"work\":1}");
        let result = session.call(failed.as_bytes(), &CancelToken::new());
        assert_eq!(result.status, Status::Failed);
        assert!(session.take(0).is_none());
        let absent = session.call(
            br#"{"v":1,"op":"drawing","doc":"out"}"#,
            &CancelToken::new(),
        );
        assert_eq!(absent.status, Status::Failed);
        case::call(&mut session, &request);
        assert!(session.take(0).is_some());
    }
}

#[test]
fn stops_at_different_phases_have_one_diagnostic() {
    use zumen_base::{Kind, Limit, Limits, codes};
    let model = drawing();
    for format in ["sxf", "jww"] {
        for kind in [Kind::Work, Kind::DecodedBytes] {
            let run = |limit| {
                let mut budget = Budget::new(Limits::default().with(kind, limit));
                let mut diagnostics = Diagnostics::new();
                let cancel = CancelToken::new();
                let mut cx = Cx::new(&mut budget, &mut diagnostics, &cancel);
                let result = if format == "sxf" {
                    zumen_sxf::lower_to_p21(&model, 100, &mut cx).map(|_| ())
                } else {
                    zumen_jww::lower_to_jww(&model, 100, &mut cx).map(|_| ())
                };
                (result, budget.used(kind), diagnostics)
            };
            let (result, used, _) = run(Limit::Unlimited);
            assert!(result.is_ok());
            for limit in (0..used)
                .step_by((used / 25).max(1) as usize)
                .chain([used - 1])
            {
                let (result, _, diagnostics) = run(Limit::Finite(limit));
                assert!(matches!(result, Err(zumen_base::Error::Stopped(_))));
                let code = if kind == Kind::Work {
                    codes::BUDGET_WORK_EXCEEDED
                } else {
                    codes::BUDGET_DECODED_BYTES_EXCEEDED
                };
                assert_eq!(
                    diagnostics.iter().filter(|d| d.code() == code).count(),
                    1,
                    "{format} {kind:?} {limit}"
                );
                assert!(
                    !diagnostics
                        .iter()
                        .any(|d| d.code() == codes::INTERNAL_PANIC_CAUGHT)
                );
            }
        }
    }
}

#[test]
fn p21_uses_first_eligible_view_and_keeps_custom_paper_size() {
    use zumen_drawing::{Affine, PaperLength, Sheet, SheetId, View};
    let mut model = drawing();
    model.spaces[0].elements.truncate(1);
    model.spaces[0].order = vec![0];
    let sheet = SheetId(12);
    model.sheets = vec![Sheet {
        id: sheet,
        name: Some("custom".into()),
        size: Some([PaperLength(650.0), PaperLength(450.0)]),
        elements: vec![],
        order: vec![],
        source: Ref::default(),
        views: vec![View {
            space: model.spaces[0].id,
            sheet,
            transform: Affine {
                m: [0.5, 0.0, 100.0, 0.0, 0.5, 200.0],
            },
            clip: vec![],
            hidden_layers: vec![],
            source: Ref::at_record(99),
        }],
    }];
    let (_, read, report, count) = lower(&model, "sxf");
    assert_eq!(count, 1);
    assert_eq!(
        read.sheets[0].size,
        Some([PaperLength(650.0), PaperLength(450.0)])
    );
    let Shape::Line { start, end } = read.sheets[0].elements[0].shape else {
        panic!()
    };
    assert_eq!(
        [start.x, start.y, end.x, end.y],
        [100.5, 201.0, 105.5, 206.0]
    );
    assert!(report.details.iter().any(|d| d.kind == "view"
        && d.source.record() == Some(99)
        && d.fidelity == zumen_sdk::conversion::ConversionFidelity::Dropped));
}
