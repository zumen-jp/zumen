//! 壁芯・内法の推定、指定優先、換算、材料の根拠と判定できない入力を確かめる。
#![cfg(feature = "measure")]

#[path = "support/evaluation.rs"]
#[allow(dead_code, reason = "数量の試験に使う自作入力を共有する")]
mod fixtures;
#[path = "support/measure.rs"]
mod input;

use fixtures::with;
use input::{options, request, room};
use zumen_base::{Budget, CancelToken, Cx, Diagnostics, Kind, Limit, Limits, codes};
use zumen_building::measure::{self, Boundary, BoundaryEvidence, InputBoundary, Method, WallBasis};
use zumen_building::{ElementRef, Local, Opening, Placement, Transform};
use zumen_geom3d::Failure;

#[test]
fn inside_and_center_inputs_are_inferred_and_converted_in_both_directions() {
    for inside in [false, true] {
        let (b, space, walls) = room(inside, Some(65.0), false);
        let before = b.clone();
        for boundary in [Boundary::Inside, Boundary::WallCenter] {
            let out = with(|cx| {
                measure::areas(
                    &b,
                    &[space],
                    request(&walls, boundary, InputBoundary::Auto),
                    options(),
                    cx,
                )
            })
            .unwrap();
            let expected = if boundary == Boundary::Inside {
                10.64
            } else {
                12.0
            };
            assert!((out.raw_total.unwrap() - expected).abs() < 1e-10, "{out:?}");
            let decision = out.measurements[0].boundary.as_ref().unwrap();
            assert_eq!(
                decision.source,
                if inside {
                    Boundary::Inside
                } else {
                    Boundary::WallCenter
                }
            );
            assert_eq!(decision.evidence, BoundaryEvidence::Inferred);
            assert_eq!(decision.vertices, 4);
            assert_eq!(decision.walls.len(), 4);
            assert_eq!(decision.center_vertices, if inside { 0 } else { 4 });
            assert_eq!(decision.inside_vertices, if inside { 4 } else { 0 });
            assert!(
                decision
                    .walls
                    .iter()
                    .all(|w| w.basis == WallBasis::AxisAndShape)
            );
            assert!(matches!(out.method,Method::Area {walls:ids,..} if ids.len()==4));
        }
        assert_eq!(b, before);
    }
}

#[test]
fn explicit_interpretation_overrides_distance_and_shape_never_infers() {
    let (b, space, walls) = room(false, Some(0.0), false);
    let out = with(|cx| {
        measure::areas(
            &b,
            &[space],
            request(&walls, Boundary::WallCenter, InputBoundary::Inside),
            options(),
            cx,
        )
    })
    .unwrap();
    assert!((out.raw_total.unwrap() - 13.44).abs() < 1e-10);
    assert_eq!(
        out.measurements[0].boundary.as_ref().unwrap().evidence,
        BoundaryEvidence::Explicit
    );
    for (boundary, input, source, evidence) in [
        (
            Boundary::Inside,
            InputBoundary::Inside,
            Boundary::Inside,
            BoundaryEvidence::Explicit,
        ),
        (
            Boundary::WallCenter,
            InputBoundary::WallCenter,
            Boundary::WallCenter,
            BoundaryEvidence::Explicit,
        ),
        (
            Boundary::Shape,
            InputBoundary::Auto,
            Boundary::Shape,
            BoundaryEvidence::Shape,
        ),
    ] {
        let out =
            with(|cx| measure::areas(&b, &[space], request(&[], boundary, input), options(), cx))
                .unwrap();
        assert!((out.raw_total.unwrap() - 12.0).abs() < 1e-10);
        let decision = out.measurements[0].boundary.as_ref().unwrap();
        assert_eq!((decision.source, decision.evidence), (source, evidence));
        assert!(decision.walls.is_empty());
    }
}

#[test]
fn material_direction_and_verified_thickness_work_without_axis() {
    let (mut b, space, walls) = room(true, None, true);
    // 型が持つ層も使う。
    let kind = with(|cx| b.push(None, 99, cx)).unwrap();
    let material = b.components.material.remove(walls[0]).unwrap();
    b.components.material.insert(kind, material);
    b.components
        .r#type
        .insert(walls[0], zumen_building::TypeOf { target: kind });
    let out = with(|cx| {
        measure::areas(
            &b,
            &[space],
            request(&walls, Boundary::WallCenter, InputBoundary::Auto),
            options(),
            cx,
        )
    })
    .unwrap();
    assert!((out.raw_total.unwrap() - 12.0).abs() < 1e-10, "{out:?}");
    assert!(
        out.measurements[0]
            .boundary
            .as_ref()
            .unwrap()
            .walls
            .iter()
            .all(|w| w.basis == WallBasis::LayersAndShape)
    );
}

#[test]
fn inside_hole_shrinks_when_outer_boundary_expands_to_wall_centers() {
    let (mut b, space, mut walls) = room(true, Some(0.0), false);
    let opening = fixtures::add(
        &mut b,
        fixtures::data(
            vec![fixtures::cuboid(
                [1400.0, 900.0, -100.0],
                [2600.0, 2100.0, 4000.0],
            )],
            &[0],
            "Reference",
        ),
        "IfcOpeningElement",
    );
    b.components.opening.insert(opening, Opening::Voids(space));
    for transform in [
        Transform::translation(1500.0, 1000.0, 0.0),
        Transform::translation(1500.0, 2000.0, 0.0),
        Transform([
            [0.0, -1.0, 0.0, 1500.0],
            [1.0, 0.0, 0.0, 1000.0],
            [0.0, 0.0, 1.0, 0.0],
        ]),
        Transform([
            [0.0, -1.0, 0.0, 2500.0],
            [1.0, 0.0, 0.0, 1000.0],
            [0.0, 0.0, 1.0, 0.0],
        ]),
    ] {
        let wall = fixtures::add(
            &mut b,
            input::wall_data(1000.0, Some(0.0), 200.0),
            "IfcWall",
        );
        b.components.placement.insert(
            wall,
            Placement {
                relative_to: None,
                local: Local::Transform(transform),
            },
        );
        walls.push(wall);
    }
    let out = with(|cx| {
        measure::areas(
            &b,
            &[space],
            request(&walls, Boundary::WallCenter, InputBoundary::Auto),
            options(),
            cx,
        )
    })
    .unwrap();
    assert!((out.raw_total.unwrap() - 11.0).abs() < 1e-10, "{out:?}");
    let decision = out.measurements[0].boundary.as_ref().unwrap();
    assert_eq!(decision.vertices, 8);
    assert_eq!(decision.inside_vertices, 8);
    assert_eq!(decision.walls.len(), 8);
}

#[test]
fn openings_in_walls_do_not_cut_the_centerline_and_rotations_keep_area() {
    let (mut b, space, walls) = room(true, Some(0.0), false);
    let opening = fixtures::add(
        &mut b,
        fixtures::data(
            vec![fixtures::cuboid(
                [500.0, -200.0, -100.0],
                [1500.0, 200.0, 2200.0],
            )],
            &[0],
            "Reference",
        ),
        "IfcOpeningElement",
    );
    b.components
        .opening
        .insert(opening, Opening::Voids(walls[0]));
    // 3-4-5 の回転。壁と室を同じ配置で移す。
    let rotation = Transform([
        [0.6, -0.8, 0.0, 100000.0],
        [0.8, 0.6, 0.0, -200000.0],
        [0.0, 0.0, 1.0, 0.0],
    ]);
    for at in (0..b.elements().len()).map(|n| ElementRef(n as u32)) {
        let local = match b.components.placement.get(at) {
            Some(Placement {
                local: Local::Transform(t),
                ..
            }) => *t,
            _ => Transform::IDENTITY,
        };
        b.components.placement.insert(
            at,
            Placement {
                relative_to: None,
                local: Local::Transform(rotation.then(&local)),
            },
        );
    }
    let mut req = request(&walls, Boundary::WallCenter, InputBoundary::Auto);
    req.origin.x = 100000.0;
    req.origin.y = -200000.0;
    let out = with(|cx| measure::areas(&b, &[space], req, options(), cx)).unwrap();
    assert!((out.raw_total.unwrap() - 12.0).abs() < 1e-6, "{out:?}");
}

#[test]
fn missing_or_conflicting_wall_evidence_never_becomes_a_guessed_quantity() {
    for layers in [false, true] {
        let (mut b, space, walls) = room(true, None, layers);
        if layers {
            for &wall in &walls {
                b.components.material.insert(wall, input::material(300.0));
            }
        }
        let mut diagnostics = Diagnostics::new();
        let out = measure::areas(
            &b,
            &[space],
            request(&walls, Boundary::WallCenter, InputBoundary::Auto),
            options(),
            &mut Cx::new(
                &mut Budget::default(),
                &mut diagnostics,
                &CancelToken::new(),
            ),
        )
        .unwrap();
        assert!(out.raw_total.is_none());
        assert!(out.measurements[0].raw.is_none());
        assert!(
            diagnostics
                .into_vec()
                .iter()
                .any(|d| d.code() == codes::BUILDING_MEASURE_FAILED)
        );
    }
    let (mut b, space, walls) = room(false, Some(0.0), false);
    // 1 mm の壁では壁芯・内法の両方の多数条件に入るので、推定を採用しない。
    for (i, &wall) in walls.iter().enumerate() {
        input::replace_wall(
            &mut b,
            wall,
            input::wall_data(if i < 2 { 4000.0 } else { 3000.0 }, Some(0.0), 1.0),
        );
    }
    let out = with(|cx| {
        measure::areas(
            &b,
            &[space],
            request(&walls, Boundary::WallCenter, InputBoundary::Auto),
            options(),
            cx,
        )
    })
    .unwrap();
    assert!(out.raw_total.is_none());
}

#[test]
fn boundary_conversion_obeys_limits_and_cancel_without_partial_results() {
    let (b, space, walls) = room(true, Some(0.0), false);
    for kind in [Kind::Work, Kind::DecodedBytes, Kind::Segments, Kind::Depth] {
        let mut limits = Limits::default();
        limits.set(kind, Limit::Finite(0));
        let out = measure::areas(
            &b,
            &[space],
            request(&walls, Boundary::WallCenter, InputBoundary::Auto),
            options(),
            &mut Cx::new(
                &mut Budget::new(limits),
                &mut Diagnostics::new(),
                &CancelToken::new(),
            ),
        );
        assert!(matches!(out, Err(Failure::Stopped(_))), "{kind:?}: {out:?}");
    }
    let cancel = CancelToken::new();
    cancel.cancel();
    assert!(matches!(
        measure::areas(
            &b,
            &[space],
            request(&walls, Boundary::WallCenter, InputBoundary::Auto),
            options(),
            &mut Cx::new(&mut Budget::default(), &mut Diagnostics::new(), &cancel)
        ),
        Err(Failure::Stopped(_))
    ));
}
