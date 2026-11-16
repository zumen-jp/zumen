//! 解析的な期待値・失敗と予算・形の手当てを、採用したカーネルで確かめる。

use zumen_base::{Budget, CancelToken, Cx, Diagnostics, Kind, Limit, Limits, Stop, codes};
use zumen_geom3d::{
    BooleanOp, Curve, Face, Failure, Kernel, ManifoldKernel, Mesh, Plane, Profile, Shape,
    Tolerance, Transform,
    cases::{self, Expect},
};

const TOL: Tolerance = Tolerance {
    chord: 1e-3,
    length: 1e-9,
};

fn run<T>(f: impl FnOnce(&mut Cx) -> T) -> T {
    f(&mut Cx::new(
        &mut Budget::default(),
        &mut Diagnostics::new(),
        &CancelToken::new(),
    ))
}

fn boolean(op: BooleanOp, a: Shape, b: Shape) -> Shape {
    Shape::Boolean {
        op,
        first: Box::new(a),
        second: Box::new(b),
    }
}

#[test]
fn analytical_cases_with_the_selected_kernel() {
    for case in cases::all() {
        let result = run(|cx| ManifoldKernel.evaluate(&case.shape, TOL, cx));
        match (case.expect, result) {
            (
                Expect::Solid {
                    volume,
                    relative,
                    bounds,
                },
                Ok(mesh),
            ) => {
                assert!(
                    run(|cx| ManifoldKernel.is_solid(&mesh, cx)).unwrap(),
                    "{}",
                    case.name
                );
                assert!(
                    (mesh.volume() - volume).abs() <= volume * relative,
                    "{} volume={} want={volume}",
                    case.name,
                    mesh.volume()
                );
                let found = mesh.bounds().unwrap();
                for (a, b) in found.iter().flatten().zip(bounds.iter().flatten()) {
                    assert!(
                        (a - b).abs() <= TOL.chord * 1.1,
                        "{} bounds={found:?} want={bounds:?}",
                        case.name
                    );
                }
                assert!(!mesh.edges.is_empty(), "{}", case.name);
            }
            (Expect::Invalid, Err(Failure::Invalid(_))) | (Expect::Rejected, Err(_)) => {}
            (expected, result) => panic!("{}: {expected:?} {result:?}", case.name),
        }
    }
}

#[test]
fn swept_disks_follow_bent_paths_and_reject_cusps() {
    for inner_radius in [None, Some(0.1)] {
        let shape = Shape::SweptDisk {
            directrix: vec![[0.0; 3], [0.0, 0.0, 2.0], [3.0, 0.0, 2.0]],
            radius: 0.2,
            inner_radius,
        };
        let out = run(|cx| ManifoldKernel.evaluate(&shape, TOL, cx)).unwrap();
        assert!(run(|cx| ManifoldKernel.is_solid(&out, cx)).unwrap());
        let inner = inner_radius.unwrap_or(0.0);
        let expected = core::f64::consts::PI * (0.04 - inner * inner) * 5.0;
        assert!(
            (out.volume() - expected).abs() < 0.01 * expected,
            "{} != {expected}",
            out.volume()
        );
    }
    let cusp = Shape::SweptDisk {
        directrix: vec![[0.0; 3], [0.0, 0.0, 2.0], [0.0, 0.0, 1.0]],
        radius: 0.2,
        inner_radius: None,
    };
    assert!(matches!(
        run(|cx| ManifoldKernel.evaluate(&cusp, TOL, cx)),
        Err(Failure::Invalid("sweep_corner"))
    ));
}

#[test]
fn half_space_box_covers_a_mesh_far_from_the_plane_origin() {
    let a = cases::cuboid([1e6, 2e6, 0.0], [1e6 + 4.0, 2e6 + 3.0, 2.0]);
    for (op, want) in [
        (BooleanOp::Difference, 12.0),
        (BooleanOp::Intersection, 12.0),
    ] {
        let shape = boolean(
            op,
            a.clone(),
            Shape::HalfSpace {
                plane: Plane {
                    origin: [0.0, 0.0, 1.0],
                    normal: [0.0, 0.0, 1.0],
                },
            },
        );
        let mesh = run(|cx| ManifoldKernel.evaluate(&shape, TOL, cx)).unwrap();
        assert!((mesh.volume() - want).abs() < 1e-6, "{}", mesh.volume());
        assert!(mesh.is_closed());
    }
}

#[test]
fn repeated_openings_are_snapped_to_the_same_local_grid() {
    let mut wall = cases::cuboid([0.0; 3], [8.0, 0.2, 3.0]);
    for x in [1.0, 3.0, 5.0] {
        wall = boolean(
            BooleanOp::Difference,
            wall,
            cases::cuboid([x, -1.6e-16, 0.0], [x + 1.0, 0.2 + 1.6e-16, 2.0]),
        );
    }
    let out = run(|cx| ManifoldKernel.evaluate_with_changes(&wall, TOL, cx)).unwrap();
    assert!((out.mesh.volume() - 3.6).abs() < 1e-10);
    assert!(out.mesh.is_closed());
    assert!(out.changes.snapped_vertices > 0);
    let again = run(|cx| ManifoldKernel.evaluate_with_changes(&wall, TOL, cx)).unwrap();
    assert_eq!(out, again);
}

#[test]
fn extrusion_keeps_boundary_vertices_that_earcut_would_remove() {
    let shape = Shape::Extrusion {
        profile: Profile {
            outer: Curve::Polygon(vec![
                [0.0, 0.0],
                [1.0, 0.0],
                [2.0, 0.0],
                [2.0, 2.0],
                [0.0, 2.0],
            ]),
            holes: vec![],
        },
        position: Transform::IDENTITY,
        direction: [0.0, 0.0, 1.0],
        depth: 2.0,
    };
    let out = run(|cx| ManifoldKernel.evaluate_with_changes(&shape, TOL, cx)).unwrap();
    assert_eq!(out.mesh.volume(), 8.0);
    assert!(out.mesh.is_closed());
}

#[test]
fn faces_meeting_at_a_t_junction_share_the_subdivided_edge() {
    let cube = cases::all()
        .into_iter()
        .find(|c| c.name == "face_set_cube")
        .unwrap();
    let Shape::FaceSet { mut faces, closed } = cube.shape else {
        panic!()
    };
    faces[1].outer.insert(1, [0.5, 0.0, 1.0]);
    let out =
        run(|cx| ManifoldKernel.evaluate_with_changes(&Shape::FaceSet { faces, closed }, TOL, cx))
            .unwrap();
    assert_eq!(out.mesh.volume(), 1.0);
    assert!(out.mesh.is_closed());
    assert!(out.changes.split_edges > 0);
}

#[test]
fn degenerate_faces_are_counted_and_do_not_remove_the_rest() {
    let cube = cases::all()
        .into_iter()
        .find(|c| c.name == "face_set_cube")
        .unwrap();
    let Shape::FaceSet { mut faces, closed } = cube.shape else {
        panic!()
    };
    faces.push(Face {
        outer: vec![[0.0; 3], [1.0, 0.0, 0.0], [2.0, 0.0, 0.0]],
        holes: vec![],
    });
    let shape = Shape::FaceSet { faces, closed };
    let mut budget = Budget::default();
    let mut diag = Diagnostics::new();
    let cancel = CancelToken::new();
    let mesh = ManifoldKernel
        .evaluate(&shape, TOL, &mut Cx::new(&mut budget, &mut diag, &cancel))
        .unwrap();
    assert_eq!(mesh.volume(), 1.0);
    assert!(mesh.is_closed());
    let entries = diag.into_vec();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].code(), codes::GEOM3D_FACE_DROPPED);
    assert_eq!(entries[0].count(), 1);
}

#[test]
fn touching_solids_are_closed_but_flat_and_open_inputs_are_rejected() {
    let shape = Shape::Group(vec![
        cases::cuboid([0.0; 3], [1.0; 3]),
        cases::cuboid([1.0, 1.0, 0.0], [2.0, 2.0, 1.0]),
    ]);
    let mesh = run(|cx| ManifoldKernel.evaluate(&shape, TOL, cx)).unwrap();
    assert!(!mesh.is_closed());
    assert!(run(|cx| ManifoldKernel.is_solid(&mesh, cx)).unwrap());
    let clip = run(|cx| {
        ManifoldKernel.evaluate(&cases::cuboid([0.0, 0.0, -1.0], [2.0, 2.0, 0.5]), TOL, cx)
    })
    .unwrap();
    let cut =
        run(|cx| ManifoldKernel.boolean(BooleanOp::Difference, &mesh, &clip, TOL, cx)).unwrap();
    assert!((cut.mesh.volume() - 1.0).abs() < 1e-10);
    for triangles in [vec![[0, 1, 2]], vec![[0, 1, 2], [2, 1, 0]]] {
        let flat = Mesh {
            positions: vec![[0.0; 3], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
            triangles,
            edges: vec![],
        };
        assert!(!run(|cx| ManifoldKernel.is_solid(&flat, cx)).unwrap());
        assert!(matches!(
            run(|cx| ManifoldKernel.boolean(BooleanOp::Difference, &mesh, &flat, TOL, cx)),
            Err(Failure::Invalid(_))
        ));
    }
}

#[test]
fn opposite_faces_with_rounding_residue_do_not_count_as_volume() {
    let p0 = [1.1, 2.2, 3.3];
    let p1 = [2.4, 2.7, 4.9];
    let p3 = [2.9, 4.5, 3.7];
    let p2 = std::array::from_fn(|i| p1[i] + p3[i] - p0[i]);
    let flat = Mesh {
        positions: vec![p0, p1, p2, p3],
        triangles: vec![[0, 1, 2], [0, 2, 3], [0, 3, 1], [1, 3, 2]],
        edges: vec![],
    };
    assert!(flat.is_closed());
    assert!(!run(|cx| ManifoldKernel.is_solid(&flat, cx)).unwrap());
    let cube =
        run(|cx| ManifoldKernel.evaluate(&cases::cuboid([0.0; 3], [10.0; 3]), TOL, cx)).unwrap();
    assert!(matches!(
        run(|cx| ManifoldKernel.boolean(BooleanOp::Difference, &cube, &flat, TOL, cx)),
        Err(Failure::Invalid(_))
    ));
}

#[test]
fn stop_is_checked_before_allocations_and_recursion_is_balanced() {
    let cube = cases::cuboid([0.0; 3], [1.0; 3]);
    let shape = boolean(BooleanOp::Difference, cube.clone(), cube);
    for kind in [Kind::DecodedBytes, Kind::Segments, Kind::Work, Kind::Depth] {
        let mut budget = Budget::new(Limits::default().with(kind, Limit::Finite(0)));
        let result = ManifoldKernel.evaluate(
            &shape,
            TOL,
            &mut Cx::new(&mut budget, &mut Diagnostics::new(), &CancelToken::new()),
        );
        assert!(matches!(result,Err(Failure::Stopped(Stop::Exceeded(e))) if e.kind==kind));
        assert_eq!(budget.used(Kind::Depth), 0);
    }
    let cancel = CancelToken::new();
    cancel.cancel();
    let result = ManifoldKernel.evaluate(
        &shape,
        TOL,
        &mut Cx::new(&mut Budget::default(), &mut Diagnostics::new(), &cancel),
    );
    assert_eq!(result, Err(Failure::Stopped(Stop::Cancelled)));
}

#[test]
fn invalid_indices_and_non_finite_points_do_not_reach_the_dependency() {
    for (positions, triangles) in [
        (vec![[0.0; 3]], vec![[0, 1, u32::MAX]]),
        (vec![[f64::NAN, 0.0, 0.0]], vec![[0; 3]]),
    ] {
        let shape = Shape::Triangles {
            positions,
            triangles,
            closed: true,
        };
        assert!(matches!(
            run(|cx| ManifoldKernel.evaluate(&shape, TOL, cx)),
            Err(Failure::Invalid(_))
        ));
    }
    let huge = Shape::Extrusion {
        profile: Profile {
            outer: Curve::Circle {
                center: [0.0; 2],
                radius: 1e300,
            },
            holes: vec![],
        },
        position: Transform::IDENTITY,
        direction: [0.0, 0.0, 1.0],
        depth: 1.0,
    };
    assert!(matches!(
        run(|cx| ManifoldKernel.evaluate(&huge, TOL, cx)),
        Err(Failure::Invalid(_))
    ));
}
