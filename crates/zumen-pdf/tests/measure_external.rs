//! #520: pdf-lib の型付き原値と、PDF4QT の未改変の距離算術を別々に照らす。
//! PDF4QT 全体のパーサー・Qt の文字整形や丸めを比べたとは扱わない。
#![cfg(test)]
#![cfg(feature = "drawing")]

use std::collections::BTreeSet;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use kurbo::{Affine, Point};
use zumen_base::{Budget, CancelToken, Cx, Diagnostics};
use zumen_drawing::{Calibration, Drawing2D, Extension, FieldValue, PaperPoint};
use zumen_json::{Value, Writer};

mod common;
#[path = "common/measure.rs"]
mod fixture;

// 指定した外部照合だけの起動条件。コアの振る舞いには使わない。
#[allow(clippy::disallowed_methods, clippy::print_stderr)]
fn oracle(script: &str) -> Option<PathBuf> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../xtask/verify/pdfmeasure")
        .join(script);
    let result = Command::new("node").arg(&path).arg("--check").output();
    let missing = match &result {
        Ok(output) if output.status.success() => return Some(path),
        Ok(output) => output.status.code() == Some(77),
        Err(error) => error.kind() == std::io::ErrorKind::NotFound,
    };
    assert!(missing, "計測の外部照合を起動できない: {result:?}");
    assert!(
        std::env::var_os("ZUMEN_PDFMEASURE_REQUIRED").is_none(),
        "計測の外部照合が必要だが、準備されていない"
    );
    eprintln!("計測の外部照合を飛ばす（xtask/verify/pdfmeasure/README.md）");
    None
}

fn external(path: &Path, input: &[u8]) -> Value {
    let mut child = Command::new("node")
        .arg(path)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(input).unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "計測の外部照合が失敗: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    zumen_json::parse(&output.stdout).expect("外部照合の出力が JSON でない")
}

fn json<'a>(value: &'a Value, key: &str) -> &'a Value {
    let Value::Object(value) = value else {
        panic!("JSON object でない: {value:?}");
    };
    value
        .get(key)
        .unwrap_or_else(|| panic!("JSON のキーがない: {key}"))
}

fn text(value: &Value) -> &str {
    let Value::String(value) = value else {
        panic!("JSON string でない: {value:?}");
    };
    value
}

fn number(value: &Value) -> f64 {
    let Value::Number(value) = value else {
        panic!("JSON number でない: {value:?}");
    };
    value.to_f64().unwrap()
}

fn array(value: &Value) -> &[Value] {
    let Value::Array(value) = value else {
        panic!("JSON array でない: {value:?}");
    };
    value
}

fn kind(value: &Value) -> &str {
    text(json(value, "type"))
}

fn resolved(mut value: &Value) -> &Value {
    while kind(value) == "ref" {
        value = json(value, "value");
    }
    value
}

fn member<'a>(value: &'a Value, key: &str) -> &'a Value {
    json(json(resolved(value), "value"), key)
}

fn typed_numbers(value: &Value) -> Vec<f64> {
    array(json(resolved(value), "value"))
        .iter()
        .map(|value| number(json(resolved(value), "value")))
        .collect()
}

fn field<'a>(extension: &'a Extension, key: &str) -> Option<&'a FieldValue> {
    extension
        .fields
        .iter()
        .find(|field| field.key == key)
        .map(|field| &field.value)
}

fn hex(value: &str) -> Vec<u8> {
    assert!(value.len().is_multiple_of(2));
    value
        .as_bytes()
        .as_chunks::<2>()
        .0
        .iter()
        .map(|bytes| u8::from_str_radix(std::str::from_utf8(bytes).unwrap(), 16).unwrap())
        .collect()
}

fn compare_value(case: &str, extension: &Extension, prefix: &str, raw: &Value) {
    let found = |key: &str| field(extension, key);
    assert_eq!(
        found(&format!("{prefix}/present")),
        Some(&FieldValue::Integer(i64::from(kind(raw) != "absent"))),
        "{case}: {prefix}"
    );
    if kind(raw) == "ref" {
        for key in ["object", "generation"] {
            assert_eq!(
                found(&format!("{prefix}/ref/{key}")),
                Some(&FieldValue::Integer(number(json(raw, key)) as i64)),
                "{case}: {prefix}/{key}"
            );
        }
    } else {
        assert!(found(&format!("{prefix}/ref/object")).is_none());
    }
    let raw = resolved(raw);
    let expected = match kind(raw) {
        "absent" => "missing",
        "dict" => "dictionary",
        other => other,
    };
    assert_eq!(
        found(&format!("{prefix}/type")),
        Some(&FieldValue::Text(expected.into())),
        "{case}: {prefix}"
    );
    match kind(raw) {
        "number" => {
            let actual = match found(prefix).unwrap() {
                FieldValue::Real(value) => *value,
                FieldValue::Integer(value) => *value as f64,
                other => panic!("{case}: {prefix}: {other:?}"),
            };
            assert_eq!(actual, number(json(raw, "value")), "{case}: {prefix}");
        }
        "boolean" => {
            let Value::Bool(value) = json(raw, "value") else {
                panic!("{case}: {prefix}: JSON boolean でない: {raw:?}");
            };
            assert_eq!(found(prefix), Some(&FieldValue::Integer(i64::from(*value))));
        }
        "name" | "string" => {
            assert_eq!(
                found(prefix),
                Some(&FieldValue::Text(text(json(raw, "value")).into())),
                "{case}: {prefix}"
            );
            assert_eq!(
                found(&format!("{prefix}/bytes")),
                Some(&FieldValue::Bytes(hex(text(json(raw, "bytes"))))),
                "{case}: {prefix}/bytes"
            );
        }
        _ => assert!(found(prefix).is_none(), "{case}: {prefix}"),
    }
}

fn compare_numbers(case: &str, extension: &Extension, prefix: &str, raw: &Value) {
    compare_value(case, extension, prefix, raw);
    if kind(resolved(raw)) == "array" {
        let values = array(json(resolved(raw), "value"));
        assert_eq!(
            field(extension, &format!("{prefix}/count")),
            Some(&FieldValue::Integer(values.len() as i64))
        );
        for (index, value) in values.iter().enumerate() {
            compare_value(case, extension, &format!("{prefix}/{index}"), value);
        }
    }
}

fn compare_format(case: &str, extension: &Extension, prefix: &str, raw: &Value) {
    for (key, suffix) in [
        ("Type", "type_name"),
        ("U", "u"),
        ("C", "c"),
        ("F", "f"),
        ("D", "d"),
        ("FD", "fd"),
        ("RT", "rt"),
        ("RD", "rd"),
        ("PS", "ps"),
        ("SS", "ss"),
        ("O", "o"),
    ] {
        compare_value(
            case,
            extension,
            &format!("{prefix}/{suffix}"),
            member(raw, key),
        );
    }
}

fn compare_formats(case: &str, extension: &Extension, prefix: &str, raw: &Value) {
    compare_value(case, extension, prefix, raw);
    let raw = resolved(raw);
    let items = match kind(raw) {
        "array" => array(json(raw, "value")),
        "dict" => std::slice::from_ref(raw),
        _ => return,
    };
    assert_eq!(
        field(extension, &format!("{prefix}/count")),
        Some(&FieldValue::Integer(items.len() as i64))
    );
    for (index, value) in items.iter().enumerate() {
        let prefix = format!("{prefix}/{index}");
        if kind(raw) == "array" {
            compare_value(case, extension, &prefix, value);
        }
        if kind(resolved(value)) == "dict" {
            compare_format(case, extension, &prefix, value);
        }
    }
}

fn cases() -> Vec<fixture::Case> {
    let mut cases = fixture::cases();
    cases.push(fixture::Case {
        name: "external-precision-and-display-r",
        fixture: fixture::make(&[fixture::Viewport::new(
            "<< /Subtype /RL /R (1 = 999999, display only) /X [<< /U (m) /C 0.1234567890123456 /F /D /D 100 >>] /D [<< /U (cm) /C 100 >>] /A [] >>",
        )]),
    });
    cases.push(fixture::Case {
        name: "external-null-fields-and-format-items",
        fixture: fixture::make(&[fixture::Viewport::new(
            "<< /Subtype /RL /R null /X [<< /U (m) /C 1 /F null /D null /FD null /RT null /RD null /PS null /SS null /O null >> null 99 0 R] /Y null /CYX null /D [] /A [] /O [0 null] >>",
        )]),
    });
    cases.push(fixture::Case {
        name: "external-referenced-scalars",
        fixture: fixture::with_objects(
            fixture::PAGE,
            "",
            &[
                b"(m)".to_vec(),
                b"0.1234567890123456".to_vec(),
                b"/D".to_vec(),
            ],
            &[fixture::Viewport::new(
                "<< /Subtype /RL /X [<< /U 5 0 R /C 6 0 R /F 7 0 R >>] /D [] /A [] >>",
            )],
        ),
    });
    cases
}

#[test]
fn compares_typed_measure_originals_with_pdf_lib() {
    let Some(path) = oracle("read.mjs") else {
        return;
    };
    for case in cases() {
        let raw = external(&path, &case.fixture.bytes);
        assert_eq!(text(json(&raw, "version")), "1.17.1");
        let page = &array(json(&raw, "pages"))[0];
        let vp = resolved(json(page, "VP"));
        let items = if kind(vp) == "array" {
            array(json(vp, "value"))
        } else {
            &[]
        };
        let (drawing, _) = common::lift(&case.fixture.bytes);
        let originals: Vec<_> = drawing
            .extensions
            .iter()
            .filter(|item| item.namespace == "pdf.measure")
            .collect();
        let mut count = 0;
        for (ordinal, viewport) in items.iter().enumerate() {
            if kind(resolved(viewport)) != "dict" {
                break;
            }
            if kind(member(viewport, "Measure")) == "absent" {
                continue;
            }
            let extension = originals[count];
            count += 1;
            let mut keys = BTreeSet::new();
            assert!(
                extension
                    .fields
                    .iter()
                    .all(|field| keys.insert(field.key.as_ref())),
                "{}: 原値のキーが重なっている",
                case.name
            );
            assert_eq!(
                extension.name,
                format!("sheet/0/vp/{ordinal}"),
                "{}",
                case.name
            );
            for (key, prefix) in [
                ("Type", "viewport/type_name"),
                ("Name", "viewport/name"),
                ("Measure", "measure"),
            ] {
                compare_value(case.name, extension, prefix, member(viewport, key));
            }
            compare_numbers(
                case.name,
                extension,
                "viewport/bbox",
                member(viewport, "BBox"),
            );
            let measure = member(viewport, "Measure");
            if kind(resolved(measure)) == "dict" {
                for (key, prefix) in [
                    ("Type", "measure/type_name"),
                    ("Subtype", "measure/subtype"),
                    ("R", "r"),
                    ("CYX", "cyx"),
                ] {
                    compare_value(case.name, extension, prefix, member(measure, key));
                }
                for (key, prefix) in [
                    ("X", "x"),
                    ("Y", "y"),
                    ("D", "d"),
                    ("A", "a"),
                    ("T", "t"),
                    ("S", "s"),
                ] {
                    compare_formats(case.name, extension, prefix, member(measure, key));
                }
                compare_numbers(case.name, extension, "o", member(measure, "O"));
            }
        }
        assert_eq!(count, originals.len(), "{}", case.name);
    }
}

fn calibrations(drawing: &Drawing2D) -> Vec<Calibration> {
    let mut budget = Budget::default();
    let mut diagnostics = Diagnostics::new();
    let cancel = CancelToken::new();
    let result = drawing
        .calibrations(&mut Cx::new(&mut budget, &mut diagnostics, &cancel))
        .unwrap();
    assert!(diagnostics.is_empty());
    result
}

// 独立に読んだページの直接値で、比較する 2 点だけを用紙へ写す。距離校正式はここに書かない。
fn paper_transform(page: &Value) -> Affine {
    let media = typed_numbers(json(page, "MediaBox"));
    let crop = json(page, "CropBox");
    let crop = if kind(resolved(crop)) == "absent" {
        media.clone()
    } else {
        typed_numbers(crop)
    };
    let x = media[0].max(crop[0]);
    let y = media[1].max(crop[1]);
    let width = media[2].min(crop[2]) - x;
    let height = media[3].min(crop[3]) - y;
    let unit = json(page, "UserUnit");
    let unit = if kind(resolved(unit)) == "number" {
        number(json(resolved(unit), "value"))
    } else {
        1.0
    };
    let rotate = json(page, "Rotate");
    let rotate = if kind(resolved(rotate)) == "number" {
        number(json(resolved(rotate), "value"))
    } else {
        0.0
    };
    let rotation = match rotate {
        0.0 => Affine::IDENTITY,
        90.0 => Affine::new([0.0, -1.0, 1.0, 0.0, 0.0, width]),
        180.0 => Affine::new([-1.0, 0.0, 0.0, -1.0, width, height]),
        270.0 => Affine::new([0.0, 1.0, -1.0, 0.0, height, 0.0]),
        _ => panic!("比較用入力の回転でない"),
    };
    Affine::scale((25.4 / 72.0) * unit) * rotation * Affine::translate((-x, -y))
}

fn coefficients(measure: &Value, key: &str) -> Vec<f64> {
    let raw = resolved(member(measure, key));
    if kind(raw) == "absent" {
        return Vec::new();
    }
    array(json(raw, "value"))
        .iter()
        .map(|format| {
            let factor = resolved(member(format, "C"));
            // これは算術関数への入力準備。PDF4QT の parser と同じ既定値をコアへ要求しない。
            if kind(factor) == "absent" {
                1.0
            } else {
                number(json(factor, "value"))
            }
        })
        .collect()
}

fn calculation(delta: [f64; 2], x: &[f64], y: &[f64], distance: &[f64], cyx: f64) -> Vec<u8> {
    let mut writer = Writer::new();
    writer.begin_object();
    writer.key("delta");
    writer.array(&delta, |writer, value| writer.float(*value));
    for (key, values) in [("x", x), ("y", y), ("distance", distance)] {
        writer.key(key);
        writer.array(values, |writer, value| writer.float(*value));
    }
    writer.key("cyx");
    writer.float(cyx);
    writer.end_object();
    writer.into_bytes()
}

#[test]
fn compares_calibrated_displacements_with_original_pdf4qt_arithmetic() {
    let (Some(raw_oracle), Some(arithmetic)) = (oracle("read.mjs"), oracle("run.mjs")) else {
        return;
    };
    let mut compared = 0;
    for case in cases() {
        let raw = external(&raw_oracle, &case.fixture.bytes);
        let page = &array(json(&raw, "pages"))[0];
        let (drawing, _) = common::lift(&case.fixture.bytes);
        let calibrations = calibrations(&drawing);
        let expected_count = match case.name {
            "zero-c"
            | "zero-cyx"
            | "null-measure"
            | "unresolved-measure"
            | "wrong-measure-type"
            | "geo"
            | "wrong-bbox-count"
            | "vp-value-type"
            | "vp-null-before-valid"
            | "no-measure"
            | "external-null-fields-and-format-items" => 0,
            "overlapping-viewports" => 2,
            _ => 1,
        };
        assert_eq!(calibrations.len(), expected_count, "{}", case.name);
        for calibration in calibrations {
            let ordinal: usize = calibration.id.rsplit('/').next().unwrap().parse().unwrap();
            let viewport = &array(json(resolved(json(page, "VP")), "value"))[ordinal];
            let measure = member(viewport, "Measure");
            let x = coefficients(measure, "X");
            let y = coefficients(measure, "Y");
            let distance = coefficients(measure, "D");
            let first_x = &array(json(resolved(member(measure, "X")), "value"))[0];
            assert_eq!(
                calibration.unit_label,
                text(json(resolved(member(first_x, "U")), "value"))
            );
            let name = resolved(member(viewport, "Name"));
            let name = (kind(name) == "string").then(|| text(json(name, "value")));
            assert_eq!(calibration.name.as_deref(), name);
            let cyx = resolved(member(measure, "CYX"));
            // Y がなければ CYX は使わない。原値の CYX は上の別テストでそのまま照らす。
            let cyx = if y.is_empty() || kind(cyx) == "absent" {
                1.0
            } else {
                number(json(cyx, "value"))
            };
            let bbox = typed_numbers(member(viewport, "BBox"));
            let [min_x, max_x] = [bbox[0].min(bbox[2]), bbox[0].max(bbox[2])];
            let [min_y, max_y] = [bbox[1].min(bbox[3]), bbox[1].max(bbox[3])];
            let start = Point::new(min_x * 0.5 + max_x * 0.5, min_y * 0.5 + max_y * 0.5);
            let dx = (max_x - min_x).min(2.0) / 8.0;
            let dy = (max_y - min_y).min(2.0) / 8.0;
            let transform = paper_transform(page);
            for delta in [[dx, 0.0], [0.0, dy], [dx, dy], [-dx, dy]] {
                let output = external(&arithmetic, &calculation(delta, &x, &y, &distance, cyx));
                let sample = &array(json(&output, "samples"))[0];
                let formatted = number(json(sample, "value"));
                let expected = if text(json(sample, "array")) == "D" {
                    formatted / distance[0]
                } else {
                    formatted
                };
                let from = transform * start;
                let to = transform * (start + kurbo::Vec2::new(delta[0], delta[1]));
                let point = |point: Point| PaperPoint {
                    sheet: calibration.sheet,
                    x: point.x,
                    y: point.y,
                };
                let actual = calibration.measure(point(from), point(to)).unwrap();
                let tolerance = expected.abs() * 1e-10 + f64::from_bits(4);
                assert!(
                    (actual - expected).abs() <= tolerance,
                    "{}: {delta:?}: {actual} != {expected}",
                    case.name
                );
                compared += 1;
            }
        }
    }
    assert!(compared >= 60, "有効な校正の比較が足りない: {compared}");
}
