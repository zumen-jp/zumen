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
                r#"{{"v":1,"op":"convert_estimate","doc":"{name}","format":"dxf","to":"out.dxf","limit":{limit}}}"#
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
        &format!(r#"{{"v":1,"op":"convert","doc":"{name}","format":"dxf","to":"out.dxf"}}"#),
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
        estimate.pointer("/diagnostics"),
        converted.pointer("/diagnostics")
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
