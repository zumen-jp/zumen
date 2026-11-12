fn estimate_array<'a>(response: &'a Value, pointer: &str) -> &'a [Value] {
    let Some(Value::Array(items)) = response.pointer(pointer) else {
        panic!("{pointer}: 配列がない");
    };
    items
}

fn estimate_u64(response: &Value, pointer: &str) -> u64 {
    number(response, pointer).parse().unwrap()
}

fn estimate_source(session: &mut Session, bytes: &[u8], name: &str) {
    session.push(name.as_bytes(), bytes).unwrap();
    let (status, response) = call(
        session,
        &format!(r#"{{"v":1,"op":"open","input":"{name}"}}"#),
    );
    assert_eq!(status, Status::Ok, "{response:?}");
}

fn estimate_matches_conversion(bytes: &[u8], name: &str) -> Value {
    estimate_matches_conversion_with_places(bytes, name, None)
}

fn estimate_matches_conversion_with_places(
    bytes: &[u8],
    name: &str,
    max_places: Option<u64>,
) -> Value {
    let places = max_places.map_or(String::new(), |limit| format!(",\"max_places\":{limit}"));
    let mut session = Session::new(b"").unwrap();
    estimate_source(&mut session, bytes, name);
    let before = seen(&mut session, name);
    let (_, drawing) = call(
        &mut session,
        &format!(r#"{{"v":1,"op":"drawing","doc":"{name}"}}"#),
    );
    let source_hash = text(&drawing, "/result/hash");
    let mut estimates: Vec<Value> = Vec::new();
    for limit in [0, 1, 1000] {
        let (status, response) = call(
            &mut session,
            &format!(
                r#"{{"v":1,"op":"convert_estimate","doc":"{name}","format":"dxf","to":"out.dxf","limit":{limit}{places}}}"#
            ),
        );
        assert_eq!(status, Status::Ok, "{response:?}");
        assert!(estimate_array(&response, "/blobs").is_empty());
        assert!(session.take(0).is_none());
        let details = estimate_array(&response, "/result/estimate/details");
        assert!(details.len() <= limit as usize);
        let total = estimate_u64(&response, "/result/estimate/total");
        assert_eq!(
            response.pointer("/result/estimate/truncated"),
            Some(&Value::Bool(total > details.len() as u64))
        );
        let counts = estimate_array(&response, "/result/estimate/counts");
        assert_eq!(
            counts
                .iter()
                .map(|row| estimate_u64(row, "/count"))
                .sum::<u64>(),
            total
        );
        assert_eq!(
            counts
                .iter()
                .map(|row| estimate_u64(row, "/output_elements"))
                .sum::<u64>(),
            estimate_u64(&response, "/result/estimate/elements")
        );
        // 上限は明細の数だけに効き、集計と実際のファイルは変えない。
        if let Some(previous) = estimates.first() {
            for pointer in [
                "/result/estimate/counts",
                "/result/estimate/total",
                "/result/estimate/hash",
                "/result/estimate/elements",
                "/diagnostics",
            ] {
                assert_eq!(
                    response.pointer(pointer),
                    previous.pointer(pointer),
                    "{pointer}"
                );
            }
        }
        let (status, missing) = call(&mut session, r#"{"v":1,"op":"drawing","doc":"out.dxf"}"#);
        assert_eq!(status, Status::Failed);
        assert_eq!(codes_of(&missing), ["call.document.missing"]);
        assert_eq!(seen(&mut session, name), before);
        estimates.push(response);
    }
    let estimate = estimates.pop().unwrap();
    let (status, converted) = call(
        &mut session,
        &format!(
            r#"{{"v":1,"op":"convert","doc":"{name}","format":"dxf","to":"out.dxf"{places}}}"#
        ),
    );
    assert_eq!(status, Status::Ok, "{converted:?}");
    for field in ["doc", "format", "hash", "elements"] {
        assert_eq!(
            estimate.pointer(&format!("/result/estimate/{field}")),
            converted.pointer(&format!("/result/{field}")),
            "{field}"
        );
    }
    assert_eq!(
        estimate_diagnostics_json(&estimate),
        estimate_diagnostics_json(&converted)
    );
    assert!(session.take(0).is_some());
    assert_eq!(seen(&mut session, name), before);
    let (_, drawing) = call(
        &mut session,
        &format!(r#"{{"v":1,"op":"drawing","doc":"{name}"}}"#),
    );
    assert_eq!(text(&drawing, "/result/hash"), source_hash);
    estimate
}

#[test]
fn estimate_of_the_lower_matches_the_written_file_without_registering_it() {
    let estimate = estimate_matches_conversion(CONVERTIBLE.as_bytes(), "source.dxf");
    assert_eq!(estimate_u64(&estimate, "/result/estimate/elements"), 6);
    let details = estimate_array(&estimate, "/result/estimate/details");
    assert!(
        details
            .iter()
            .any(|item| text(item, "/fidelity") == "dropped")
    );
    assert!(
        details
            .iter()
            .any(|item| text(item, "/fidelity") == "drawn")
    );
    assert!(
        details
            .iter()
            .any(|item| item.pointer("/source/record").is_some())
    );
}

#[test]
fn estimate_of_same_family_reencoding_matches_the_written_file() {
    let estimate = estimate_matches_conversion(
        include_bytes!("../../../../samples/dwg/zumen-arcdim-r2004.dwg"),
        "source.dwg",
    );
    assert_eq!(estimate_u64(&estimate, "/result/estimate/elements"), 79);
    let details = estimate_array(&estimate, "/result/estimate/details");
    assert!(
        details
            .iter()
            .any(|item| text(item, "/fidelity") == "native")
    );
    assert!(
        details
            .iter()
            .any(|item| item.pointer("/source/bytes").is_some())
    );
    // 一つの図形に拡張辞書とリアクターの診断があっても、図形は一つとして数える。
    assert!(
        estimate_array(&estimate, "/diagnostics")
            .iter()
            .any(|item| estimate_u64(item, "/count") > 1)
    );
}

#[test]
fn estimate_does_not_count_failed_attributes_as_output() {
    let source = CONVERTIBLE.replace("AC1015", "AC1021").replace("D-1", "😺");
    let estimate = estimate_matches_conversion(source.as_bytes(), "source.dxf");
    assert_eq!(estimate_u64(&estimate, "/result/estimate/elements"), 5);
    assert!(!codes_of(&estimate).contains(&"convert.attribute.drawn".to_string()));
}

#[test]
fn estimate_handles_a_missing_block_through_the_same_writer() {
    let source = CONVERTIBLE.replace("2\nDOOR\n10\n50.0", "2\nMISSING\n10\n50.0");
    let estimate = estimate_matches_conversion(source.as_bytes(), "source.dxf");
    assert!(codes_of(&estimate).contains(&"convert.element.unsupported".to_string()));
}

#[test]
fn estimate_failure_and_cancellation_keep_the_original_and_allow_retry() {
    let mut session = Session::new(b"").unwrap();
    estimate_source(&mut session, CONVERTIBLE.as_bytes(), "source.dxf");
    let before = seen(&mut session, "source.dxf");
    let request =
        r#"{"v":1,"op":"convert_estimate","doc":"source.dxf","format":"dxf","to":"out.dxf"}"#;
    let cancelled = CancelToken::new();
    cancelled.cancel();
    let response = session.call(request.as_bytes(), &cancelled);
    assert_eq!(response.status, Status::Failed);
    let response = zumen_json::parse(&response.response).unwrap();
    assert_eq!(codes_of(&response), ["call.request.cancelled"]);
    for work in [0, 1, 64, 1024] {
        let (status, response) = call(
            &mut session,
            &format!(
                r#"{{"v":1,"op":"convert_estimate","doc":"source.dxf","format":"dxf","to":"out.dxf","budget":{{"work":{work}}}}}"#
            ),
        );
        assert_eq!(status, Status::Failed, "{response:?}");
        assert_eq!(
            codes_of(&response).last().map(String::as_str),
            Some("budget.work.exceeded")
        );
        assert!(response.pointer("/result").is_none());
        assert!(session.take(0).is_none());
        assert_eq!(seen(&mut session, "source.dxf"), before);
    }
    assert_eq!(call(&mut session, request).0, Status::Ok);
    assert_eq!(
        call(
            &mut session,
            r#"{"v":1,"op":"convert","doc":"source.dxf","format":"dxf","to":"out.dxf"}"#
        )
        .0,
        Status::Ok
    );
}

#[test]
fn estimate_checks_names_formats_and_limits_without_registering_output() {
    let mut session = Session::new(b"").unwrap();
    estimate_source(&mut session, CONVERTIBLE.as_bytes(), "source.dxf");
    let before = seen(&mut session, "source.dxf");
    for (request, code) in [
        (
            r#"{"v":1,"op":"convert_estimate","doc":"source.dxf","format":"sxf","to":"out.sfc"}"#,
            "convert.format.unsupported",
        ),
        (
            r#"{"v":1,"op":"convert_estimate","doc":"source.dxf","format":"dxf","to":"source.dxf"}"#,
            "call.document.exists",
        ),
        (
            r#"{"v":1,"op":"convert_estimate","doc":"missing.dxf","format":"dxf","to":"out.dxf"}"#,
            "call.document.missing",
        ),
    ] {
        let (status, response) = call(&mut session, request);
        assert_eq!(status, Status::Failed, "{response:?}");
        assert_eq!(codes_of(&response), [code]);
        assert!(session.take(0).is_none());
        assert_eq!(seen(&mut session, "source.dxf"), before);
    }
    for request in [
        r#"{"v":1,"op":"convert_estimate","doc":"source.dxf","format":"dxf","to":"../out.dxf"}"#,
        r#"{"v":1,"op":"convert_estimate","doc":"source.dxf","format":"dxf","to":"out.dxf","limit":1001}"#,
        r#"{"v":1,"op":"convert_estimate","doc":"source.dxf","format":"dxf","to":"out.dxf","limit":-1}"#,
    ] {
        let (status, response) = call(&mut session, request);
        assert!(
            matches!(status, Status::Failed | Status::Invalid),
            "{response:?}"
        );
        assert!(response.pointer("/result").is_none());
        assert!(session.take(0).is_none());
    }
}

#[test]
fn estimate_sources_are_independent_of_the_diagnostic_place_limit() {
    let mut session = Session::new(b"").unwrap();
    estimate_source(&mut session, CONVERTIBLE.as_bytes(), "source.dxf");
    let mut responses = Vec::new();
    for max_places in [0, 100] {
        let (status, response) = call(
            &mut session,
            &format!(
                r#"{{"v":1,"op":"convert_estimate","doc":"source.dxf","format":"dxf","to":"out.dxf","limit":1000,"max_places":{max_places}}}"#
            ),
        );
        assert_eq!(status, Status::Ok, "{response:?}");
        let details = estimate_array(&response, "/result/estimate/details");
        assert!(
            details
                .iter()
                .any(|item| item.pointer("/source/record").is_some())
        );
        if max_places == 0 {
            assert!(
                estimate_array(&response, "/diagnostics")
                    .iter()
                    .all(|item| item
                        .pointer("/at")
                        .is_none_or(|at| *at == Value::Array(Vec::new())))
            );
        }
        responses.push(response);
    }
    assert_eq!(
        responses[0].pointer("/result/estimate"),
        responses[1].pointer("/result/estimate")
    );
    assert_eq!(codes_of(&responses[0]), codes_of(&responses[1]));
    let counts = |response: &Value| {
        estimate_array(response, "/diagnostics")
            .iter()
            .map(|item| estimate_u64(item, "/count"))
            .collect::<Vec<_>>()
    };
    assert_eq!(counts(&responses[0]), counts(&responses[1]));
}

#[test]
fn estimate_keeps_lift_losses_in_diagnostics_outside_the_writer_counts() {
    let baseline = estimate_matches_conversion(CONVERTIBLE.as_bytes(), "source.dxf");
    let source = CONVERTIBLE.replace(
        "0\nENDSEC\n0\nEOF\n",
        "0\nXLINE\n5\n60\n8\n0\n10\n0.0\n20\n0.0\n11\n1.0\n21\n0.0\n0\nENDSEC\n0\nEOF\n",
    );
    let estimate = estimate_matches_conversion(source.as_bytes(), "source.dxf");
    let lost = estimate_array(&estimate, "/diagnostics")
        .iter()
        .find(|item| {
            text(item, "/code") == "dxf.lift.dropped" && text(item, "/params/kind") == "XLINE"
        })
        .expect("lift が落とした XLINE の診断がない");
    assert_eq!(estimate_u64(lost, "/count"), 1);
    assert!(!estimate_array(lost, "/at").is_empty());
    // 元ファイルに XLINE が一つ増えても、lower が扱う図面モデルの項目は増えない。
    for pointer in [
        "/result/estimate/counts",
        "/result/estimate/total",
        "/result/estimate/elements",
    ] {
        assert_eq!(
            estimate.pointer(pointer),
            baseline.pointer(pointer),
            "{pointer}"
        );
    }
}

// 同じ LINE の拡張データを五つ落とす。1099 は異なる登録名で二度現れ、
// 診断の code・params・source が同じでも、別々の報告として残す。
const ESTIMATE_XDATA_LOSSES: &str = "0\nSECTION\n2\nHEADER\n9\n$ACADVER\n1\nAC1015\n9\n$INSUNITS\n70\n4\n0\nENDSEC\n\
    0\nSECTION\n2\nENTITIES\n0\nLINE\n5\nA\n8\n0\n10\n0.0\n20\n0.0\n11\n1.0\n21\n0.0\n\
    1000\norphan\n1001\nA\n1010\n1.0\n1020\n2.0\n1001\nB\n1002\n[\n\
    1001\nC\n1099\nx\n1001\nE\n1099\ny\n1001\nD\n1070\n1\n0\nENDSEC\n0\nEOF\n";

fn estimate_input_diagnostics(response: &Value) -> &Value {
    response
        .pointer("/result/estimate/input_diagnostics")
        .expect("入力の診断の集計がない")
}

fn estimate_diagnostics_json(response: &Value) -> Vec<u8> {
    let mut out = zumen_json::Writer::new();
    out.value(response.pointer("/diagnostics").unwrap());
    out.into_bytes()
}

#[test]
fn estimate_input_reports_keep_multiple_losses_at_one_source() {
    let mut session = Session::new(b"").unwrap();
    estimate_source(&mut session, ESTIMATE_XDATA_LOSSES.as_bytes(), "source.dxf");
    let (status, estimate) = call(
        &mut session,
        r#"{"v":1,"op":"convert_estimate","doc":"source.dxf","format":"dxf","to":"out.dxf","limit":1000,"max_places":0}"#,
    );
    assert_eq!(status, Status::Ok, "{estimate:?}");
    let input = estimate_input_diagnostics(&estimate);
    assert_eq!(estimate_u64(input, "/total"), 5);
    assert_eq!(input.pointer("/truncated"), Some(&Value::Bool(false)));
    let counts = estimate_array(input, "/counts");
    assert_eq!(counts.len(), 4);
    assert!(
        counts
            .iter()
            .all(|diagnostic| estimate_array(diagnostic, "/at").is_empty())
    );
    let repeated = counts
        .iter()
        .find(|diagnostic| estimate_u64(diagnostic, "/params/code") == 1099)
        .unwrap();
    assert_eq!(estimate_u64(repeated, "/count"), 2);
    let details = estimate_array(input, "/details");
    assert_eq!(details.len(), 5);
    assert_eq!(
        details
            .iter()
            .map(|detail| estimate_u64(detail, "/diagnostic/params/code"))
            .collect::<Vec<_>>(),
        [1000, 1030, 1002, 1099, 1099]
    );
    for detail in details {
        assert_eq!(text(detail, "/diagnostic/code"), "dxf.lift.invalid");
        assert_eq!(text(detail, "/diagnostic/fidelity"), "dropped");
        assert_eq!(text(detail, "/diagnostic/params/kind"), "LINE");
        assert_eq!(estimate_u64(detail, "/diagnostic/count"), 1);
        assert_eq!(estimate_u64(detail, "/diagnostic/at/0/record"), 3);
        assert_eq!(estimate_u64(detail, "/sources/0/record"), 3);
        assert_eq!(text(detail, "/sources/0/handle"), "A");
    }
    // 同じ source の二つの報告を、値の一致でまとめ直さない。
    assert_eq!(details[3], details[4]);
    assert_eq!(estimate_u64(&estimate, "/result/estimate/elements"), 1);
    assert!(
        estimate_array(&estimate, "/diagnostics")
            .iter()
            .all(|diagnostic| estimate_array(diagnostic, "/at").is_empty())
    );
    let (status, converted) = call(
        &mut session,
        r#"{"v":1,"op":"convert","doc":"source.dxf","format":"dxf","to":"out.dxf","max_places":0}"#,
    );
    assert_eq!(status, Status::Ok, "{converted:?}");
    assert_eq!(
        estimate_diagnostics_json(&estimate),
        estimate_diagnostics_json(&converted)
    );
    assert_eq!(
        text(&estimate, "/result/estimate/hash"),
        text(&converted, "/result/hash")
    );
    assert!(session.take(0).is_some());
}

#[test]
fn estimate_input_report_limits_keep_all_counts_and_original_places() {
    let mut source = String::from(
        "0\nSECTION\n2\nHEADER\n9\n$ACADVER\n1\nAC1015\n9\n$INSUNITS\n70\n4\n0\nENDSEC\n0\nSECTION\n2\nENTITIES\n",
    );
    for index in 0..40 {
        source.push_str(&format!(
            "0\nLINE\n5\n{:X}\n8\n0\n10\n0.0\n20\n0.0\n11\n1.0\n21\n0.0\n1000\norphan\n",
            index + 1
        ));
    }
    source.push_str("0\nENDSEC\n0\nEOF\n");
    let mut session = Session::new(b"").unwrap();
    estimate_source(&mut session, source.as_bytes(), "source.dxf");
    let mut responses: Vec<Value> = Vec::new();
    for (limit, max_places) in [(0, 0), (2, 0), (1000, 0), (1000, 16)] {
        let (status, response) = call(
            &mut session,
            &format!(
                r#"{{"v":1,"op":"convert_estimate","doc":"source.dxf","format":"dxf","to":"out.dxf","limit":{limit},"max_places":{max_places}}}"#
            ),
        );
        assert_eq!(status, Status::Ok, "{response:?}");
        let input = estimate_input_diagnostics(&response);
        assert_eq!(estimate_u64(input, "/total"), 40);
        let counts = estimate_array(input, "/counts");
        assert_eq!(counts.len(), 1);
        assert_eq!(text(&counts[0], "/code"), "dxf.lift.invalid");
        assert_eq!(estimate_u64(&counts[0], "/count"), 40);
        assert!(estimate_array(&counts[0], "/at").is_empty());
        let details = estimate_array(input, "/details");
        assert_eq!(details.len(), (limit as usize).min(40));
        assert_eq!(input.pointer("/truncated"), Some(&Value::Bool(limit < 40)));
        for (index, detail) in details.iter().enumerate() {
            assert_eq!(estimate_u64(detail, "/diagnostic/count"), 1);
            assert_eq!(
                estimate_u64(detail, "/diagnostic/at/0/record"),
                3 + index as u64
            );
            assert_eq!(estimate_u64(detail, "/sources/0/record"), 3 + index as u64);
            assert_eq!(
                text(detail, "/sources/0/handle"),
                format!("{:X}", index + 1)
            );
        }
        let diagnostic = estimate_array(&response, "/diagnostics")
            .iter()
            .find(|diagnostic| text(diagnostic, "/code") == "dxf.lift.invalid")
            .unwrap();
        assert_eq!(estimate_array(diagnostic, "/at").len(), max_places);
        if let Some(first) = responses.first() {
            for pointer in [
                "/result/estimate/total",
                "/result/estimate/counts",
                "/result/estimate/elements",
                "/result/estimate/hash",
                "/result/estimate/input_diagnostics/counts",
            ] {
                assert_eq!(
                    response.pointer(pointer),
                    first.pointer(pointer),
                    "{pointer}"
                );
            }
        }
        responses.push(response);
    }
    assert_eq!(
        estimate_input_diagnostics(&responses[2]),
        estimate_input_diagnostics(&responses[3])
    );
    let (status, converted) = call(
        &mut session,
        r#"{"v":1,"op":"convert","doc":"source.dxf","format":"dxf","to":"out.dxf","max_places":0}"#,
    );
    assert_eq!(status, Status::Ok, "{converted:?}");
    assert_eq!(
        estimate_diagnostics_json(&responses[2]),
        estimate_diagnostics_json(&converted)
    );
}

#[test]
fn estimate_input_reports_include_diagnostics_without_fidelity_or_source() {
    let source = CONVERTIBLE.replace("9\n$INSUNITS\n70\n4\n", "");
    let estimate = estimate_matches_conversion(source.as_bytes(), "source.dxf");
    let input = estimate_input_diagnostics(&estimate);
    assert_eq!(estimate_u64(input, "/total"), 1);
    assert_eq!(estimate_array(input, "/details").len(), 1);
    assert_eq!(
        text(input, "/details/0/diagnostic/code"),
        "dxf.units.missing"
    );
    assert!(input.pointer("/details/0/diagnostic/fidelity").is_none());
    assert!(input.pointer("/details/0/diagnostic/params/kind").is_none());
    assert!(estimate_array(input, "/details/0/diagnostic/at").is_empty());
    assert!(estimate_array(input, "/details/0/sources").is_empty());
}

#[test]
fn same_family_writer_diagnostics_are_not_input_diagnostics() {
    let estimate = estimate_matches_conversion(
        include_bytes!("../../../../samples/dwg/zumen-arcdim-r2004.dwg"),
        "source.dwg",
    );
    assert!(codes_of(&estimate).contains(&"dwg.dxf-reference.dropped".to_string()));
    let input = estimate_input_diagnostics(&estimate);
    assert_eq!(estimate_u64(input, "/total"), 0);
    assert!(estimate_array(input, "/counts").is_empty());
    assert!(estimate_array(input, "/details").is_empty());
    assert_eq!(input.pointer("/truncated"), Some(&Value::Bool(false)));
}

const ESTIMATE_P21_LOSSES: &str = concat!(
    "ISO-10303-21;\nHEADER;\nFILE_DESCRIPTION(('SCADEC level2 AP202_mode'),'2;1');\n",
    "FILE_SCHEMA(('ASSOCIATIVE_DRAUGHTING'));\nENDSEC;\nDATA;\n",
    "#1=PRESENTATION_STYLE_ASSIGNMENT(());\n",
    "#2=CARTESIAN_POINT(' ',(1.,2.));\n",
    "#3=CARTESIAN_POINT(' ',(4.,6.));\n",
    "#4=POLYLINE(' ',(#2,#3));\n",
    "#5=ANNOTATION_CURVE_OCCURRENCE(' ',(#1),#4);\n",
    "#110=ANNOTATION_CURVE_OCCURRENCE(' ',(#1),#999);\n",
    "#20=PRESENTATION_LAYER_ASSIGNMENT('L',' ',(#5,#110));\n",
    "#70=DRAWING_SHEET_REVISION('A3_horizontal',(#110,#5),$,'01');\n",
    "ENDSEC;\nEND-ISO-10303-21;\n",
);

#[test]
fn estimate_input_reports_include_p21_reference_and_lift_diagnostics_once() {
    let estimate = estimate_matches_conversion_with_places(
        ESTIMATE_P21_LOSSES.as_bytes(),
        "source.p21",
        Some(0),
    );
    let input = estimate_input_diagnostics(&estimate);
    assert_eq!(estimate_u64(input, "/total"), 2);
    assert_eq!(estimate_array(input, "/counts").len(), 2);
    let details = estimate_array(input, "/details");
    assert_eq!(details.len(), 2);
    let reference = details
        .iter()
        .find(|detail| text(detail, "/diagnostic/code") == "sxf.p21-reference.missing")
        .unwrap();
    let dropped = details
        .iter()
        .find(|detail| text(detail, "/diagnostic/code") == "sxf.lift.dropped")
        .unwrap();
    assert_eq!(estimate_u64(reference, "/diagnostic/count"), 1);
    assert_eq!(estimate_u64(reference, "/diagnostic/params/id"), 999);
    assert!(reference.pointer("/diagnostic/fidelity").is_none());
    assert_eq!(
        text(dropped, "/diagnostic/params/kind"),
        "annotation_curve_occurrence"
    );
    assert_eq!(text(dropped, "/diagnostic/fidelity"), "dropped");
    assert_eq!(text(dropped, "/sources/0/instance"), "110");
    assert_eq!(
        reference.pointer("/sources/0/record"),
        dropped.pointer("/sources/0/record")
    );
    assert!(!estimate_array(reference, "/diagnostic/at").is_empty());
    assert!(!estimate_array(dropped, "/diagnostic/at").is_empty());
    assert!(
        estimate_array(&estimate, "/diagnostics")
            .iter()
            .all(|diagnostic| estimate_array(diagnostic, "/at").is_empty())
    );
    assert_eq!(estimate_u64(&estimate, "/result/estimate/elements"), 1);
}

fn estimate_jww_with_invalid_and_valid_lines() -> Vec<u8> {
    // 既存の適合テストの合成入力を再用し、同じクラスの正常な線を後ろへ足す。
    let case = zumen_json::parse(include_bytes!(
        "../../../../bindings/conformance/cases/open_jww.json"
    ))
    .unwrap();
    let hex = text(&case, "/steps/0/push/hex");
    let source: Vec<u8> = hex
        .as_bytes()
        .as_chunks::<2>()
        .0
        .iter()
        .map(|pair| u8::from_str_radix(core::str::from_utf8(pair).unwrap(), 16).unwrap())
        .collect();
    let mut budget = Budget::default();
    let mut diagnostics = Diagnostics::new();
    let cancel = CancelToken::new();
    let file = <zumen_jww::Jww as zumen_sdk::Codec>::read(
        &source,
        &mut Cx::new(&mut budget, &mut diagnostics, &cancel),
    )
    .unwrap();
    let entity = file
        .records()
        .iter()
        .find(|record| record.kind() == zumen_jww::RecordKind::Entity)
        .unwrap();
    let span = entity.span();
    let body = entity.body();
    let mut bad = source[span.clone()].to_vec();
    let coordinate = bad.len() - 4 * 8;
    bad[coordinate..coordinate + 8].copy_from_slice(&f64::NAN.to_le_bytes());
    let mut input = source[..span.start - 2].to_vec();
    input.extend(2_u16.to_le_bytes());
    input.extend(bad);
    // MFC CArchive の既知のクラス 1。新しいクラスの宣言を複製しない。
    input.extend(0x8001_u16.to_le_bytes());
    input.extend_from_slice(&source[body]);
    input.extend_from_slice(&source[span.end..]);
    input
}

#[test]
fn estimate_input_reports_keep_jww_invalid_entities_and_the_following_line() {
    let source = estimate_jww_with_invalid_and_valid_lines();
    let estimate = estimate_matches_conversion_with_places(&source, "source.jww", Some(0));
    let input = estimate_input_diagnostics(&estimate);
    assert_eq!(estimate_u64(input, "/total"), 1);
    assert_eq!(estimate_array(input, "/counts").len(), 1);
    assert_eq!(text(input, "/counts/0/code"), "jww.lift.invalid");
    assert!(estimate_array(input, "/counts/0/at").is_empty());
    assert_eq!(estimate_array(input, "/details").len(), 1);
    assert_eq!(
        text(input, "/details/0/diagnostic/code"),
        "jww.lift.invalid"
    );
    assert_eq!(text(input, "/details/0/diagnostic/params/kind"), "CDataSen");
    assert_eq!(text(input, "/details/0/diagnostic/fidelity"), "dropped");
    assert_eq!(estimate_u64(input, "/details/0/diagnostic/at/0/record"), 1);
    assert_eq!(estimate_u64(input, "/details/0/sources/0/record"), 1);
    assert_eq!(estimate_u64(&estimate, "/result/estimate/elements"), 1);
    assert!(
        estimate_array(&estimate, "/diagnostics")
            .iter()
            .all(|diagnostic| estimate_array(diagnostic, "/at").is_empty())
    );
}
