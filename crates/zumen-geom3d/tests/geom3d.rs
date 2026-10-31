//! 型と trait の試験

use zumen_base::{Budget, CancelToken, Cx, Diagnostics, Kind, Limit, Limits, Stop};
use zumen_geom3d::cases::{self, Expect};
use zumen_geom3d::{
    Failure, Kernel, Mesh, MeshSection, Plane, Section, Shape, Tolerance, Transform, Xy, Xyz,
    cross, dot, length,
};

fn with_cx<T>(limits: Limits, f: impl FnOnce(&mut Cx) -> T) -> T {
    let mut budget = Budget::new(limits);
    let mut diag = Diagnostics::new();
    let cancel = CancelToken::new();
    let mut cx = Cx::new(&mut budget, &mut diag, &cancel);
    f(&mut cx)
}

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() <= 1e-12 * (1.0 + a.abs().max(b.abs()))
}

/// 試験のためのカーネル。三角形と凸の面の集合・写像・組だけを評価し、ほかは評価しないと返す
struct Faces;

impl Kernel for Faces {
    fn name(&self) -> &'static str {
        "faces"
    }

    fn evaluate(&self, shape: &Shape, tolerance: Tolerance, cx: &mut Cx) -> Result<Mesh, Failure> {
        if tolerance.length.is_nan() || tolerance.length < 0.0 {
            return Err(Failure::Invalid("tolerance"));
        }
        let mesh = match shape {
            Shape::Triangles {
                positions,
                triangles,
                ..
            } => Mesh {
                positions: positions.clone(),
                triangles: triangles.clone(),
                edges: Vec::new(),
            },
            Shape::FaceSet { faces, .. } => {
                let mut mesh = Mesh::default();
                for face in faces {
                    if !face.holes.is_empty() {
                        return Err(Failure::Unsupported("face_set"));
                    }
                    let base = mesh.positions.len() as u32;
                    mesh.positions.extend_from_slice(&face.outer);
                    for i in 1..face.outer.len().saturating_sub(1) as u32 {
                        mesh.triangles.push([base, base + i, base + i + 1]);
                    }
                }
                mesh
            }
            Shape::Mapped { transform, shape } => {
                self.evaluate(shape, tolerance, cx)?.transformed(transform)
            }
            Shape::Group(shapes) => {
                let mut mesh = Mesh::default();
                for shape in shapes {
                    mesh.append(&self.evaluate(shape, tolerance, cx)?);
                }
                mesh
            }
            other => return Err(Failure::Unsupported(other.kind())),
        };
        mesh.validate()?;
        cx.check(Kind::Segments, mesh.triangles.len() as u64)?;
        Ok(mesh)
    }
}

fn unit_cube() -> Mesh {
    let p = [
        [0.0, 0.0, 0.0],
        [1.0, 0.0, 0.0],
        [1.0, 1.0, 0.0],
        [0.0, 1.0, 0.0],
        [0.0, 0.0, 1.0],
        [1.0, 0.0, 1.0],
        [1.0, 1.0, 1.0],
        [0.0, 1.0, 1.0],
    ];
    let quads = [
        [0, 3, 2, 1],
        [4, 5, 6, 7],
        [0, 1, 5, 4],
        [1, 2, 6, 5],
        [2, 3, 7, 6],
        [3, 0, 4, 7],
    ];
    Mesh {
        positions: p.to_vec(),
        triangles: quads
            .iter()
            .flat_map(|[a, b, c, d]| [[*a, *b, *c], [*a, *c, *d]])
            .collect(),
        edges: vec![[0, 1], [1, 2], [2, 3], [3, 0]],
    }
}

fn signed_area(ring: &[Xy]) -> f64 {
    let next = ring.iter().cycle().skip(1);
    ring.iter()
        .zip(next)
        .map(|(a, b)| a[0] * b[1] - b[0] * a[1])
        .sum::<f64>()
        / 2.0
}

#[test]
fn places_axes_orthonormal_and_composes() {
    let t = Transform::from_axes(
        [1.0, 2.0, 3.0],
        Some([0.0, 0.0, 2.0]),
        Some([1.0, 1.0, 5.0]),
    )
    .unwrap();
    let (x, y, z) = (t.column(0), t.column(1), t.column(2));
    assert!(close(length(x), 1.0) && close(length(y), 1.0) && close(length(z), 1.0));
    assert!(close(dot(x, y), 0.0) && close(dot(y, z), 0.0) && close(dot(z, x), 0.0));
    let xy = cross(x, y);
    assert!((0..3).all(|k| close(xy[k], z[k])));
    assert!(close(t.determinant(), 1.0));
    assert_eq!(t.point([0.0, 0.0, 0.0]), [1.0, 2.0, 3.0]);
    let moved = Transform::scaling([2.0, 2.0, 2.0]).then(&Transform::translation([1.0, 0.0, 0.0]));
    assert_eq!(moved.point([1.0, 1.0, 1.0]), [3.0, 2.0, 2.0]);
    assert_eq!(moved.vector([1.0, 1.0, 1.0]), [2.0, 2.0, 2.0]);
    assert!(Transform::from_axes([0.0; 3], Some([0.0; 3]), None).is_none());
    assert!(Transform::from_axes([f64::NAN, 0.0, 0.0], None, None).is_none());
    // 参照の向きが軸と平行なら、ほかの座標軸を使う
    assert!(Transform::from_axes([0.0; 3], Some([1.0, 0.0, 0.0]), Some([2.0, 0.0, 0.0])).is_some());
}

#[test]
fn measures_and_checks_meshes() {
    let cube = unit_cube();
    assert!(cube.validate().is_ok());
    assert!(close(cube.volume(), 1.0));
    assert!(close(cube.area(), 6.0));
    assert_eq!(cube.bounds(), Some([[0.0; 3], [1.0; 3]]));
    assert!(cube.is_closed());
    // 裏返す変換でも、外向きのまま
    let mirrored = cube.transformed(&Transform::scaling([-1.0, 2.0, 1.0]));
    assert!(close(mirrored.volume(), 2.0));
    assert!(mirrored.is_closed());
    // 点を分けたメッシュは、1 つにすると閉じる
    let mut split = Mesh::default();
    for t in &cube.triangles {
        let base = split.positions.len() as u32;
        split
            .positions
            .extend(t.iter().map(|i| cube.positions[*i as usize]));
        split.triangles.push([base, base + 1, base + 2]);
    }
    assert!(!split.is_closed());
    let welded = split.welded();
    assert_eq!(welded.positions.len(), 8);
    assert!(welded.is_closed());
    let mut two = cube.clone();
    two.append(&cube.transformed(&Transform::translation([2.0, 0.0, 0.0])));
    assert!(close(two.volume(), 2.0));
    assert_eq!(two.edges.len(), 8);
    assert!(two.is_closed());
    let mut broken = cube.clone();
    broken.triangles.push([0, 1, 99]);
    assert_eq!(broken.validate(), Err(Failure::Invalid("index")));
    broken.positions[0] = [f64::INFINITY, 0.0, 0.0];
    assert_eq!(broken.validate(), Err(Failure::Invalid("position")));
    assert!(!Mesh::default().is_closed());
    assert_eq!(Mesh::default().bounds(), None);
}

#[test]
fn cuts_closed_meshes_into_rings_with_material_on_the_left() {
    let cube = unit_cube();
    let plane = Plane {
        origin: [0.0, 0.0, 0.5],
        normal: [0.0, 0.0, 1.0],
    };
    let rings = with_cx(Limits::default(), |cx| {
        MeshSection.section(&cube, &plane, cx)
    })
    .unwrap();
    assert_eq!(rings.len(), 1, "{rings:?}");
    assert_eq!(rings[0].len(), 8, "三角形の斜めの辺の交点も持つ");
    assert!(close(signed_area(&rings[0]), 1.0));
    // 下から見ると、向きが逆になる
    let below = Plane {
        origin: [0.0, 0.0, 0.5],
        normal: [0.0, 0.0, -1.0],
    };
    let rings = with_cx(Limits::default(), |cx| {
        MeshSection.section(&cube, &below, cx)
    })
    .unwrap();
    assert!(close(signed_area(&rings[0]), 1.0));
    // 点を分けたメッシュでも、座標で輪になる
    let mut split = Mesh::default();
    for t in &cube.triangles {
        let base = split.positions.len() as u32;
        split
            .positions
            .extend(t.iter().map(|i| cube.positions[*i as usize]));
        split.triangles.push([base, base + 1, base + 2]);
    }
    let rings = with_cx(Limits::default(), |cx| {
        MeshSection.section(&split, &plane, cx)
    })
    .unwrap();
    assert_eq!(rings.len(), 1);
    // 平面の上の点は裏に寄せる。面と同じ平面で切っても、輪は 1 つ
    let bottom = Plane {
        origin: [0.0, 0.0, 0.0],
        normal: [0.0, 0.0, 1.0],
    };
    let rings = with_cx(Limits::default(), |cx| {
        MeshSection.section(&cube, &bottom, cx)
    })
    .unwrap();
    assert_eq!(rings.len(), 1);
    assert_eq!(
        rings[0].len(),
        4,
        "頂点を通る所で、同じ点を続けない: {rings:?}"
    );
    assert!(close(signed_area(&rings[0]), 1.0));
    // 平面が外れていれば、何もない
    let away = Plane {
        origin: [0.0, 0.0, 5.0],
        normal: [0.0, 0.0, 1.0],
    };
    assert!(
        with_cx(Limits::default(), |cx| MeshSection
            .section(&cube, &away, cx))
        .unwrap()
        .is_empty()
    );
}

#[test]
fn cuts_holes_clockwise_and_open_meshes_into_chains() {
    let tube = cases::all()
        .into_iter()
        .find(|c| c.name == "face_set_with_hole")
        .unwrap();
    let Shape::FaceSet { faces, .. } = tube.shape else {
        panic!()
    };
    // 側面だけを三角形にする（穴のある上下の面は切り口に効かない）
    let sides = Shape::FaceSet {
        faces: faces.into_iter().filter(|f| f.holes.is_empty()).collect(),
        closed: false,
    };
    let tolerance = Tolerance {
        chord: cases::CHORD,
        length: 1e-9,
    };
    let mesh = with_cx(Limits::default(), |cx| {
        Faces.evaluate(&sides, tolerance, cx)
    })
    .unwrap();
    let plane = Plane {
        origin: [0.0, 0.0, 0.5],
        normal: [0.0, 0.0, 1.0],
    };
    let rings = with_cx(Limits::default(), |cx| {
        MeshSection.section(&mesh, &plane, cx)
    })
    .unwrap();
    let mut areas: Vec<f64> = rings.iter().map(|r| signed_area(r)).collect();
    areas.sort_by(f64::total_cmp);
    assert_eq!(areas.len(), 2);
    assert!(close(areas[0], -1.0) && close(areas[1], 4.0), "{areas:?}");
    // 開いたメッシュは、開いた鎖にする
    let triangle = Mesh {
        positions: vec![[0.0, 0.0, 0.0], [1.0, 0.0, 1.0], [0.0, 1.0, 1.0]],
        triangles: vec![[0, 1, 2]],
        edges: Vec::new(),
    };
    let chains = with_cx(Limits::default(), |cx| {
        MeshSection.section(&triangle, &plane, cx)
    })
    .unwrap();
    assert_eq!(chains, vec![vec![[0.0, 0.5], [0.5, 0.0]]]);
}

#[test]
fn section_reports_bad_planes_and_counts_segments() {
    let cube = unit_cube();
    let flat = Plane {
        origin: [0.0; 3],
        normal: [0.0; 3],
    };
    assert_eq!(
        with_cx(Limits::default(), |cx| MeshSection
            .section(&cube, &flat, cx)),
        Err(Failure::Invalid("plane"))
    );
    let plane = Plane {
        origin: [0.0, 0.0, 0.5],
        normal: [0.0, 0.0, 1.0],
    };
    let limits = Limits::default().with(Kind::Segments, Limit::Finite(3));
    let result = with_cx(limits, |cx| MeshSection.section(&cube, &plane, cx));
    assert!(
        matches!(result, Err(Failure::Stopped(Stop::Exceeded(_)))),
        "{result:?}"
    );
}

#[test]
fn cases_have_unique_names_and_consistent_expectations() {
    let all = cases::all();
    let mut names: Vec<&str> = all.iter().map(|c| c.name).collect();
    names.sort_unstable();
    names.dedup();
    assert_eq!(names.len(), all.len());
    for case in &all {
        if let Expect::Solid {
            volume,
            relative,
            bounds,
        } = &case.expect
        {
            let [lo, hi] = bounds;
            let size: Xyz = [hi[0] - lo[0], hi[1] - lo[1], hi[2] - lo[2]];
            assert!(*volume > 0.0 && *relative > 0.0, "{}", case.name);
            assert!(
                *volume <= size[0] * size[1] * size[2] * (1.0 + relative),
                "{}",
                case.name
            );
        }
    }
}

#[test]
fn kernels_return_failures_instead_of_panicking() {
    let tolerance = Tolerance {
        chord: cases::CHORD,
        length: 1e-9,
    };
    let mut evaluated = 0;
    for case in cases::all() {
        let result = with_cx(Limits::default(), |cx| {
            Faces.evaluate(&case.shape, tolerance, cx)
        });
        match (&case.expect, result) {
            (
                Expect::Solid {
                    volume,
                    relative,
                    bounds,
                },
                Ok(mesh),
            ) => {
                let welded = mesh.welded();
                assert!(welded.is_closed(), "{}", case.name);
                assert!(
                    (welded.volume() - volume).abs() <= relative * volume,
                    "{}: {}",
                    case.name,
                    welded.volume()
                );
                let got = welded.bounds().unwrap();
                for k in 0..3 {
                    assert!(
                        (got[0][k] - bounds[0][k]).abs() < 1e-9
                            && (got[1][k] - bounds[1][k]).abs() < 1e-9,
                        "{}",
                        case.name
                    );
                }
                evaluated += 1;
            }
            (Expect::Invalid, Err(Failure::Invalid(_))) | (_, Err(Failure::Unsupported(_))) => {}
            (Expect::Rejected, Err(_)) => {}
            (Expect::Rejected, Ok(mesh)) => assert!(mesh.welded().is_closed(), "{}", case.name),
            (_, other) => panic!("{}: {other:?}", case.name),
        }
    }
    assert_eq!(evaluated, 2, "face_set_cube と triangles_tetrahedron");
    // 予算を超えたら止める
    let limits = Limits::default().with(Kind::Segments, Limit::Finite(1));
    let cube = cases::all()
        .into_iter()
        .find(|c| c.name == "face_set_cube")
        .unwrap();
    let result = with_cx(limits, |cx| Faces.evaluate(&cube.shape, tolerance, cx));
    assert!(matches!(result, Err(Failure::Stopped(_))));
}
