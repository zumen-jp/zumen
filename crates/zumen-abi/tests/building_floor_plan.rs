//! 共通 API で切った 2FL の実寸の線・用紙・出所を、既知の高さで切った Rust の図面と照合する。
#![cfg(feature = "bim")]

use zumen_abi1::{Session, Status};
use zumen_base::{Budget, CancelToken, Cx, Diagnostics, Kind, Limit, Limits, codes};
use zumen_building::drawing::{self, Options, Paper, View};
use zumen_crs::ProjectPoint;
use zumen_drawing::{PaperLength, binary};
use zumen_ifc::Ifc;
use zumen_sdk::Codec;

#[test]
fn public_storey_id_reproduces_the_reference_plan_after_cancellation() {
    let bytes = include_bytes!("../../../samples/ifc/rinya/wood-office-architecture.ifc");
    let mut session = Session::new(b"").unwrap();
    let budget = r#""max_places":1000,"budget":{"decoded_bytes":17179869184,"work":10000000000}"#;
    session.push(b"building", bytes).unwrap();
    let open = format!(r#"{{"v":1,"op":"open","input":"building",{budget}}}"#);
    let cancel = CancelToken::new();
    assert_eq!(session.call(open.as_bytes(), &cancel).status, Status::Ok);
    let plan = format!(
        r#"{{"v":1,"op":"building_drawing","doc":"building","to":"plan","storey":"22re20MQ16jwdWDv3FKVw8","unit":"meter","height":1.2,"depth":2,"scale":100,"tolerance":0.00001,"chord":0.001,{budget}}}"#
    );
    for (bad, pointer) in [
        (
            plan.replace("22re20MQ16jwdWDv3FKVw8", "0000000000000000000000"),
            "/storey",
        ),
        (plan.replace("\"meter\"", "\"unknown\""), "/unit"),
    ] {
        let failure = session.call(bad.as_bytes(), &cancel);
        assert_eq!(failure.status, Status::Failed);
        assert_eq!(
            zumen_json::parse(&failure.response)
                .unwrap()
                .pointer("/diagnostics/0/params/pointer"),
            Some(&zumen_json::Value::String(pointer.into()))
        );
        assert!(session.take(0).is_none());
    }
    let interrupted = CancelToken::new();
    interrupted.cancel();
    assert_eq!(
        session.call(plan.as_bytes(), &interrupted).status,
        Status::Failed
    );
    assert!(session.take(0).is_none());
    let reply = session.call(plan.as_bytes(), &cancel);
    assert_eq!(
        reply.status,
        Status::Ok,
        "{}",
        String::from_utf8_lossy(&reply.response)
    );
    let json = zumen_json::parse(&reply.response).unwrap();
    let number = |path| match json.pointer(path) {
        Some(zumen_json::Value::Number(n)) => n.to_safe_integer().unwrap(),
        _ => panic!("応答に {path} がない"),
    };
    assert_eq!(number("/result/lines"), 6683);
    assert_eq!(number("/result/collapsed"), 549);
    assert_eq!(number("/result/evaluation_failures"), 0);
    assert_eq!(number("/result/projection_failures"), 0);

    let mut budget = Budget::new(
        Limits::default()
            .with(Kind::DecodedBytes, Limit::Finite(16 * 1024 * 1024 * 1024))
            .with(Kind::Work, Limit::Finite(10_000_000_000)),
    );
    let mut diagnostics = Diagnostics::with_max_places(1000);
    let mut cx = Cx::new(&mut budget, &mut diagnostics, &cancel);
    let (_, actual) = binary::read(&session.take(0).unwrap(), &mut cx).unwrap();
    let building = Ifc::read(bytes, &mut cx)
        .unwrap()
        .lift_geometry(&mut cx)
        .unwrap();
    let expected = drawing::cut(
        &building,
        Options {
            view: View::Plan {
                floor: ProjectPoint {
                    x: 0.,
                    y: 0.,
                    z: 4300.,
                },
                height: 1200.,
                depth: 2000.,
            },
            tolerance: 0.01,
            chord: 1.,
            paper: Paper {
                denominator: 100.,
                origin: [PaperLength(0.); 2],
                size: None,
            },
            symbols: &[],
        },
        &mut cx,
    )
    .unwrap();
    assert!(
        actual == expected.drawing,
        "各線の座標・出所、または用紙が異なる"
    );
    let dropped = diagnostics
        .iter()
        .find(|d| d.code() == codes::BUILDING_LINE_DROPPED)
        .unwrap();
    let Some(zumen_json::Value::Array(items)) = json.pointer("/diagnostics") else {
        panic!("切り出しの診断がない")
    };
    let code = zumen_json::Value::String("building.line.dropped".into());
    let item = items
        .iter()
        .find(|item| item.pointer("/code") == Some(&code))
        .unwrap();
    let Some(zumen_json::Value::Array(places)) = item.pointer("/at") else {
        panic!("縮んだ区間の場所がない")
    };
    let mut wire_places: Vec<_> = places
        .iter()
        .map(|p| {
            let Some(zumen_json::Value::String(id)) = p.pointer("/id") else {
                panic!("出所を特定できない")
            };
            id.as_str()
        })
        .collect();
    let mut rust_places: Vec<_> = dropped.places().iter().map(|p| p.id().unwrap()).collect();
    wire_places.sort_unstable();
    rust_places.sort_unstable();
    assert_eq!(wire_places, rust_places);
}
