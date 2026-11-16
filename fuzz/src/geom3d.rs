//! 有効な立体と、入力の値で変えた形を同じ予算で 2 回評価する。

use zumen_base::{Budget, CancelToken, Cx, Diagnostics, Kind, Limit, Limits};
use zumen_geom3d::{
    BooleanOp, Curve, Face, Kernel, ManifoldKernel, Plane, Profile, Shape, Tolerance, Transform,
    cases,
};

fn byte(data: &[u8], i: usize) -> u8 {
    data.get(i).copied().unwrap_or(0)
}

fn scalar(data: &[u8], i: usize) -> f64 {
    if byte(data, 1) & 1 == 0 {
        f64::from(byte(data, i)) / 16.0 - 2.0
    } else {
        f64::from_bits(u64::from_le_bytes(std::array::from_fn(|j| {
            byte(data, i + j)
        })))
    }
}

pub fn shape(data: &[u8]) -> Shape {
    let point = |i| std::array::from_fn(|j| scalar(data, i + 8 * j));
    let base = cases::cuboid([0.0; 3], [1.0; 3]);
    match byte(data, 0) % 7 {
        0 => Shape::Boolean {
            op: match byte(data, 2) % 3 {
                0 => BooleanOp::Difference,
                1 => BooleanOp::Union,
                _ => BooleanOp::Intersection,
            },
            first: Box::new(base),
            second: Box::new(cases::cuboid(point(3), point(27))),
        },
        1 => Shape::Boolean {
            op: BooleanOp::Difference,
            first: Box::new(base),
            second: Box::new(Shape::HalfSpace {
                plane: Plane {
                    origin: point(3),
                    normal: point(27),
                },
            }),
        },
        2 => Shape::Triangles {
            positions: (0..4).map(|i| point(3 + i * 24)).collect(),
            triangles: (0..4)
                .map(|i| std::array::from_fn(|j| u32::from(byte(data, 100 + i * 3 + j))))
                .collect(),
            closed: true,
        },
        3 => Shape::FaceSet {
            faces: vec![Face {
                outer: (0..(3 + usize::from(byte(data, 2) % 8)))
                    .map(|i| point(3 + i * 24))
                    .collect(),
                holes: vec![],
            }],
            closed: false,
        },
        4 => Shape::SweptDisk {
            directrix: vec![[0.0; 3], [0.0, 0.0, 1.0], point(3)],
            radius: scalar(data, 27),
            inner_radius: (byte(data, 2) & 1 != 0).then(|| scalar(data, 35)),
        },
        5 => Shape::Extrusion {
            profile: Profile {
                outer: Curve::Circle {
                    center: [0.0; 2],
                    radius: scalar(data, 3),
                },
                holes: vec![],
            },
            position: Transform::IDENTITY,
            direction: point(11),
            depth: scalar(data, 35),
        },
        _ => Shape::Revolution {
            profile: Profile {
                outer: Curve::Polygon(vec![[1.0, 0.0], [2.0, 0.0], [2.0, 1.0], [1.0, 1.0]]),
                holes: vec![],
            },
            position: Transform::IDENTITY,
            axis_origin: point(3),
            axis_direction: point(27),
            angle: scalar(data, 51),
        },
    }
}

pub fn exercise(data: &[u8]) {
    let shape = shape(data);
    let limits = Limits::default()
        .with(Kind::DecodedBytes, Limit::Finite(8 * 1024 * 1024))
        .with(Kind::Work, Limit::Finite(1 << 20))
        .with(Kind::Segments, Limit::Finite(8192))
        .with(Kind::Depth, Limit::Finite(16));
    let run = |limits, cancelled| {
        let mut budget = Budget::new(limits);
        let mut diagnostics = Diagnostics::new();
        let cancel = CancelToken::new();
        if cancelled {
            cancel.cancel();
        }
        let result = ManifoldKernel.evaluate(
            &shape,
            Tolerance {
                chord: 0.001,
                length: 1e-9,
            },
            &mut Cx::new(&mut budget, &mut diagnostics, &cancel),
        );
        if let Ok(mesh) = &result {
            assert!(mesh.validate().is_ok());
        }
        assert_eq!(budget.used(Kind::Depth), 0);
        (
            result,
            format!("{diagnostics:?}"),
            Kind::ALL.map(|k| budget.used(k)),
        )
    };
    assert_eq!(run(limits, false), run(limits, false));
    for kind in [Kind::DecodedBytes, Kind::Segments, Kind::Work, Kind::Depth] {
        let limits = limits.with(kind, Limit::Finite(u64::from(byte(data, 200))));
        assert_eq!(run(limits, false), run(limits, false));
    }
    assert_eq!(run(limits, true), run(limits, true));
}

#[cfg(test)]
mod tests {
    #[test]
    fn fixed_inputs_exercise_each_shape_and_non_finite_values() {
        for mode in 0..7 {
            for value in [0, 1, 48, 127, 255] {
                let mut data = vec![value; 208];
                data[0] = mode;
                super::exercise(&data);
            }
        }
    }
}
