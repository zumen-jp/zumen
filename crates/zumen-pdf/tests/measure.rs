//! #520: 紙の図形を動かさず、計測辞書の原値と汎用の距離校正を保持する。
#![cfg(test)]
#![cfg(feature = "drawing")]

use zumen_base::{Budget, CancelToken, Cx, Diagnostic, Diagnostics, Kind, Limit, Limits};
use zumen_drawing::{
    Basis, Calibration, Drawing2D, Extension, FieldValue, Hash, MeasurementError, PaperPoint,
    SheetId, Target, Unit, id,
};
use zumen_pdf::Pdf;
use zumen_sdk::{Error, Lift, testkit};

mod common;
#[path = "common/measure.rs"]
mod fixture;

use fixture::Viewport;

fn count(diagnostics: &[Diagnostic], code: &str) -> u64 {
    diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.code().name() == code)
        .map(Diagnostic::count)
        .sum()
}

fn calibrations(drawing: &Drawing2D) -> Vec<Calibration> {
    let mut budget = Budget::default();
    let mut diag = Diagnostics::new();
    let cancel = CancelToken::new();
    let values = drawing
        .calibrations(&mut Cx::new(&mut budget, &mut diag, &cancel))
        .unwrap();
    assert!(diag.is_empty(), "{diag:?}");
    values
}

fn originals(drawing: &Drawing2D) -> Vec<&Extension> {
    drawing
        .extensions
        .iter()
        .filter(|extension| extension.namespace == "pdf.measure")
        .collect()
}

fn field<'a>(extension: &'a Extension, key: &str) -> &'a FieldValue {
    &extension
        .fields
        .iter()
        .find(|field| field.key == key)
        .unwrap_or_else(|| panic!("原値がない: {key}"))
        .value
}

fn paper(x: f64, y: f64) -> PaperPoint {
    PaperPoint {
        sheet: SheetId(0),
        x: common::mm(x),
        y: common::mm(y),
    }
}

fn near(found: f64, expected: f64) {
    assert!((found - expected).abs() < 1e-10, "{found} != {expected}");
}

fn unchanged_page(input: &fixture::Fixture, drawing: &Drawing2D) {
    let (baseline, _) = common::lift(&input.without_viewports);
    assert_eq!(drawing.sheets, baseline.sheets);
    assert_eq!(drawing.spaces, baseline.spaces);
    assert_eq!(drawing.layers, baseline.layers);
    assert_eq!(drawing.line_types, baseline.line_types);
    assert_eq!(drawing.text_styles, baseline.text_styles);
    let mut before = id::Namer::new("sheet/0");
    let mut after = id::Namer::new("sheet/0");
    for (index, (new, old)) in drawing.sheets[0]
        .elements
        .iter()
        .zip(&baseline.sheets[0].elements)
        .enumerate()
    {
        assert_eq!(new.source, old.source);
        assert_eq!(Hash::element(new), Hash::element(old));
        assert_eq!(after.next(index, new), before.next(index, old));
    }
}

#[test]
fn measures_isotropic_anisotropic_signed_and_unknown_units_without_parsing_r() {
    for (measure, delta, expected, unit, label) in [
        (
            fixture::rl("1", None, ""),
            [3.0, 4.0],
            5.0,
            Unit::Meter,
            "m",
        ),
        (
            fixture::rl("2", Some("1"), ""),
            [3.0, 8.0],
            10.0,
            Unit::Meter,
            "m",
        ),
        (
            fixture::rl("-2", Some("-1"), ""),
            [3.0, 8.0],
            10.0,
            Unit::Meter,
            "m",
        ),
        (
            fixture::rl("2", Some("0.5"), "/CYX 2"),
            [3.0, 8.0],
            10.0,
            Unit::Meter,
            "m",
        ),
        (
            fixture::rl("2", Some("1"), "/CYX -1"),
            [3.0, 8.0],
            10.0,
            Unit::Meter,
            "m",
        ),
        (
            fixture::rl("1", None, "/CYX 9"),
            [3.0, 4.0],
            5.0,
            Unit::Meter,
            "m",
        ),
        (
            fixture::rl("1", None, "/CYX 0"),
            [3.0, 4.0],
            5.0,
            Unit::Meter,
            "m",
        ),
        (
            fixture::rl_units("(cubit)", "1", None, ""),
            [3.0, 4.0],
            5.0,
            Unit::Unknown,
            "cubit",
        ),
        (
            fixture::rl_units("(span)", "2", Some(("(palm)", "0.5")), "/CYX 2"),
            [3.0, 8.0],
            10.0,
            Unit::Unknown,
            "span",
        ),
        (
            fixture::rl_units("( Feet )", "1", None, ""),
            [3.0, 4.0],
            5.0,
            Unit::Foot,
            " Feet ",
        ),
    ] {
        let input = fixture::make(&[Viewport::new(&measure)]);
        let (drawing, diagnostics) = common::lift(&input.bytes);
        let values = calibrations(&drawing);
        assert_eq!(values.len(), 1, "{diagnostics:?}");
        assert_eq!(values[0].id, "sheet/0/vp/0");
        assert_eq!(values[0].unit, unit);
        assert_eq!(values[0].unit_label, label);
        near(
            values[0]
                .measure(paper(10.0, 10.0), paper(10.0 + delta[0], 10.0 + delta[1]))
                .unwrap(),
            expected,
        );
        assert_eq!(count(&diagnostics, "pdf.measure-distance.equivalent"), 1);
        assert_eq!(count(&diagnostics, "pdf.measure.unsupported"), 0);
        assert_eq!(
            count(&diagnostics, "pdf.measure.invalid"),
            0,
            "{diagnostics:?}"
        );
        assert_eq!(
            field(originals(&drawing)[0], "r"),
            &FieldValue::Text("source scale text".to_owned())
        );
        unchanged_page(&input, &drawing);
    }
}

#[test]
fn composes_rotation_crop_and_user_unit_into_paper_calibration() {
    let input = fixture::with_page(
        fixture::PAGE,
        "/CropBox [10 20 110 120] /Rotate 90 /UserUnit 2",
        &[Viewport::new(&fixture::rl("2", Some("1"), "")).bbox("[10 20 110 120]")],
    );
    let (drawing, diagnostics) = common::lift(&input.bytes);
    let values = calibrations(&drawing);
    assert_eq!(values.len(), 1, "{diagnostics:?}");
    let calibration = &values[0];
    // PDF の (20,30)→(23,38) は、crop/90度/UserUnit=2 の紙上では (20,180)→(36,174) pt。
    near(
        calibration
            .measure(paper(20.0, 180.0), paper(36.0, 174.0))
            .unwrap(),
        10.0,
    );
    near(calibration.bounds[0], 0.0);
    near(calibration.bounds[1], 0.0);
    near(calibration.bounds[2], common::mm(200.0));
    near(calibration.bounds[3], common::mm(200.0));
    unchanged_page(&input, &drawing);
}

#[test]
fn preserves_all_number_formats_text_bytes_names_booleans_and_origin() {
    let input =
        fixture::make(&[Viewport::new(&fixture::full_formats()).name("<FEFF5E73976256F3>")]);
    let (drawing, diagnostics) = common::lift(&input.bytes);
    let values = calibrations(&drawing);
    assert_eq!(values.len(), 1, "{diagnostics:?}");
    assert_eq!(values[0].name.as_deref(), Some("平面図"));
    assert_eq!(values[0].unit_label, "尺");
    assert_eq!(values[0].unit, Unit::Unknown);
    near(
        values[0]
            .measure(paper(20.0, 20.0), paper(23.0, 24.0))
            .unwrap(),
        8.48528137423857,
    );
    let raw = originals(&drawing)[0];
    assert_eq!(raw.name, "sheet/0/vp/0");
    assert_eq!(raw.schema, 1);
    assert_eq!(raw.target, Target::Document);
    assert_eq!(raw.basis, Basis::Document);
    assert_eq!(raw.source, values[0].source);
    assert_eq!(
        raw.source.record(),
        Some(u64::from(input.viewport_objects[0]) + 1)
    );
    for (key, expected) in [
        ("viewport/name", FieldValue::Text("平面図".to_owned())),
        ("r", FieldValue::Text("縮尺".to_owned())),
        (
            "r/bytes",
            FieldValue::Bytes(vec![0xfe, 0xff, 0x7e, 0x2e, 0x5c, 0x3a]),
        ),
        ("r/raw", FieldValue::Bytes(b"<FEFF7E2E5C3A>".to_vec())),
        ("x/count", FieldValue::Integer(2)),
        ("x/0/u", FieldValue::Text("尺".to_owned())),
        ("x/0/u/type", FieldValue::Text("string".to_owned())),
        (
            "x/0/u/bytes",
            FieldValue::Bytes(vec![0xfe, 0xff, 0x5c, 0x3a]),
        ),
        ("x/0/c", FieldValue::Integer(2)),
        ("x/0/f", FieldValue::Text("F".to_owned())),
        ("x/0/f/type", FieldValue::Text("name".to_owned())),
        ("x/0/d", FieldValue::Integer(16)),
        ("x/0/fd", FieldValue::Integer(1)),
        ("x/0/fd/type", FieldValue::Text("boolean".to_owned())),
        ("x/0/rt", FieldValue::Text(",".to_owned())),
        ("x/0/rt/raw", FieldValue::Bytes(br"(\054)".to_vec())),
        ("x/0/rd", FieldValue::Text(".".to_owned())),
        ("x/0/ps", FieldValue::Text("<".to_owned())),
        ("x/0/ss", FieldValue::Text(">".to_owned())),
        ("x/0/o", FieldValue::Text("P".to_owned())),
        ("x/1/u", FieldValue::Text("in".to_owned())),
        ("x/1/c", FieldValue::Integer(12)),
        ("d/count", FieldValue::Integer(2)),
        ("d/1/u", FieldValue::Text("in".to_owned())),
        ("a/count", FieldValue::Integer(1)),
        ("a/0/c", FieldValue::Real(0.25)),
        ("t/0/u", FieldValue::Text("degree".to_owned())),
        ("s/0/u", FieldValue::Text("rise".to_owned())),
        ("o/count", FieldValue::Integer(2)),
        ("o/0", FieldValue::Real(12.5)),
        ("o/1", FieldValue::Real(-3.25)),
        ("cyx", FieldValue::Real(0.5)),
    ] {
        assert_eq!(field(raw, key), &expected, "{key}");
    }
    let FieldValue::Bytes(item) = field(raw, "x/0/raw") else {
        panic!("元の辞書でない")
    };
    assert!(
        item.windows(b"/Private << /Kept true >>".len())
            .any(|bytes| bytes == b"/Private << /Kept true >>")
    );
    assert_eq!(
        count(&diagnostics, "pdf.measure.invalid"),
        0,
        "{diagnostics:?}"
    );
    unchanged_page(&input, &drawing);
}

#[test]
fn preserves_references_and_distinguishes_missing_null_unresolved_and_wrong_types() {
    let input = fixture::with_objects(
        fixture::PAGE,
        "",
        &[
            b"<< /Type /NumberFormat /U (m) /C 1 >>".to_vec(),
            b"<< /Type /Measure /Subtype /RL /R (source) /X [5 0 R] /D [] /A [] >>".to_vec(),
        ],
        &[Viewport::new("6 0 R")],
    );
    let (drawing, diagnostics) = common::lift(&input.bytes);
    assert_eq!(calibrations(&drawing).len(), 1, "{diagnostics:?}");
    let raw = originals(&drawing)[0];
    assert_eq!(field(raw, "measure/ref/object"), &FieldValue::Integer(6));
    assert_eq!(field(raw, "x/0/ref/object"), &FieldValue::Integer(5));
    assert_eq!(field(raw, "x/0/ref/generation"), &FieldValue::Integer(0));
    assert_eq!(raw.source.record(), Some(7));
    for (measure, kind, reason) in [
        ("null", "null", "type"),
        ("99 0 R", "unresolved", "unresolved"),
        ("17", "number", "type"),
    ] {
        let input = fixture::make(&[Viewport::new(measure)]);
        let (drawing, diagnostics) = common::lift(&input.bytes);
        assert!(calibrations(&drawing).is_empty());
        assert_eq!(count(&diagnostics, "pdf.measure.unsupported"), 1);
        assert_eq!(
            count(&diagnostics, "pdf.measure.invalid"),
            1,
            "{diagnostics:?}"
        );
        let raw = originals(&drawing)[0];
        assert_eq!(field(raw, "measure/present"), &FieldValue::Integer(1));
        assert_eq!(
            field(raw, "measure/type"),
            &FieldValue::Text(kind.to_owned())
        );
        assert!(diagnostics.iter().any(|diagnostic| diagnostic.code().name()
            == "pdf.measure.invalid"
            && diagnostic.param("reason") == Some(&zumen_base::Value::Text(reason.to_owned()))));
        unchanged_page(&input, &drawing);
    }
    let input = fixture::make(&[Viewport::new("null").without_measure()]);
    let (drawing, diagnostics) = common::lift(&input.bytes);
    assert!(originals(&drawing).is_empty() && calibrations(&drawing).is_empty());
    assert_eq!(count(&diagnostics, "pdf.measure.invalid"), 0);
    assert_eq!(count(&diagnostics, "pdf.measure.unsupported"), 0);
}

#[test]
fn reads_long_decimal_factors_and_origins_with_correctly_rounded_binary64_values() {
    let tiny = 1e-307_f64;
    let twenty = fixture::TWENTY_DIGITS.parse::<f64>().unwrap();
    let precise = fixture::PRECISE_SMALL.parse::<f64>().unwrap();
    assert_eq!(precise.to_bits(), 0.00139_f64.to_bits());
    for (name, input, x, y, cyx, numbers) in [
        (
            "direct",
            fixture::direct_tiny_factor(),
            1.0,
            tiny,
            1.0,
            vec![("y/0/c", tiny), ("o/0", tiny), ("o/1", twenty)],
        ),
        (
            "referenced",
            fixture::referenced_tiny_factor(),
            tiny,
            precise,
            twenty,
            vec![
                ("x/0/c", tiny),
                ("y/0/c", precise),
                ("cyx", twenty),
                ("o/0", tiny),
                ("o/1", precise),
            ],
        ),
        (
            "twenty-digits",
            fixture::decimal_twenty_digits(),
            precise,
            twenty,
            precise,
            vec![
                ("x/0/c", precise),
                ("y/0/c", twenty),
                ("cyx", precise),
                ("o/0", twenty),
                ("o/1", -precise),
            ],
        ),
    ] {
        let (drawing, diagnostics) = common::lift(&input.bytes);
        let values = calibrations(&drawing);
        assert_eq!(values.len(), 1, "{name}: {diagnostics:?}");
        assert_eq!(count(&diagnostics, "pdf.measure.invalid"), 0);
        assert_eq!(count(&diagnostics, "pdf.measure.unsupported"), 0);
        assert_eq!(count(&diagnostics, "pdf.measure-distance.equivalent"), 1);
        let raw = originals(&drawing)[0];
        for (key, expected) in numbers {
            let FieldValue::Real(found) = field(raw, key) else {
                panic!("{name}: {key} が実数でない")
            };
            assert_eq!(found.to_bits(), expected.to_bits(), "{name}: {key}");
        }
        if name == "referenced" {
            for (key, object) in [
                ("x/0/ref/object", 6),
                ("x/0/c/ref/object", 5),
                ("y/0/ref/object", 7),
                ("o/ref/object", 8),
                ("o/0/ref/object", 5),
            ] {
                assert_eq!(field(raw, key), &FieldValue::Integer(object), "{key}");
            }
        }
        // 原字句を標準の binary64 読み取りで決めてから、紙 mm から PDF pt への倍率を掛ける。
        let pt_to_mm = 25.4 / 72.0;
        assert_eq!(
            values[0].matrix[0].to_bits(),
            (x / pt_to_mm).to_bits(),
            "{name}: X"
        );
        assert_eq!(
            values[0].matrix[3].to_bits(),
            ((y * cyx) / pt_to_mm).to_bits(),
            "{name}: Y"
        );
        assert_eq!(values[0].matrix[1], 0.0);
        assert_eq!(values[0].matrix[2], 0.0);
        unchanged_page(&input, &drawing);
    }
}

#[test]
fn keeps_invalid_values_and_geo_without_creating_a_calibration() {
    for measure in [
        fixture::rl("0", None, ""),
        fixture::rl("1", Some("1"), "/CYX 0"),
        "<< /Subtype /RL /R (bad) /X null /D [] /A [] >>".to_owned(),
        "<< /Subtype /RL /R (bad) /D [] /A [] >>".to_owned(),
        "<< /Subtype /RL /R (bad) /X << /U (m) /C 1 >> /D [] /A [] >>".to_owned(),
        "<< /Subtype /RL /R (bad) /X [null << /U (m) /C 1 >>] /D [] /A [] >>".to_owned(),
        "<< /Subtype /RL /X [<< /U /meters /C 1 >>] /D [] /A [] >>".to_owned(),
    ] {
        let input = fixture::make(&[Viewport::new(&measure)]);
        let (drawing, diagnostics) = common::lift(&input.bytes);
        assert!(calibrations(&drawing).is_empty(), "{diagnostics:?}");
        assert_eq!(originals(&drawing).len(), 1);
        assert_eq!(count(&diagnostics, "pdf.measure.unsupported"), 1);
        assert!(count(&diagnostics, "pdf.measure.invalid") > 0);
        unchanged_page(&input, &drawing);
    }
    let geo = "<< /Type /Measure /Subtype /GEO /GPTS [0 0 1 1] /LPTS [0 0 1 1] /GCS << /Type /GEOGCS >> >>";
    let input = fixture::make(&[Viewport::new(geo)]);
    let (drawing, diagnostics) = common::lift(&input.bytes);
    assert!(calibrations(&drawing).is_empty());
    assert_eq!(count(&diagnostics, "pdf.measure.unsupported"), 1);
    assert_eq!(
        count(&diagnostics, "pdf.measure.invalid"),
        0,
        "{diagnostics:?}"
    );
    let FieldValue::Bytes(raw) = field(originals(&drawing)[0], "measure/raw") else {
        panic!("原値でない")
    };
    assert_eq!(raw, geo.as_bytes());
}

#[test]
fn keeps_viewport_ordinals_and_requires_explicit_selection_for_overlaps() {
    let input = fixture::make(&[
        Viewport::new(&fixture::rl("1", None, "")),
        Viewport::new(&fixture::rl("2", None, "")),
    ]);
    let (drawing, diagnostics) = common::lift(&input.bytes);
    let values = calibrations(&drawing);
    assert_eq!(
        values
            .iter()
            .map(|value| value.id.as_str())
            .collect::<Vec<_>>(),
        ["sheet/0/vp/0", "sheet/0/vp/1"]
    );
    assert_eq!(values[0].bounds, values[1].bounds);
    near(
        values[0].measure(paper(0.0, 0.0), paper(3.0, 4.0)).unwrap(),
        5.0,
    );
    near(
        values[1].measure(paper(0.0, 0.0), paper(3.0, 4.0)).unwrap(),
        10.0,
    );
    assert_eq!(count(&diagnostics, "pdf.measure-distance.equivalent"), 2);
    unchanged_page(&input, &drawing);
    let input = fixture::make(&[
        Viewport::new("null").without_measure(),
        Viewport::new(&fixture::rl("1", None, "")),
    ]);
    let (drawing, diagnostics) = common::lift(&input.bytes);
    let values = calibrations(&drawing);
    assert_eq!(values.len(), 1, "{diagnostics:?}");
    assert_eq!(values[0].id, "sheet/0/vp/1");
    assert_eq!(originals(&drawing).len(), 1);
    assert_eq!(count(&diagnostics, "pdf.measure.invalid"), 0);
    // 既存の VP の型付き反復と同じく、辞書でない項目で終了する。
    for value in [
        "17".to_owned(),
        format!(
            "[null << /BBox [0 0 144 144] /Measure {} >>]",
            fixture::rl("1", None, "")
        ),
        format!(
            "[99 0 R << /BBox [0 0 144 144] /Measure {} >>]",
            fixture::rl("1", None, "")
        ),
    ] {
        let input = fixture::with_vp_value(&value);
        let (drawing, diagnostics) = common::lift(&input.bytes);
        assert!(originals(&drawing).is_empty() && calibrations(&drawing).is_empty());
        assert_eq!(count(&diagnostics, "pdf.measure.invalid"), 0);
        assert_eq!(count(&diagnostics, "pdf.measure.unsupported"), 0);
        unchanged_page(&input, &drawing);
    }
}

#[test]
fn validates_regions_and_calibrated_measurement_errors() {
    for viewport in [
        Viewport::new(&fixture::rl("1", None, "")).extra("/Type /Other"),
        Viewport::new(&fixture::rl("1", None, "")).without_bbox(),
        Viewport::new(&fixture::rl("1", None, "")).bbox("[0 0 144]"),
        Viewport::new(&fixture::rl("1", None, "")).bbox("[0 0 0 144]"),
    ] {
        let input = fixture::make(&[viewport]);
        let (drawing, diagnostics) = common::lift(&input.bytes);
        assert!(calibrations(&drawing).is_empty());
        assert_eq!(count(&diagnostics, "pdf.measure.unsupported"), 1);
        unchanged_page(&input, &drawing);
    }
    let input = fixture::make(&[Viewport::new(&fixture::rl("1", None, "")).bbox("[144 144 0 0]")]);
    let (drawing, diagnostics) = common::lift(&input.bytes);
    let values = calibrations(&drawing);
    assert_eq!(values.len(), 1, "{diagnostics:?}");
    assert_eq!(
        values[0].measure(paper(0.0, 0.0), paper(200.0, 0.0)),
        Err(MeasurementError::OutsideBounds)
    );
    assert_eq!(
        values[0].measure(
            paper(0.0, 0.0),
            PaperPoint {
                sheet: SheetId(1),
                ..paper(1.0, 1.0)
            }
        ),
        Err(MeasurementError::SheetMismatch)
    );
    assert!(
        values[0]
            .measure(paper(0.0, 0.0), paper(144.0, 144.0))
            .is_ok()
    );
    let input = fixture::overflow();
    let (drawing, diagnostics) = common::lift(&input.bytes);
    let values = calibrations(&drawing);
    assert_eq!(values.len(), 1, "{diagnostics:?}");
    assert_eq!(
        values[0].measure(paper(0.0, 0.0), paper(100.0, 1.0)),
        Err(MeasurementError::Overflow)
    );
    assert!(
        matches!(field(originals(&drawing)[0], "x/0/c"), FieldValue::Real(value) if *value == 1e307)
    );
}

#[test]
fn resolves_active_numeric_references_across_objstm_and_duplicate_definitions() {
    let measure = "<< /Type /Measure /Subtype /RL /X [<< /U (m) /C 5 0 R >>] /D [<< /U (m) /C 1 >>] /A [<< /U (m2) /C 1 >>] >>";
    // 1. 別の ObjStm を含む文書でも、通常の数値オブジェクトの元字句を保つ。
    let alongside_objstm = fixture::with_objects(
        fixture::PAGE,
        "",
        &[
            fixture::PRECISE_SMALL.as_bytes().to_vec(),
            common::stream("/Type /ObjStm /N 1 /First 5", b"99 0 1"),
        ],
        &[Viewport::new(measure)],
    );
    let (drawing, diagnostics) = common::lift(&alongside_objstm.bytes);
    assert_eq!(calibrations(&drawing).len(), 1, "{diagnostics:?}");
    let raw = originals(&drawing)[0];
    assert_eq!(
        field(raw, "x/0/c/resolved_raw"),
        &FieldValue::Bytes(fixture::PRECISE_SMALL.as_bytes().to_vec())
    );
    assert_eq!(
        field(raw, "x/0/c"),
        &FieldValue::Real(fixture::PRECISE_SMALL.parse::<f64>().unwrap())
    );

    // 2. 重複定義を含む文書では、アクティブな相互参照表が指す定義（ここでは元の xref が指す 1 個目の定義）だけを採用する。
    let mut duplicate = fixture::with_objects(
        fixture::PAGE,
        "",
        &[b"1.25".to_vec()],
        &[Viewport::new(measure)],
    );
    duplicate
        .bytes
        .extend_from_slice(b"\n5 0 obj\n9.5\nendobj\n");
    let (dup_drawing, dup_diag) = common::lift(&duplicate.bytes);
    assert_eq!(calibrations(&dup_drawing).len(), 1, "{dup_diag:?}");
    let dup_raw = originals(&dup_drawing)[0];
    assert_eq!(
        field(dup_raw, "x/0/c/resolved_raw"),
        &FieldValue::Bytes(b"1.25".to_vec())
    );
    assert_eq!(field(dup_raw, "x/0/c"), &FieldValue::Real(1.25));

    // 3. 圧縮された ObjStm の中にある数値参照（type 2 の XRef 項目）も、丸め前の十進字句を取り出して校正を作る。
    let mut stm_pdf = b"%PDF-1.7\n".to_vec();
    let mut entries = std::collections::BTreeMap::new();
    let member_body = format!("5 0 {}\n", fixture::PRECISE_SMALL);
    let obj_stm = common::stream("/Type /ObjStm /N 1 /First 4", member_body.into_bytes());
    let vp_body = format!("<< /Type /Viewport /BBox [0 0 144 144] /Measure {measure} >>");
    for (num, body) in [
        (1u32, b"<< /Type /Catalog /Pages 2 0 R >>".to_vec()),
        (2, b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_vec()),
        (
            3,
            b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 200 200] /VP [4 0 R] >>".to_vec(),
        ),
        (4, vp_body.into_bytes()),
        (6, obj_stm),
    ] {
        entries.insert(num, (1u8, stm_pdf.len() as u32, 0u16));
        stm_pdf.extend(format!("{num} 0 obj\n").as_bytes());
        stm_pdf.extend(body);
        stm_pdf.extend(b"\nendobj\n");
    }
    entries.insert(5, (2, 6, 0));
    let xref_pos = stm_pdf.len();
    entries.insert(99, (1, xref_pos as u32, 0));
    let mut index = String::new();
    let mut table = Vec::new();
    for (num, (kind, f1, f2)) in entries {
        index.push_str(&format!("{num} 1 "));
        table.push(kind);
        table.extend(f1.to_be_bytes());
        table.extend(f2.to_be_bytes());
    }
    stm_pdf.extend(b"99 0 obj\n");
    stm_pdf.extend(common::stream(
        &format!("/Type /XRef /Size 100 /W [1 4 2] /Index [{index}] /Root 1 0 R"),
        table,
    ));
    stm_pdf.extend(format!("\nendobj\nstartxref\n{xref_pos}\n%%EOF\n").as_bytes());

    let (stm_drawing, stm_diag) = common::lift(&stm_pdf);
    assert_eq!(calibrations(&stm_drawing).len(), 1, "{stm_diag:?}");
    let stm_raw = originals(&stm_drawing)[0];
    assert_eq!(
        field(stm_raw, "x/0/c/resolved_raw"),
        &FieldValue::Bytes(fixture::PRECISE_SMALL.as_bytes().to_vec())
    );
    assert_eq!(
        field(stm_raw, "x/0/c"),
        &FieldValue::Real(fixture::PRECISE_SMALL.parse::<f64>().unwrap())
    );

    // 4. 後続に余分な字句がある壊れた数値オブジェクトは引き続き拒否し、raw-unavailable を返す。
    let malformed = fixture::with_objects(
        fixture::PAGE,
        "",
        &[b"1.25 extra".to_vec()],
        &[Viewport::new(measure)],
    );
    let (bad_drawing, bad_diag) = common::lift(&malformed.bytes);
    assert!(calibrations(&bad_drawing).is_empty(), "{bad_diag:?}");
    let bad_raw = originals(&bad_drawing)[0];
    assert_eq!(
        field(bad_raw, "x/0/c/type"),
        &FieldValue::Text("number".into())
    );
    assert!(bad_raw.fields.iter().all(|field| field.key != "x/0/c"));
    assert!(
        bad_diag.iter().any(|diagnostic| {
            diagnostic.code().name() == "pdf.measure.invalid"
                && diagnostic.param("reason")
                    == Some(&zumen_base::Value::Text("raw-unavailable".into()))
        }),
        "{bad_diag:?}"
    );
}

#[test]
fn stops_at_work_and_memory_boundaries_and_retries_without_partial_results() {
    let mut stm_measure = b"%PDF-1.7\n".to_vec();
    {
        let mut entries = std::collections::BTreeMap::new();
        let member = format!("5 0 {}\n", fixture::PRECISE_SMALL);
        let obj_stm = common::stream("/Type /ObjStm /N 1 /First 4", member.into_bytes());
        let vp_body = "<< /Type /Viewport /BBox [0 0 144 144] /Measure << /Type /Measure /Subtype /RL /X [<< /U (m) /C 5 0 R >>] /D [<< /U (m) /C 1 >>] /A [<< /U (m2) /C 1 >>] >> >>";
        for (num, body) in [
            (1u32, b"<< /Type /Catalog /Pages 2 0 R >>".to_vec()),
            (2, b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_vec()),
            (
                3,
                b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 200 200] /VP [4 0 R] >>".to_vec(),
            ),
            (4, vp_body.as_bytes().to_vec()),
            (6, obj_stm),
        ] {
            entries.insert(num, (1u8, stm_measure.len() as u32, 0u16));
            stm_measure.extend(format!("{num} 0 obj\n").as_bytes());
            stm_measure.extend(body);
            stm_measure.extend(b"\nendobj\n");
        }
        entries.insert(5, (2, 6, 0));
        let xref_pos = stm_measure.len();
        entries.insert(99, (1, xref_pos as u32, 0));
        let mut index = String::new();
        let mut table = Vec::new();
        for (num, (kind, f1, f2)) in entries {
            index.push_str(&format!("{num} 1 "));
            table.push(kind);
            table.extend(f1.to_be_bytes());
            table.extend(f2.to_be_bytes());
        }
        stm_measure.extend(b"99 0 obj\n");
        stm_measure.extend(common::stream(
            &format!("/Type /XRef /Size 100 /W [1 4 2] /Index [{index}] /Root 1 0 R"),
            table,
        ));
        stm_measure.extend(format!("\nendobj\nstartxref\n{xref_pos}\n%%EOF\n").as_bytes());
    }
    for bytes in [
        fixture::make(&[Viewport::new(&fixture::full_formats())]).bytes,
        fixture::with_objects(
            fixture::PAGE,
            "",
            &[b"<< /U (m) /C 1 >>".to_vec()],
            &[Viewport::new(
                "<< /Type /Measure /Subtype /RL /X [5 0 R] /D [] /A [] >>",
            )],
        )
        .bytes,
        stm_measure,
    ] {
        let file = testkit::read::<Pdf>(&bytes).0.unwrap();
        let cancel = CancelToken::new();
        let mut budget = Budget::default();
        let mut diagnostics = Diagnostics::new();
        let expected = file
            .lift(&mut Cx::new(&mut budget, &mut diagnostics, &cancel))
            .unwrap();
        let expected_diagnostics = diagnostics.into_vec();
        assert_eq!(calibrations(&expected).len(), 1);
        for kind in [Kind::Work, Kind::DecodedBytes] {
            let used = budget.used(kind);
            for limit in [used - 1, used] {
                let mut budget = Budget::new(Limits::default().with(kind, Limit::Finite(limit)));
                let mut diagnostics = Diagnostics::new();
                let result = file.lift(&mut Cx::new(&mut budget, &mut diagnostics, &cancel));
                if limit == used {
                    assert_eq!(result.unwrap(), expected);
                    assert_eq!(diagnostics.into_vec(), expected_diagnostics);
                } else {
                    assert!(
                        matches!(result, Err(Error::Stopped(_))),
                        "{kind:?}: {result:?}"
                    );
                }
            }
        }
        cancel.cancel();
        let mut budget = Budget::default();
        let mut diagnostics = Diagnostics::new();
        assert!(matches!(
            file.lift(&mut Cx::new(&mut budget, &mut diagnostics, &cancel)),
            Err(Error::Stopped(_))
        ));
        assert_eq!(common::lift(&bytes), (expected, expected_diagnostics));
    }
}
