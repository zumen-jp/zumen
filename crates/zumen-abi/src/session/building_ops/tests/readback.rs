//! 公開の取得操作で、編集中の文書と、保存した版を別のセッションで読み直した文書を照らす。

use super::*;

const PROJECT: &str = "0000000000000000000001";
const STOREY: &str = "0000000000000000000030";
const WALL: &str = "0000000000000000000050";
const IDS: &[u8] =
    br#"<ids xmlns="http://standards.buildingsmart.org/IDS"><info><title>Project</title></info>
<specifications><specification name="name" ifcVersion="IFC4"><applicability maxOccurs="unbounded">
<entity><name><simpleValue>IFCPROJECT</simpleValue></name></entity></applicability>
<requirements><attribute><name><simpleValue>Name</simpleValue></name>
<value><simpleValue>Renamed project</simpleValue></value></attribute></requirements>
</specification></specifications></ids>"#;

fn check_reads(session: &mut Session, name: &str, saved: &[u8], ids_status: &str, min_x: i64) {
    let mut reopened = opened(name, saved, "");
    reopened.push(b"check.ids", IDS).unwrap();
    let mut compare = |request: String, status| {
        let (actual, response) = same(session, &mut reopened, &request);
        assert_eq!(
            actual,
            status,
            "{request}: {}",
            String::from_utf8_lossy(&response)
        );
        zumen_json::parse(&response).unwrap()
    };
    let building = format!(r#"{{"v":1,"op":"building","doc":"{name}"}}"#);
    compare(building, Status::Ok);
    let components = crate::messages::BuildingComponent::ALL
        .iter()
        .map(|c| format!(r#""{}""#, c.name()))
        .collect::<Vec<_>>()
        .join(",");
    compare(
        format!(r#"{{"v":1,"op":"building_elements","doc":"{name}","components":[{components}]}}"#),
        Status::Ok,
    );
    let ids = compare(
        format!(r#"{{"v":1,"op":"ids_check","doc":"{name}","ids":"check.ids"}}"#),
        Status::Ok,
    );
    assert_eq!(
        ids.pointer("/result/specifications/0/status"),
        Some(&Value::String(ids_status.into()))
    );

    for limit in [0, 1] {
        let scene = compare(
            format!(r#"{{"v":1,"op":"scene3d","doc":"{name}","max_places":{limit}}}"#),
            Status::Ok,
        );
        let x = zumen_json::number(scene.pointer("/result/extent/min/0").unwrap()).unwrap();
        assert!((x - min_x as f64).abs() < 0.001);
        let Some(Value::Array(diagnostics)) = scene.pointer("/diagnostics") else {
            panic!("診断の配列がない")
        };
        let missing = diagnostics
            .iter()
            .find(|d| d.pointer("/code") == Some(&Value::String("ifc.shape.missing".into())))
            .unwrap();
        assert_eq!(missing.pointer("/at/0/bytes/start").is_some(), limit > 0);
    }

    let plan = format!(
        r#""op":"building_drawing","doc":"{name}","to":"plan","storey":"{STOREY}","height":1200,"depth":2000,"unit":"millimeter","tolerance":0.01,"chord":1,"scale":100"#
    );
    let stopped = compare(
        format!(r#"{{"v":1,{plan},"budget":{{"model_bytes":0}}}}"#),
        Status::Failed,
    );
    assert_eq!(stopped.pointer("/blobs"), Some(&Value::Array(vec![])));
    let Some(Value::Array(diagnostics)) = stopped.pointer("/diagnostics") else {
        panic!("診断の配列がない")
    };
    assert!(
        diagnostics
            .iter()
            .any(|d| d.pointer("/code")
                == Some(&Value::String("budget.model-bytes.exceeded".into())))
    );
    let cut = compare(format!(r#"{{"v":1,{plan}}}"#), Status::Ok);
    assert_eq!(
        zumen_json::uint(cut.pointer("/result/lines").unwrap(), 0, 4),
        Ok(4)
    );
    #[cfg(feature = "drawing")]
    {
        compare(r#"{"v":1,"op":"drawing","doc":"plan"}"#.into(), Status::Ok);
        compare(r#"{"v":1,"op":"entities","doc":"plan","fields":["id","geometry","extensions","source"]}"#.into(), Status::Ok);
        let scene = compare(
            r#"{"v":1,"op":"scene","doc":"plan","from":"space/0"}"#.into(),
            Status::Ok,
        );
        let Some(Value::Number(x)) = scene.pointer("/result/extent/min_x") else {
            panic!("範囲がない")
        };
        assert_eq!(x.to_safe_integer(), Some(min_x));
    }
    compare(r#"{"v":1,"op":"close","docs":["plan"]}"#.into(), Status::Ok);
}

#[test]
fn public_reads_after_building_edits_match_the_saved_version() {
    let spf = conformance_text("building_floor_plan", "/steps/0/push/text");
    let zipped = zumen_zip::write(&[("model/a.ifc", &spf)]).unwrap();
    for (name, bytes) in [("read.ifc", spf), ("read.ifczip", zipped)] {
        let mut session = opened(name, &bytes, "");
        session.push(b"check.ids", IDS).unwrap();
        let project_hash = element_hash(&mut session, name, PROJECT);
        let rename = attribute(PROJECT, &project_hash, "name", Some("Renamed project"));
        let (status, _, saved) = answer(&mut session, &request(name, "", &[rename]));
        assert_eq!(status, Status::Ok);
        check_reads(&mut session, name, &saved[0], "passed", -1000);

        let hash = element_hash(&mut session, name, WALL);
        let place = format!(
            r#"{{"kind":"place","element":{{"id":"{WALL}","hash":"{hash}"}},"placement":{{"matrix":[[1,0,0,5000],[0,1,0,200],[0,0,1,0]]}}}}"#
        );
        let (status, _, saved) = answer(&mut session, &request(name, "", &[place]));
        assert_eq!(status, Status::Ok);
        assert_eq!(session.engine_applies, 2);
        check_reads(&mut session, name, &saved[0], "passed", 4000);

        // Session で前の版に戻す公開手順。IfcLive の undo とは別に確かめる。
        let close = format!(r#"{{"v":1,"op":"close","docs":["{name}"]}}"#);
        assert_eq!(call(&mut session, &close).0, Status::Ok);
        let open = format!(r#"{{"v":1,"op":"open","input":"{name}"}}"#);
        assert_eq!(call(&mut session, &open).0, Status::Ok);
        check_reads(&mut session, name, &bytes, "failed", -1000);
    }
}

/// 空のプロジェクトに、既存の適合試料と同じ二階建ての編集を当て、保存結果を取得する。
fn saved_storeys() -> Vec<u8> {
    let fixture = zumen_json::parse(include_bytes!(
        "../../../../../../bindings/conformance/cases/building_storeys.json"
    ))
    .unwrap();
    let source = zumen_json::str(fixture.pointer("/steps/0/push/text").unwrap()).unwrap();
    let mut session = opened("a.ifc", source.as_bytes(), "");
    let mut writer = zumen_json::Writer::new();
    writer.value(fixture.pointer("/steps/3/call").unwrap());
    let req = String::from_utf8(writer.into_bytes()).unwrap();
    let (status, response, mut files) = answer(&mut session, &req);
    assert_eq!(status, Status::Ok, "{}", String::from_utf8_lossy(&response));
    files.remove(0)
}

#[test]
fn reopened_joinery_matches_the_opening_edit_and_keeps_the_other_storey() {
    let initial = saved_storeys();
    let mut edit = opened("a.ifc", &initial, "");
    let opening = "0000000000000000002020";
    let hash = element_hash(&mut edit, "a.ifc", opening);
    let shape = format!(
        r#"{{"kind":"opening_shape","opening":{{"id":"{opening}","hash":"{hash}"}},"distance":1500,"sill":0,"width":1050,"height":2150}}"#
    );
    let (status, response, revised) = answer(&mut edit, &request("a.ifc", "", &[shape]));
    assert_eq!(status, Status::Ok, "{}", String::from_utf8_lossy(&response));
    for (bytes, width, height) in [(&initial, 900, 2100), (&revised[0], 1050, 2150)] {
        let mut session = opened("saved.ifc", bytes, "");
        let list = r#"{"v":1,"op":"building_elements","doc":"saved.ifc","ids":["0000000000000000002021","0000000000000000003021","0000000000000000002020"],"components":["joinery","opening"]}"#;
        let (status, response) = call(&mut session, list);
        assert_eq!(status, Status::Ok);
        let result = zumen_json::parse(&response).unwrap();
        let number = |key| zumen_json::uint(result.pointer(key).unwrap(), 0, 100_000).unwrap();
        let text = |key| zumen_json::str(result.pointer(key).unwrap()).unwrap();
        assert_eq!(text("/result/elements/0/id"), "0000000000000000002021");
        assert_eq!(text("/result/elements/0/opening/fills"), opening);
        assert_eq!(
            text("/result/elements/2/opening/voids"),
            "0000000000000000002000"
        );
        assert_eq!(
            number("/result/elements/0/joinery/overall_width/value"),
            width
        );
        assert_eq!(
            number("/result/elements/0/joinery/overall_height/value"),
            height
        );
        assert_eq!(
            text("/result/elements/0/joinery/overall_width/unit/prefix"),
            "MILLI"
        );
        assert_eq!(text("/result/elements/1/id"), "0000000000000000003021");
        assert_eq!(
            number("/result/elements/1/joinery/overall_width/value"),
            1200
        );
        assert_eq!(
            number("/result/elements/1/joinery/overall_height/value"),
            1200
        );
    }
}
