//! civil の op は、線形の上の所を求めるために区間と不連続をたどる作業を、予算に数える（#560）。

#![cfg(feature = "civil")]

use zumen_abi1::{Session, Status};
use zumen_base::CancelToken;

fn call(session: &mut Session, request: &str) -> (Status, String) {
    let reply = session.call(request.as_bytes(), &CancelToken::new());
    (
        reply.status,
        String::from_utf8(reply.response).unwrap_or_default(),
    )
}

/// 北へ 0.1 m の直線を 1000 個並べた線形の上の所を、`ask` で求める要求。`budget` は要求の後ろに足す
fn request(ask: &str, budget: &str) -> String {
    let horizontal: Vec<String> = (0..1000)
        .map(|i| {
            format!(
                r#"{{"kind":"line","northing":{},"easting":0,"direction":0,"length":0.1}}"#,
                0.1 * f64::from(i)
            )
        })
        .collect();
    format!(
        r#"{{"v":1,"op":"civil","alignment":{{"horizontal":[{}]}},{ask}{budget}}}"#,
        horizontal.join(",")
    )
}

#[test]
fn positions_count_the_work_of_walking_the_segments() {
    let mut session = Session::new(b"").unwrap();
    // 0.1 m ごとの所は 1001 個で、所ごとに区間を 2 × 1000 回ほどたどる。作業は 200 万ほどになる
    let (status, response) = call(&mut session, &request(r#""interval":0.1"#, ""));
    assert_eq!(status, Status::Ok, "{response}");
    let (status, response) = call(
        &mut session,
        &request(r#""interval":0.1"#, r#","budget":{"work":1000000}"#),
    );
    assert_eq!(status, Status::Failed, "{response}");
    assert!(
        response.contains(r#""code":"budget.work.exceeded""#),
        "{response}"
    );
    // 測点の表記ごとにも、平面線形の区間をたどる
    let stations: Vec<String> = (0..1000).map(|_| r#""No.1""#.to_string()).collect();
    let ask = format!(r#""stations":[{}]"#, stations.join(","));
    let (status, response) = call(&mut session, &request(&ask, r#","budget":{"work":500000}"#));
    assert_eq!(status, Status::Failed, "{response}");
    assert!(
        response.contains(r#""code":"budget.work.exceeded""#),
        "{response}"
    );
}
