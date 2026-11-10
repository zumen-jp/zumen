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
    // 測点の表記ごとにも、不連続と平面線形の区間をたどる。線形の外の表記は追加距離を返さないので、表記ごとの
    // 作業だけで止まる
    let stations: Vec<String> = (0..1000).map(|_| r#""No.999""#.to_string()).collect();
    let ask = format!(r#""stations":[{}]"#, stations.join(","));
    let (status, response) = call(&mut session, &request(&ask, r#","budget":{"work":500000}"#));
    assert_eq!(status, Status::Failed, "{response}");
    assert!(
        response.contains(r#""code":"budget.work.exceeded""#),
        "{response}"
    );
}

/// 曲率の大きい緩和曲線の上の所を求める手間（向きの式を積む刻み）も予算に数え、ほかの予算の停止と同じく失敗に
/// する（#727）。区間の初めの点を確かめる手間（`check_within`）も数える
#[test]
fn positions_count_the_work_of_steep_transitions() {
    let mut session = Session::new(b"").unwrap();
    let steep = r#"{"kind":"transition","northing":0,"easting":0,"direction":0,"length":1000,"end_radius":0.01}"#;
    let line = r#"{"kind":"line","northing":0,"easting":0,"direction":0,"length":1}"#;
    let request = |horizontal: &str, budget: &str| {
        format!(
            r#"{{"v":1,"op":"civil","alignment":{{"horizontal":[{horizontal}]}},"distances":[500,1000]{budget}}}"#
        )
    };
    // 所を求めず、区間の初めの点を確かめるだけの要求
    let checked = |horizontal: &str, budget: &str| {
        format!(
            r#"{{"v":1,"op":"civil","alignment":{{"horizontal":[{horizontal}]}},"distances":[]{budget}}}"#
        )
    };
    let (status, response) = call(&mut session, &request(steep, ""));
    assert_eq!(status, Status::Ok, "{response}");
    // 1 つの所に刻みの上限に近い手間を数えるので、2 つの所は 10 万の上限を超える
    let (status, response) = call(
        &mut session,
        &request(steep, r#","budget":{"work":100000}"#),
    );
    assert_eq!(status, Status::Failed, "{response}");
    assert!(
        response.contains(r#""code":"budget.work.exceeded""#),
        "{response}"
    );
    // 後ろに区間があれば、初めの点を確かめるときに、緩和曲線の終わりの点を求める手間を数える
    let both = format!("{steep},{line}");
    let (status, response) = call(&mut session, &checked(&both, ""));
    assert_eq!(status, Status::Ok, "{response}");
    let (status, response) = call(&mut session, &checked(&both, r#","budget":{"work":50000}"#));
    assert_eq!(status, Status::Failed, "{response}");
    assert!(
        response.contains(r#""code":"budget.work.exceeded""#),
        "{response}"
    );
}
