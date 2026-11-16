//! CityGML の地物を共通の scene3d の入口へ接続し、座標・ID・失敗後の再利用を確かめる。
#![cfg(feature = "civil")]
#![allow(
    clippy::unwrap_used,
    clippy::panic,
    reason = "合成入力の固定した期待値"
)]
use zumen_abi1::{Session, Status};
use zumen_base::{Budget, CancelToken, Cx, Diagnostics};
use zumen_json::Value;

const INPUT: &str = r#"<core:CityModel xmlns:core="http://www.opengis.net/citygml/2.0" xmlns:gml="http://www.opengis.net/gml" xmlns:bldg="http://www.opengis.net/citygml/building/2.0"><gml:boundedBy><gml:Envelope srsName="EPSG:6677" srsDimension="3"><gml:lowerCorner>200 100 12</gml:lowerCorner><gml:upperCorner>210 110 12</gml:upperCorner></gml:Envelope></gml:boundedBy><core:cityObjectMember><bldg:Building gml:id="area"><gml:name>面</gml:name><bldg:lod2MultiSurface><gml:MultiSurface><gml:surfaceMember><gml:Polygon><gml:exterior><gml:LinearRing><gml:posList>200 100 12 200 110 12 210 110 12 210 100 12 200 100 12</gml:posList></gml:LinearRing></gml:exterior></gml:Polygon></gml:surfaceMember></gml:MultiSurface></bldg:lod2MultiSurface></bldg:Building></core:cityObjectMember></core:CityModel>"#;
const SCENE: &str = r#"{"v":1,"op":"scene3d","doc":"geo.gml","zone":9,"lod":2,"ids":["area"],"classes":["bldg:Building"],"tolerance":0.001}"#;

fn call(session: &mut Session, request: &str, expected: Status) -> Value {
    let reply = session.call(request.as_bytes(), &CancelToken::new());
    assert_eq!(
        reply.status,
        expected,
        "{}",
        String::from_utf8_lossy(&reply.response)
    );
    zumen_json::parse(&reply.response).unwrap()
}
fn open() -> Session {
    let mut session = Session::new(b"").unwrap();
    session.push(b"geo.gml", INPUT.as_bytes()).unwrap();
    call(
        &mut session,
        r#"{"v":1,"op":"open","input":"geo.gml"}"#,
        Status::Ok,
    );
    session
}
fn expected(value: &Value, path: &str, json: &str) {
    assert_eq!(
        value.pointer(path),
        Some(&zumen_json::parse(json.as_bytes()).unwrap()),
        "{path}"
    );
}

#[test]
fn geographic_features_use_the_existing_scene_columns_and_original_ids() {
    let mut session = open();
    let result = call(&mut session, SCENE, Status::Ok);
    expected(&result, "/result/unit/name", r#""METRE""#);
    expected(&result, "/result/map/crs_name", r#""EPSG:6677""#);
    expected(&result, "/result/map/scale", "1");
    expected(&result, "/result/lod", "2");
    expected(&result, "/result/counts/elements", "1");
    expected(&result, "/result/counts/instances", "1");
    expected(&result, "/result/counts/triangles", "2");
    expected(&result, "/result/extent/min", "[100,200,12]");
    expected(&result, "/result/extent/max", "[110,210,12]");
    let bytes = session.take(0).unwrap();
    let mut budget = Budget::default();
    let mut diag = Diagnostics::new();
    let cancel = CancelToken::new();
    let reader =
        zumen_scene3d::read(&bytes, &mut Cx::new(&mut budget, &mut diag, &cancel)).unwrap();
    let element = reader.element(0).unwrap();
    assert_eq!(element.id, "area");
    assert_eq!(element.class, "bldg:Building");
    assert_eq!(element.name, "面");
    assert_eq!(element.storey, None);
    assert_eq!(call(&mut session, SCENE, Status::Ok), result);
    assert_eq!(session.take(0).unwrap(), bytes);
}

#[test]
fn invalid_options_and_stops_leave_no_blob_and_keep_the_document() {
    let mut session = open();
    for extra in [
        r#""zone":0"#,
        r#""zone":20"#,
        r#""lod":5"#,
        r#""storeys":[]"#,
        r#""tolerance":0"#,
        r#""box":{"min":[1,1,1],"max":[0,0,0]}"#,
        r#""budget":{"work":1}"#,
        r#""budget":{"decoded_bytes":1}"#,
        r#""budget":{"segments":1}"#,
    ] {
        let result = call(
            &mut session,
            &format!(r#"{{"v":1,"op":"scene3d","doc":"geo.gml",{extra}}}"#),
            Status::Failed,
        );
        expected(&result, "/blobs", "[]");
        assert!(session.take(0).is_none());
    }
    let result = call(&mut session, SCENE, Status::Ok);
    expected(&result, "/result/counts/triangles", "2");
    let result = call(
        &mut session,
        r#"{"v":1,"op":"scene3d","doc":"geo.gml","ids":[]}"#,
        Status::Ok,
    );
    expected(&result, "/result/counts/elements", "0");
    expected(&result, "/result/counts/triangles", "0");
}
