//! カーネルの候補を、試料なしで比べるための合成の形の組（#414）。
//!
//! 期待する体積と箱は、形から解析的に求めた値。曲面を持つ形は、弦の誤差で折れ線にした分だけ
//! 小さくなるので、許す相対の誤差を広くとる。

use std::f64::consts::PI;

use crate::{BooleanOp, Curve, Face, Plane, Profile, Shape, Transform, Xy, Xyz};

/// 期待する結果
#[derive(Clone, Debug, PartialEq)]
pub enum Expect {
    /// 閉じた立体
    Solid {
        /// 体積
        volume: f64,
        /// 許す相対の誤差
        relative: f64,
        /// 囲む箱（最小と最大）
        bounds: [Xyz; 2],
    },
    /// 形が正しくないので、失敗を返す（パニックしない）
    Invalid,
    /// 閉じていない・厚さのない形のブーリアン。失敗を返すか、閉じた形を返す（パニックしない）
    Rejected,
}

/// 形と期待する結果の組
#[derive(Clone, Debug, PartialEq)]
pub struct Case {
    /// 名前
    pub name: &'static str,
    /// 形
    pub shape: Shape,
    /// 期待する結果
    pub expect: Expect,
}

/// 平面の曲面だけの形の誤差
const FLAT: f64 = 1e-9;
/// 曲面を持つ形の誤差（弦の誤差 [`CHORD`] のとき）
const CURVED: f64 = 1e-2;
/// 比べるときの弦の誤差
pub const CHORD: f64 = 1e-3;

fn rectangle(x0: f64, y0: f64, x1: f64, y1: f64) -> Curve {
    Curve::Polygon(vec![[x0, y0], [x1, y0], [x1, y1], [x0, y1]])
}

fn profile(outer: Curve) -> Profile {
    Profile {
        outer,
        holes: Vec::new(),
    }
}

/// `[x0, x1] × [y0, y1] × [z0, z1]` の箱を、押し出しで作る
pub fn cuboid(lo: Xyz, hi: Xyz) -> Shape {
    Shape::Extrusion {
        profile: profile(rectangle(lo[0], lo[1], hi[0], hi[1])),
        position: Transform::translation([0.0, 0.0, lo[2]]),
        direction: [0.0, 0.0, 1.0],
        depth: hi[2] - lo[2],
    }
}

fn solid(volume: f64, relative: f64, lo: Xyz, hi: Xyz) -> Expect {
    Expect::Solid {
        volume,
        relative,
        bounds: [lo, hi],
    }
}

fn cube_faces() -> Vec<Face> {
    let p = |x: f64, y: f64, z: f64| [x, y, z];
    let quad = |a: Xyz, b: Xyz, c: Xyz, d: Xyz| Face {
        outer: vec![a, b, c, d],
        holes: Vec::new(),
    };
    vec![
        quad(p(0., 0., 0.), p(0., 1., 0.), p(1., 1., 0.), p(1., 0., 0.)),
        quad(p(0., 0., 1.), p(1., 0., 1.), p(1., 1., 1.), p(0., 1., 1.)),
        quad(p(0., 0., 0.), p(1., 0., 0.), p(1., 0., 1.), p(0., 0., 1.)),
        quad(p(1., 1., 0.), p(0., 1., 0.), p(0., 1., 1.), p(1., 1., 1.)),
        quad(p(0., 1., 0.), p(0., 0., 0.), p(0., 0., 1.), p(0., 1., 1.)),
        quad(p(1., 0., 0.), p(1., 1., 0.), p(1., 1., 1.), p(1., 0., 1.)),
    ]
}

fn boolean(op: BooleanOp, first: Shape, second: Shape) -> Shape {
    Shape::Boolean {
        op,
        first: Box::new(first),
        second: Box::new(second),
    }
}

/// 合成の形の組
pub fn all() -> Vec<Case> {
    let ring: Vec<Xy> = vec![[0.0, 0.0], [4.0, 0.0], [4.0, 3.0], [0.0, 3.0]];
    let tilted = Transform::from_axes(
        [1.0, 2.0, 3.0],
        Some([0.0, 1.0, 1.0]),
        Some([1.0, 0.0, 0.0]),
    )
    .unwrap_or(Transform::IDENTITY);
    let s2 = core::f64::consts::FRAC_1_SQRT_2;
    vec![
        Case {
            name: "extrusion_rectangle",
            shape: Shape::Extrusion {
                profile: profile(Curve::Polygon(ring.clone())),
                position: Transform::IDENTITY,
                direction: [0.0, 0.0, 1.0],
                depth: 2.0,
            },
            expect: solid(24.0, FLAT, [0.0, 0.0, 0.0], [4.0, 3.0, 2.0]),
        },
        Case {
            name: "extrusion_with_hole_tilted",
            shape: Shape::Extrusion {
                profile: Profile {
                    outer: rectangle(-1.0, -1.0, 1.0, 1.0),
                    holes: vec![rectangle(-0.5, -0.5, 0.5, 0.5)],
                },
                position: tilted,
                direction: [0.0, 0.0, 1.0],
                depth: 3.0,
            },
            // 局所の Z は (0, 1/√2, 1/√2)、X は (1, 0, 0)、Y は (0, 1/√2, −1/√2)
            expect: solid(
                9.0,
                FLAT,
                [0.0, 2.0 - s2, 3.0 - s2],
                [2.0, 2.0 + 4.0 * s2, 3.0 + 4.0 * s2],
            ),
        },
        Case {
            name: "extrusion_oblique",
            shape: Shape::Extrusion {
                profile: profile(rectangle(0.0, 0.0, 1.0, 1.0)),
                position: Transform::IDENTITY,
                direction: [1.0, 0.0, 1.0],
                depth: 2.0 * core::f64::consts::SQRT_2,
            },
            expect: solid(2.0, FLAT, [0.0, 0.0, 0.0], [3.0, 1.0, 2.0]),
        },
        Case {
            name: "extrusion_circle",
            shape: Shape::Extrusion {
                profile: profile(Curve::Circle {
                    center: [0.0, 0.0],
                    radius: 1.0,
                }),
                position: Transform::IDENTITY,
                direction: [0.0, 0.0, 1.0],
                depth: 2.0,
            },
            expect: solid(2.0 * PI, CURVED, [-1.0, -1.0, 0.0], [1.0, 1.0, 2.0]),
        },
        Case {
            name: "revolution_full",
            // [1, 2] × [0, 1] の断面を、局所の Y 軸のまわりに一周回す
            shape: Shape::Revolution {
                profile: profile(rectangle(1.0, 0.0, 2.0, 1.0)),
                position: Transform::IDENTITY,
                axis_origin: [0.0, 0.0, 0.0],
                axis_direction: [0.0, 1.0, 0.0],
                angle: 360.0,
            },
            expect: solid(3.0 * PI, CURVED, [-2.0, 0.0, -2.0], [2.0, 1.0, 2.0]),
        },
        Case {
            name: "revolution_quarter",
            shape: Shape::Revolution {
                profile: profile(rectangle(1.0, 0.0, 2.0, 1.0)),
                position: Transform::IDENTITY,
                axis_origin: [0.0, 0.0, 0.0],
                axis_direction: [0.0, 1.0, 0.0],
                angle: 90.0,
            },
            // X から −Z へ回る（Y のまわりに右ねじ）
            expect: solid(0.75 * PI, CURVED, [0.0, 0.0, -2.0], [2.0, 1.0, 0.0]),
        },
        Case {
            name: "swept_disk_straight",
            shape: Shape::SweptDisk {
                directrix: vec![[0.0, 0.0, 0.0], [5.0, 0.0, 0.0]],
                radius: 0.5,
                inner_radius: None,
            },
            expect: solid(1.25 * PI, CURVED, [0.0, -0.5, -0.5], [5.0, 0.5, 0.5]),
        },
        Case {
            name: "swept_disk_pipe",
            shape: Shape::SweptDisk {
                directrix: vec![[0.0, 0.0, 0.0], [0.0, 0.0, 4.0]],
                radius: 0.5,
                inner_radius: Some(0.25),
            },
            expect: solid(0.75 * PI, CURVED, [-0.5, -0.5, 0.0], [0.5, 0.5, 4.0]),
        },
        Case {
            name: "face_set_cube",
            shape: Shape::FaceSet {
                faces: cube_faces(),
                closed: true,
            },
            expect: solid(1.0, FLAT, [0.0, 0.0, 0.0], [1.0, 1.0, 1.0]),
        },
        Case {
            name: "face_set_with_hole",
            // 上下の面に穴のある、角の筒（外 2 × 2、穴 1 × 1、高さ 1）
            shape: Shape::FaceSet {
                faces: tube_faces(),
                closed: true,
            },
            expect: solid(3.0, FLAT, [0.0, 0.0, 0.0], [2.0, 2.0, 1.0]),
        },
        Case {
            name: "triangles_tetrahedron",
            shape: Shape::Triangles {
                positions: vec![
                    [0.0, 0.0, 0.0],
                    [1.0, 0.0, 0.0],
                    [0.0, 1.0, 0.0],
                    [0.0, 0.0, 1.0],
                ],
                triangles: vec![[0, 2, 1], [0, 1, 3], [0, 3, 2], [1, 2, 3]],
                closed: true,
            },
            expect: solid(1.0 / 6.0, FLAT, [0.0, 0.0, 0.0], [1.0, 1.0, 1.0]),
        },
        Case {
            name: "mapped_scaled_mirrored",
            // 裏返す写像。体積は |det| 倍
            shape: Shape::Mapped {
                transform: Transform::scaling([-2.0, 1.0, 3.0])
                    .then(&Transform::translation([10.0, 0.0, 0.0])),
                shape: Box::new(cuboid([0.0, 0.0, 0.0], [1.0, 1.0, 1.0])),
            },
            expect: solid(6.0, FLAT, [8.0, 0.0, 0.0], [10.0, 1.0, 3.0]),
        },
        Case {
            name: "group_disjoint",
            shape: Shape::Group(vec![
                cuboid([0.0, 0.0, 0.0], [1.0, 1.0, 1.0]),
                cuboid([2.0, 0.0, 0.0], [3.0, 1.0, 2.0]),
            ]),
            expect: solid(3.0, FLAT, [0.0, 0.0, 0.0], [3.0, 1.0, 2.0]),
        },
        Case {
            name: "boolean_union_overlap",
            shape: boolean(
                BooleanOp::Union,
                cuboid([0.0, 0.0, 0.0], [2.0, 2.0, 2.0]),
                cuboid([1.0, 1.0, 1.0], [3.0, 3.0, 3.0]),
            ),
            expect: solid(15.0, FLAT, [0.0, 0.0, 0.0], [3.0, 3.0, 3.0]),
        },
        Case {
            name: "boolean_intersection",
            shape: boolean(
                BooleanOp::Intersection,
                cuboid([0.0, 0.0, 0.0], [2.0, 2.0, 2.0]),
                cuboid([1.0, 1.0, 1.0], [3.0, 3.0, 3.0]),
            ),
            expect: solid(1.0, FLAT, [1.0, 1.0, 1.0], [2.0, 2.0, 2.0]),
        },
        Case {
            name: "boolean_difference_coplanar",
            // 引く箱の面が、元の箱の面と同じ平面にある（BIM の開口でよくある形）
            shape: boolean(
                BooleanOp::Difference,
                cuboid([0.0, 0.0, 0.0], [4.0, 1.0, 3.0]),
                cuboid([1.0, 0.0, 0.0], [2.0, 1.0, 2.0]),
            ),
            expect: solid(10.0, FLAT, [0.0, 0.0, 0.0], [4.0, 1.0, 3.0]),
        },
        Case {
            name: "boolean_difference_near_coplanar",
            // 引く箱の面が、元の箱の面から 1e-9 だけずれている。厚さ 1e-9 の薄い壁が残る
            shape: boolean(
                BooleanOp::Difference,
                cuboid([0.0, 0.0, 0.0], [4.0, 1.0, 3.0]),
                cuboid([1.0, 1e-9, 1e-9], [2.0, 1.0 - 1e-9, 2.0]),
            ),
            expect: solid(
                12.0 - 2.0 * (1.0 - 2e-9) * (1.0 - 1e-9 / 2.0),
                1e-6,
                [0.0, 0.0, 0.0],
                [4.0, 1.0, 3.0],
            ),
        },
        Case {
            name: "boolean_cylinder_hole",
            shape: boolean(
                BooleanOp::Difference,
                cuboid([-2.0, -2.0, 0.0], [2.0, 2.0, 1.0]),
                Shape::Extrusion {
                    profile: profile(Curve::Circle {
                        center: [0.0, 0.0],
                        radius: 1.0,
                    }),
                    position: Transform::translation([0.0, 0.0, -1.0]),
                    direction: [0.0, 0.0, 1.0],
                    depth: 3.0,
                },
            ),
            expect: solid(16.0 - PI, CURVED, [-2.0, -2.0, 0.0], [2.0, 2.0, 1.0]),
        },
        Case {
            name: "half_space_clip",
            // 箱の上を、斜めの平面で切る（屋根の勾配に合わせて壁を切る形）
            shape: boolean(
                BooleanOp::Difference,
                cuboid([0.0, 0.0, 0.0], [2.0, 1.0, 2.0]),
                Shape::HalfSpace {
                    plane: Plane {
                        origin: [0.0, 0.0, 1.0],
                        normal: [-1.0, 0.0, 2.0],
                    },
                },
            ),
            // 半空間の中身（平面の裏、z ≤ 1 + x/2）を除き、その上の三角柱を残す
            expect: solid(1.0, FLAT, [0.0, 0.0, 1.0], [2.0, 1.0, 2.0]),
        },
        Case {
            name: "wall_with_openings",
            // 長さ 5・厚さ 0.2・高さ 3 の壁に、扉（1 × 2.1）と窓（1.2 × 1）をあける。開口は壁より厚い
            shape: boolean(
                BooleanOp::Difference,
                boolean(
                    BooleanOp::Difference,
                    cuboid([0.0, 0.0, 0.0], [5.0, 0.2, 3.0]),
                    cuboid([0.5, -0.1, 0.0], [1.5, 0.3, 2.1]),
                ),
                cuboid([2.5, -0.1, 1.0], [3.7, 0.3, 2.0]),
            ),
            expect: solid(
                3.0 - 0.2 * 2.1 - 0.2 * 1.2,
                1e-6,
                [0.0, 0.0, 0.0],
                [5.0, 0.2, 3.0],
            ),
        },
        Case {
            name: "slab_with_openings",
            // 6 × 4 × 0.25 の床に、階段の開口と、円の配管の穴をあける
            shape: boolean(
                BooleanOp::Difference,
                boolean(
                    BooleanOp::Difference,
                    cuboid([0.0, 0.0, 0.0], [6.0, 4.0, 0.25]),
                    cuboid([1.0, 1.0, -0.5], [3.0, 2.0, 0.75]),
                ),
                Shape::Extrusion {
                    profile: profile(Curve::Circle {
                        center: [5.0, 3.0],
                        radius: 0.1,
                    }),
                    position: Transform::translation([0.0, 0.0, -0.5]),
                    direction: [0.0, 0.0, 1.0],
                    depth: 1.25,
                },
            ),
            expect: solid(
                6.0 - 0.5 - 0.25 * PI * 0.01,
                1e-4,
                [0.0, 0.0, 0.0],
                [6.0, 4.0, 0.25],
            ),
        },
        Case {
            name: "boolean_open_operand",
            // 2 つめが、閉じていない 1 枚の面
            shape: boolean(
                BooleanOp::Difference,
                cuboid([0.0, 0.0, 0.0], [2.0, 2.0, 2.0]),
                Shape::Triangles {
                    positions: vec![
                        [-1.0, 1.0, -1.0],
                        [3.0, 1.0, -1.0],
                        [3.0, 1.0, 3.0],
                        [-1.0, 1.0, 3.0],
                    ],
                    triangles: vec![[0, 1, 2], [0, 2, 3]],
                    closed: false,
                },
            ),
            expect: Expect::Rejected,
        },
        Case {
            name: "boolean_flat_operand",
            // 2 つめが、厚さを 0 に潰した箱
            shape: boolean(
                BooleanOp::Difference,
                cuboid([0.0, 0.0, 0.0], [2.0, 2.0, 2.0]),
                Shape::Mapped {
                    transform: Transform::scaling([1.0, 0.0, 1.0])
                        .then(&Transform::translation([0.5, 1.0, 0.5])),
                    shape: Box::new(cuboid([0.0, 0.0, 0.0], [1.0, 1.0, 1.0])),
                },
            ),
            expect: Expect::Rejected,
        },
        Case {
            name: "invalid_zero_depth",
            shape: Shape::Extrusion {
                profile: profile(rectangle(0.0, 0.0, 1.0, 1.0)),
                position: Transform::IDENTITY,
                direction: [0.0, 0.0, 1.0],
                depth: 0.0,
            },
            expect: Expect::Invalid,
        },
        Case {
            name: "invalid_not_finite",
            shape: Shape::Extrusion {
                profile: profile(Curve::Polygon(vec![
                    [0.0, 0.0],
                    [f64::NAN, 0.0],
                    [1.0, 1.0],
                ])),
                position: Transform::IDENTITY,
                direction: [0.0, 0.0, 1.0],
                depth: 1.0,
            },
            expect: Expect::Invalid,
        },
        Case {
            name: "invalid_index",
            shape: Shape::Triangles {
                positions: vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
                triangles: vec![[0, 1, 7]],
                closed: true,
            },
            expect: Expect::Invalid,
        },
    ]
}

/// 上下の面に穴を持つ、角の筒の面
fn tube_faces() -> Vec<Face> {
    let quad = |a: Xyz, b: Xyz, c: Xyz, d: Xyz| Face {
        outer: vec![a, b, c, d],
        holes: Vec::new(),
    };
    let at = |x: f64, y: f64, z: f64| [x, y, z];
    let square = |z: f64, lo: f64, hi: f64, up: bool| {
        let r = vec![at(lo, lo, z), at(hi, lo, z), at(hi, hi, z), at(lo, hi, z)];
        if up { r } else { r.into_iter().rev().collect() }
    };
    let mut faces = vec![
        Face {
            outer: square(1.0, 0.0, 2.0, true),
            holes: vec![square(1.0, 0.5, 1.5, false)],
        },
        Face {
            outer: square(0.0, 0.0, 2.0, false),
            holes: vec![square(0.0, 0.5, 1.5, true)],
        },
    ];
    // 外の側面（外向き）と内の側面（穴の内を向く）
    for (lo, hi, out) in [(0.0, 2.0, true), (0.5, 1.5, false)] {
        let corners = [
            at(lo, lo, 0.0),
            at(hi, lo, 0.0),
            at(hi, hi, 0.0),
            at(lo, hi, 0.0),
        ];
        for i in 0..4 {
            let (Some(a), Some(b)) = (corners.get(i), corners.get((i + 1) % 4)) else {
                continue;
            };
            let (a, b) = if out { (*a, *b) } else { (*b, *a) };
            faces.push(quad(a, b, [b[0], b[1], 1.0], [a[0], a[1], 1.0]));
        }
    }
    faces
}
