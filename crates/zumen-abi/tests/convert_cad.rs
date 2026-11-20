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
    lift_dxf(&case::source())
}

fn lift_dxf(input: &[u8]) -> Drawing2D {
    let mut budget = Budget::default();
    let mut diag = Diagnostics::new();
    let cancel = CancelToken::new();
    let mut cx = Cx::new(&mut budget, &mut diag, &cancel);
    zumen_acad::Dxf::read(input, &mut cx)
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

fn dimension_source() -> &'static str {
    // Standard はひな形にもある。寸法の表示部品はなく、個別に目盛りを取り消して文字を変える。
    concat!(
        "0\nSECTION\n2\nHEADER\n9\n$ACADVER\n1\nAC1021\n0\nENDSEC\n",
        "0\nSECTION\n2\nTABLES\n0\nTABLE\n2\nSTYLE\n",
        "0\nSTYLE\n5\nA1\n2\nNote\n70\n0\n40\n0\n41\n1\n50\n0\n3\ntxt\n",
        "0\nSTYLE\n5\nA4\n2\nCaption\n70\n0\n40\n0\n41\n1\n50\n0\n3\ntxt\n0\nENDTAB\n",
        "0\nTABLE\n2\nDIMSTYLE\n0\nDIMSTYLE\n105\nA2\n2\nStandard\n70\n0\n",
        "40\n2\n41\n1.25\n42\n0.25\n44\n0.75\n77\n4\n140\n2\n142\n0.6\n144\n1\n147\n0.5\n271\n1\n340\nA1\n",
        "0\nENDTAB\n0\nENDSEC\n0\nSECTION\n2\nENTITIES\n",
        "0\nDIMENSION\n5\nA3\n100\nAcDbEntity\n8\n0\n100\nAcDbDimension\n",
        "10\n20\n20\n8\n30\n0\n11\n10\n21\n8\n31\n0\n70\n32\n71\n5\n3\nStandard\n",
        "100\nAcDbAlignedDimension\n13\n0\n23\n0\n33\n0\n14\n20\n24\n0\n34\n0\n",
        "100\nAcDbRotatedDimension\n1001\nACAD\n1000\nDSTYLE\n1002\n{\n1070\n140\n1040\n3\n",
        "1070\n142\n1040\n0\n1070\n77\n1070\n1\n1070\n147\n1040\n0.25\n1070\n340\n1005\nA4\n1002\n}\n",
        "0\nLINE\n5\nA5\n8\n0\n10\n0\n20\n0\n11\n20\n21\n0\n",
        "0\nENDSEC\n0\nEOF\n",
    )
}

fn convert_dxf(input: &[u8]) -> Vec<u8> {
    use zumen_abi1::Session;
    let mut session = Session::new(b"").unwrap();
    session.push(b"dim.dxf", input).unwrap();
    case::call(&mut session, r#"{"v":1,"op":"open","input":"dim.dxf"}"#);
    case::call(
        &mut session,
        r#"{"v":1,"op":"convert","doc":"dim.dxf","format":"dxf","to":"out.dxf"}"#,
    );
    session.take(0).unwrap()
}

#[test]
fn new_dxf_keeps_dimension_style_and_uses_overrides_for_the_display_block() {
    use zumen_acad::{Data, Dxf};
    use zumen_drawing::DimensionDisplayTarget;

    let input = dimension_source();
    let bytes = convert_dxf(input.as_bytes());
    let mut budget = Budget::default();
    let mut diagnostics = Diagnostics::new();
    let cancel = CancelToken::new();
    let mut cx = Cx::new(&mut budget, &mut diagnostics, &cancel);
    let file = Dxf::read(&bytes, &mut cx).unwrap();
    let drawing = file.lift(&mut cx).unwrap();
    let style = drawing.dimension_style("Standard").unwrap();
    assert_eq!(
        (style.scale, style.size, style.tick),
        (2.0, 1.25, Some(0.6))
    );
    let base = drawing
        .dimension_display
        .iter()
        .find(|r| matches!(&r.target, DimensionDisplayTarget::Style(s) if s == "Standard"))
        .unwrap();
    assert_eq!(base.values.extension_offset, Some(0.25));
    assert_eq!(base.values.extension_beyond, Some(0.75));
    assert_eq!(base.values.text_height, Some(2.0));
    assert_eq!(base.values.text_vertical_placement, Some(4));
    assert_eq!(base.values.text_gap, Some(0.5));
    assert_eq!(base.values.text_style.as_deref(), Some("Note"));
    let local = drawing
        .dimension_display
        .iter()
        .find(|r| matches!(r.target, DimensionDisplayTarget::Element { .. }))
        .unwrap();
    assert_eq!(local.values.text_height, Some(3.0));
    assert_eq!(local.values.text_vertical_placement, Some(1));
    assert_eq!(local.values.text_gap, Some(0.25));
    assert_eq!(local.values.tick_size, Some(0.0));
    assert_eq!(local.values.text_style.as_deref(), Some("Caption"));
    let db = file.objects(&mut cx).unwrap();
    let label = db.objects().iter().find(|r| r.kind == b"MTEXT").unwrap();
    assert_eq!(label.first(40), Some(&Data::Real(6.0)));
    assert_eq!(label.first(7), Some(&Data::Text(b"Caption".to_vec())));
    assert_eq!(label.first(20), Some(&Data::Real(11.5)));
    let owner = db
        .objects()
        .iter()
        .find(|r| r.kind == b"DIMENSION")
        .unwrap();
    assert_eq!(owner.first(21), Some(&Data::Real(11.5)));
    assert_eq!(owner.first(70), Some(&Data::Int(32)));

    // 文字が手で配置されているときは、同じスタイル・個別設定でも元の Y を使う。
    let input = input.replace("70\n32\n71\n5", "70\n160\n71\n5");
    let manual = convert_dxf(input.as_bytes());
    let file = Dxf::read(&manual, &mut cx).unwrap();
    let db = file.objects(&mut cx).unwrap();
    let label = db.objects().iter().find(|r| r.kind == b"MTEXT").unwrap();
    assert_eq!(label.first(20), Some(&Data::Real(8.0)));
    let owner = db
        .objects()
        .iter()
        .find(|r| r.kind == b"DIMENSION")
        .unwrap();
    assert_eq!(owner.first(21), Some(&Data::Real(8.0)));
    assert_eq!(owner.first(70), Some(&Data::Int(160)));
}

#[test]
fn new_dxf_skips_invalid_dimension_overrides_and_preserves_inheritance() {
    use zumen_acad::{Data, Dxf, Reseat};
    use zumen_drawing::{
        DimensionDisplayRecord, DimensionDisplayTarget, DimensionDisplayValues, id,
    };

    let mut budget = Budget::default();
    let mut diagnostics = Diagnostics::new();
    let cancel = CancelToken::new();
    let mut cx = Cx::new(&mut budget, &mut diagnostics, &cancel);
    let mut model = Dxf::read(dimension_source().as_bytes(), &mut cx)
        .unwrap()
        .lift(&mut cx)
        .unwrap();
    for record in &mut model.dimension_display {
        match record.target {
            DimensionDisplayTarget::Style(_) => {
                // スタイル本体の 3 値を疎なレコード側の値で上書きしない。
                record.values.scale = Some(99.0);
                record.values.arrow_size = Some(98.0);
                record.values.tick_size = Some(97.0);
            }
            DimensionDisplayTarget::Element { .. } => {
                record.values.scale = Some(-1.0);
                record.values.tick_size = Some(-1.0);
                record.values.extension_offset = Some(-1.0);
                record.values.text_height = Some(0.0);
                record.values.linear_factor = Some(0.0);
                record.values.linear_precision = Some(9);
                record.values.text_vertical_placement = Some(5);
                record.values.text_gap = Some(f64::INFINITY);
            }
        }
    }
    let mut line_target = None;
    id::each_element(&model, |_, id, element| -> Result<(), ()> {
        if matches!(element, id::Found::Model(e) if matches!(e.shape, Shape::Line { .. })) {
            line_target = Some(DimensionDisplayTarget::Element {
                id,
                basis: element.hash(),
            });
        }
        Ok(())
    })
    .unwrap();
    model.dimension_display.push(DimensionDisplayRecord {
        target: line_target.unwrap(),
        values: DimensionDisplayValues {
            text_height: Some(7.0),
            ..Default::default()
        },
    });
    let output = zumen_acad::lower_to_dxf_with_report(&model, &Reseat, 100, &mut cx).unwrap();
    for (kind, count) in [
        ("dimension_display_value", 7),
        ("dimension_linear_precision", 1),
        ("dimension_display_target", 1),
    ] {
        assert!(
            output
                .report
                .counts
                .iter()
                .any(|r| r.kind == kind && r.count == count)
        );
    }
    let file = Dxf::read(&output.bytes, &mut cx).unwrap();
    let drawing = file.lift(&mut cx).unwrap();
    let style = drawing.dimension_style("Standard").unwrap();
    assert_eq!(
        (style.scale, style.size, style.tick),
        (2.0, 1.25, Some(0.6))
    );
    let overrides: Vec<_> = drawing
        .dimension_display
        .iter()
        .filter(|r| matches!(r.target, DimensionDisplayTarget::Element { .. }))
        .collect();
    let [local] = overrides.as_slice() else {
        panic!("expected one override")
    };
    assert_eq!(
        local.values,
        DimensionDisplayValues {
            text_style: Some("Caption".into()),
            ..Default::default()
        }
    );
    let db = file.objects(&mut cx).unwrap();
    let label = db.objects().iter().find(|r| r.kind == b"MTEXT").unwrap();
    assert_eq!(label.first(40), Some(&Data::Real(4.0)));
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
    #[derive(Debug)]
    enum Format {
        Sxf,
        Jww,
        Dxf,
    }
    let model = drawing();
    let dimensions = lift_dxf(dimension_source().as_bytes());
    for (format, input) in [
        (Format::Sxf, &model),
        (Format::Jww, &model),
        (Format::Dxf, &dimensions),
    ] {
        for kind in [Kind::Work, Kind::DecodedBytes] {
            let run = |limit| {
                let mut budget = Budget::new(Limits::default().with(kind, limit));
                let mut diagnostics = Diagnostics::new();
                let cancel = CancelToken::new();
                let mut cx = Cx::new(&mut budget, &mut diagnostics, &cancel);
                let result = match format {
                    Format::Sxf => zumen_sxf::lower_to_p21(input, 100, &mut cx).map(|_| ()),
                    Format::Jww => zumen_jww::lower_to_jww(input, 100, &mut cx).map(|_| ()),
                    Format::Dxf => zumen_acad::lower_to_dxf_with_report(
                        input,
                        &zumen_acad::Reseat,
                        100,
                        &mut cx,
                    )
                    .map(|_| ()),
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
                    diagnostics
                        .iter()
                        .filter(|d| d.code() == code)
                        .map(|d| d.count())
                        .sum::<u64>(),
                    1,
                    "{format:?} {kind:?} {limit}"
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
