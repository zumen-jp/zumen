//! 出来形の差の色分けのシーン（#476）を、deviation が返した差・規格値・色の段と照らす。
#![allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)] // 合成入力の成功と応答の値を検査するテスト。
use super::*;
use zumen_json::Value;

/// 標高 0 の 20 m 四方の計画面
const PLAN: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<LandXML xmlns=\"http://www.landxml.org/schema/LandXML-1.2\" version=\"1.2\">\n<Surfaces><Surface name=\"plan\" desc=\"FinishedGrade\"><Definition surfType=\"TIN\"><Pnts><P id=\"1\">0 0 0</P><P id=\"2\">0 20 0</P><P id=\"3\">20 20 0</P><P id=\"4\">20 0 0</P></Pnts><Faces><F>1 2 3</F><F>1 3 4</F></Faces></Definition></Surface></Surfaces>\n</LandXML>\n";

/// 平場の規格値（±150 mm）に対して 0・半分・ちょうど・外の差を持つ点と、TIN の外の点
const POINTS: &str = "1 1 0\n2 2 0.075\n3 3 0.1\n4 4 0.15\n5 5 0.2\n6 6 -0.075\n7 7 -0.1\n8 8 -0.15\n9 9 -0.2\n30 30 0\n";

const PROFILE: &[u8] = include_bytes!("../../../../profiles/mlit-earthwork/2026-03/profile.json");

const REQUEST: &str = r#"{"v":1,"op":"deviation","doc":"a.xyz","surface":{"doc":"plan.xml","id":"plan"},"profiles":["profile.json"],"profile":{"id":"mlit-earthwork","edition":"2026-03"},"work":"road-excavation","limit":100"#;

fn call(session: &mut Session, request: &str) -> (Status, Value) {
    let reply = session.call(request.as_bytes(), &CancelToken::new());
    (reply.status, zumen_json::parse(&reply.response).unwrap())
}

fn num(value: Option<&Value>) -> Option<f64> {
    match value {
        Some(Value::Number(n)) => n.to_f64(),
        _ => None,
    }
}

fn text<'a>(value: &'a Value, pointer: &str) -> &'a str {
    match value.pointer(pointer) {
        Some(Value::String(text)) => text,
        other => panic!("{pointer}: {other:?}"),
    }
}

fn open() -> Session {
    let mut session = Session::new(b"").unwrap();
    session.push(b"plan.xml", PLAN.as_bytes()).unwrap();
    session.push(b"a.xyz", POINTS.as_bytes()).unwrap();
    session.push(b"profile.json", PROFILE).unwrap();
    for request in [
        r#"{"v":1,"op":"open","input":"plan.xml"}"#,
        r#"{"v":1,"op":"open","input":"a.xyz","format":"xyz"}"#,
    ] {
        assert_eq!(call(&mut session, request).0, Status::Ok);
    }
    session
}

/// 段の表から差（mm）の入る段を、文書の端の扱いで引く。規格値ちょうどは内側、内側の境は 0 に近い段に入れる
fn band_of<'a>(bands: &'a [Value], rule: &str, mm: f64) -> &'a Value {
    bands
        .iter()
        .filter(|band| band.pointer("/rule").is_some() && text(band, "/rule") == rule)
        .find(|band| {
            let lower = num(band.pointer("/lower"));
            let upper = num(band.pointer("/upper"));
            match text(band, "/band") {
                "below" => upper.is_some_and(|u| mm < u),
                "above" => lower.is_some_and(|l| mm > l),
                "low_outer" | "low_inner" => {
                    lower.is_none_or(|l| mm >= l) && upper.is_some_and(|u| mm < u)
                }
                "high_inner" => lower.is_some_and(|l| mm >= l) && upper.is_none_or(|u| mm <= u),
                "high_outer" => lower.is_some_and(|l| mm > l) && upper.is_some_and(|u| mm <= u),
                other => panic!("{other}"),
            }
        })
        .unwrap_or_else(|| panic!("{rule} {mm}"))
}

fn color(value: Option<&Value>) -> Vec<u16> {
    match value {
        Some(Value::Array(items)) => items
            .iter()
            .map(|v| u16::try_from(num(Some(v)).unwrap() as u64).unwrap())
            .collect(),
        other => panic!("{other:?}"),
    }
}

#[test]
fn deviation_scene_colors_follow_differences_and_limits() {
    let mut session = open();
    let (status, value) = call(&mut session, &format!(r#"{REQUEST},"scene3d":{{}}}}"#));
    assert_eq!(status, Status::Ok, "{value:?}");
    let Some(Value::Array(bands)) = value.pointer("/result/bands") else {
        panic!("{value:?}");
    };
    let Some(Value::Array(differences)) = value.pointer("/result/differences") else {
        panic!("{value:?}");
    };
    assert_eq!(differences.len(), 9);
    assert_eq!(num(value.pointer("/result/outside")), Some(1.0));
    // 差ごとに、期待する段の色。点群の順
    let mut expected: Vec<([f64; 3], Vec<u16>, String)> = Vec::new();
    for point in differences {
        let rule = text(point, "/rule");
        let mm = num(point.pointer("/difference")).unwrap() * 1000.0;
        let band = band_of(bands, rule, mm);
        let p: Vec<f64> = (0..3)
            .map(|i| num(point.pointer(&format!("/position/{i}"))).unwrap())
            .collect();
        // position は北・東・標高、シーンは東・北・高さ
        expected.push((
            [p[1], p[0], p[2]],
            color(band.pointer("/color")),
            text(band, "/band").to_string(),
        ));
    }
    let names: Vec<&str> = expected.iter().map(|(_, _, name)| name.as_str()).collect();
    assert_eq!(
        names,
        [
            "high_inner",
            "high_inner",
            "high_outer",
            "high_outer",
            "above",
            "low_inner",
            "low_outer",
            "low_outer",
            "below"
        ]
    );
    // 規格値を外れた点（above・below）は、指摘の最大・最小に現れる
    let Some(Value::Array(findings)) = value.pointer("/result/findings") else {
        panic!("{value:?}");
    };
    let statistics: Vec<&str> = findings
        .iter()
        .filter(|f| f.pointer("/diagnostic/params/statistic").is_some())
        .map(|f| text(f, "/diagnostic/params/statistic"))
        .collect();
    assert!(statistics.contains(&"maximum"), "{findings:?}");
    assert!(statistics.contains(&"minimum"), "{findings:?}");
    let not_evaluated = bands.last().unwrap();
    assert_eq!(text(not_evaluated, "/band"), "not_evaluated");
    assert!(not_evaluated.pointer("/rule").is_none());
    expected.push((
        [30.0, 30.0, 0.0],
        color(not_evaluated.pointer("/color")),
        "not_evaluated".into(),
    ));

    // シーンの点の色を、座標で元の点に戻して比べる
    assert_eq!(
        num(value.pointer("/result/scene3d/counts/points")),
        Some(10.0)
    );
    let blob = num(value.pointer("/result/scene3d/blob")).unwrap() as u32;
    let bytes = session.take(blob).unwrap();
    let mut budget = zumen_base::Budget::new(zumen_base::Limits::default());
    let mut diagnostics = zumen_base::Diagnostics::new();
    let cancel = CancelToken::new();
    let mut cx = Cx::new(&mut budget, &mut diagnostics, &cancel);
    let reader = zumen_scene3d::read(&bytes, &mut cx).unwrap();
    let origins = reader.column(zumen_scene3d::layout::CHUNK_ORIGINS).unwrap();
    let mut seen = 0;
    for chunk in 0..u64::from(reader.chunks) {
        let points = reader.points(chunk).unwrap();
        for i in 0..points.positions.rows() {
            let at: Vec<f64> = (0..3)
                .map(|axis| {
                    origins.f64(chunk * 3 + axis).unwrap()
                        + f64::from(points.positions.f32(i * 3 + axis).unwrap())
                })
                .collect();
            let (_, rgba, name) = expected
                .iter()
                .find(|(p, _, _)| (0..3).all(|k| (p[k] - at[k]).abs() < 1e-4))
                .unwrap_or_else(|| panic!("{at:?}"));
            let got: Vec<u16> = (0..4)
                .map(|k| points.rgba.u16(i * 4 + k).unwrap())
                .collect();
            assert_eq!(&got, rgba, "{name}");
            seen += 1;
        }
    }
    assert_eq!(seen, 10);
}

#[test]
fn deviation_scene_selection_and_errors() {
    let mut session = open();
    // 書かなければシーンも段も返さない
    let (status, value) = call(&mut session, &format!("{REQUEST}}}"));
    assert_eq!(status, Status::Ok, "{value:?}");
    assert!(value.pointer("/result/scene3d").is_none());
    assert!(value.pointer("/result/bands").is_none());
    let (status, value) = call(
        &mut session,
        &format!(r#"{REQUEST},"scene3d":{{"limit":4,"box":{{"min":[0,0,-1],"max":[6,6,1]}}}}}}"#),
    );
    assert_eq!(status, Status::Ok, "{value:?}");
    assert_eq!(
        num(value.pointer("/result/scene3d/counts/points")),
        Some(4.0)
    );
    assert!(matches!(
        value.pointer("/result/scene3d/truncated"),
        Some(Value::Bool(true))
    ));
    for (scene, pointer) in [
        (r#"{"tolerance":0}"#, "/scene3d/tolerance"),
        (r#"{"box":{"min":[1,1,1],"max":[0,0,0]}}"#, "/scene3d/box"),
    ] {
        let (status, value) = call(&mut session, &format!(r#"{REQUEST},"scene3d":{scene}}}"#));
        assert_eq!(status, Status::Failed, "{value:?}");
        assert_eq!(text(&value, "/diagnostics/0/params/pointer"), pointer);
    }
}
