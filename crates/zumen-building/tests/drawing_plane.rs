//! 平断面と鉛直面の座標、見る側、巨大座標と不正値の検査。
#![cfg(feature = "drawing")]

use zumen_building::drawing::{Side, View};
use zumen_crs::ProjectPoint;

fn point(x: f64, y: f64, z: f64) -> ProjectPoint {
    ProjectPoint { x, y, z }
}

#[test]
fn plan_preserves_project_units_and_large_origin() {
    let floor = point(1e12, -1e12, 12000.0);
    let frame = View::Plan {
        floor,
        height: 1200.0,
        depth: 2500.0,
    }
    .frame()
    .unwrap();
    let p = point(floor.x + 2.5, floor.y - 4.0, 13000.0);
    assert_eq!(frame.project(p).unwrap(), [2.5, -4.0, -200.0]);
    assert_eq!(frame.unproject([2.5, -4.0, -200.0]).unwrap(), p);
    assert_eq!(frame.depth(), 2500.0);
    assert!(frame.is_section());
}

#[test]
fn vertical_side_reverses_right_and_depth_but_keeps_up() {
    let start = point(100.0, 200.0, 300.0);
    let end = point(104.0, 203.0, 300.0);
    let p = point(104.0, 203.0, 307.0);
    for (side, sign) in [(Side::Right, 1.0), (Side::Left, -1.0)] {
        let frame = View::Vertical {
            start,
            end,
            side,
            depth: 1000.0,
            section: false,
        }
        .frame()
        .unwrap();
        assert!(
            frame
                .project(p)
                .unwrap()
                .into_iter()
                .zip([5.0 * sign, 7.0, 0.0])
                .all(|(a, b)| (a - b).abs() < 1e-12)
        );
        assert_eq!(frame.unproject([5.0 * sign, 7.0, 0.0]).unwrap(), p);
        assert!(!frame.is_section());
        let [right, up, toward] = frame.axes();
        assert_eq!(zumen_geom3d::cross(right, up), toward);
    }
}

#[test]
fn invalid_planes_and_points_are_rejected() {
    for depth in [0.0, -1.0, f64::INFINITY, f64::NAN] {
        assert!(
            View::Plan {
                floor: point(0.0, 0.0, 0.0),
                height: 1.0,
                depth
            }
            .frame()
            .is_err()
        );
    }
    for end in [
        point(0.0, 0.0, 0.0),
        point(1.0, 0.0, 1.0),
        point(f64::INFINITY, 0.0, 0.0),
    ] {
        assert!(
            View::Vertical {
                start: point(0.0, 0.0, 0.0),
                end,
                side: Side::Right,
                depth: 1.0,
                section: true
            }
            .frame()
            .is_err()
        );
    }
    let frame = View::Plan {
        floor: point(0.0, 0.0, 0.0),
        height: 1.0,
        depth: 2.0,
    }
    .frame()
    .unwrap();
    assert!(frame.project(point(f64::NAN, 0.0, 0.0)).is_err());
    assert!(frame.unproject([0.0, f64::INFINITY, 0.0]).is_err());
}
