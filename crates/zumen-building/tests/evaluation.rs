//! 合成した建物で配置・開口・失敗時の元形・共有と停止を確かめる。

#[path = "support/evaluation.rs"]
mod fixtures;

use fixtures::*;
use std::sync::Arc;
use zumen_base::{Budget, CancelToken, Cx, Diagnostics, Kind, Limit, Limits, Stop, codes};
use zumen_building::{
    evaluation::{self, Options},
    *,
};
use zumen_geom3d::{Failure, ManifoldKernel};

#[test]
fn wall_slab_and_joinery_keep_ids_and_local_precision_with_openings() {
    let b = building();
    let result = with(|cx| evaluation::evaluate(&b, Options::default(), cx)).unwrap();
    for (at, want) in [(0, 2.58), (1, 5.5), (2, 0.0348), (3, 0.0216)] {
        let element = &result[at];
        assert_eq!(element.id, b.id(ElementRef(at as u32)).unwrap());
        assert!(element.complete);
        assert_eq!(element.parts.len(), 1);
        let part = &element.parts[0];
        assert!(
            (part.mesh.volume() - want).abs() < 1e-8,
            "{at}: {}",
            part.mesh.volume()
        );
        assert!(with(|cx| ManifoldKernel.is_solid(&part.mesh, cx)).unwrap());
        assert_eq!(part.tolerance.length, 1e-6);
        assert_eq!(part.tolerance.chord, 0.001);
        assert_eq!(part.placement.point([0.0; 3]), [1e6, 2e6, 3.0]);
        assert!(!part.mesh.edges.is_empty());
    }
    assert_eq!(result.len(), b.elements().len());
    let uncut = with(|cx| {
        evaluation::evaluate(
            &b,
            Options {
                openings: false,
                ..Options::default()
            },
            cx,
        )
    })
    .unwrap();
    assert_eq!(uncut[0].parts[0].mesh.volume(), 3.0);
    assert_eq!(uncut[1].parts[0].mesh.volume(), 6.0);
}

#[test]
fn openings_of_an_empty_parent_cut_its_decomposed_parts() {
    let mut b = building();
    let parent = with(|cx| b.push(None, 100, cx)).unwrap();
    for kind in [
        ParentKind::Aggregates,
        ParentKind::Nests,
        ParentKind::Contained,
    ] {
        b.components.parent.insert(
            ElementRef(0),
            Parent {
                of: Some(Membership {
                    target: parent,
                    kind,
                    order: None,
                }),
                referenced_in: vec![],
            },
        );
        b.components
            .opening
            .insert(ElementRef(4), Opening::Voids(parent));
        let out = with(|cx| evaluation::evaluate(&b, Options::default(), cx)).unwrap();
        let expected = if kind == ParentKind::Contained {
            3.0
        } else {
            2.58
        };
        assert!((out[0].parts[0].mesh.volume() - expected).abs() < 1e-8);
        assert_eq!(out[0].id, b.id(ElementRef(0)).unwrap());
        assert!(out[0].complete);
        assert!(out[parent.0 as usize].parts.is_empty());
    }
}

#[test]
fn a_representation_without_a_body_is_reported() {
    let mut b = Building::default();
    add(
        &mut b,
        data(vec![cuboid([0.0; 3], [1000.0; 3])], &[0], "Axis"),
        "IfcMember",
    );
    let mut diagnostics = Diagnostics::new();
    let result = evaluation::evaluate(
        &b,
        Options::default(),
        &mut Cx::new(
            &mut Budget::default(),
            &mut diagnostics,
            &CancelToken::new(),
        ),
    )
    .unwrap();
    assert!(result[0].parts.is_empty());
    assert!(!result[0].complete);
    assert!(
        diagnostics
            .iter()
            .any(|d| d.code() == codes::BUILDING_SHAPE_DROPPED)
    );
}

#[test]
fn a_swept_disk_uses_polyline_parameters_and_reports_the_chord_error() {
    let mut b = Building::default();
    let sweep = node(
        GeometryLinks::Leaf,
        GeometryDefinition::SweptDisk {
            definition: DiskSweep {
                directrix: CurveRef(0),
                radius: 200.0,
                inner_radius: None,
                start_parameter: Some(0.5),
                end_parameter: Some(1.5),
            },
            directrix: CurveGraph {
                root: CurveRef(0),
                items: vec![CurveItem {
                    kind: CurveKind::Polyline(vec![
                        Coordinates::Three([0.0; 3]),
                        Coordinates::Three([0.0, 0.0, 2000.0]),
                        Coordinates::Three([3000.0, 0.0, 2000.0]),
                    ]),
                    source: CurveSource::default(),
                }],
                length_unit: Some(mm()),
                angle_unit: None,
            },
        },
    );
    add(&mut b, data(vec![sweep], &[0], "Body"), "IfcPipeSegment");
    let mut diagnostics = Diagnostics::new();
    let out = evaluation::evaluate(
        &b,
        Options::default(),
        &mut Cx::new(
            &mut Budget::default(),
            &mut diagnostics,
            &CancelToken::new(),
        ),
    )
    .unwrap();
    assert!(out[0].complete);
    let expected = core::f64::consts::PI * 0.04 * 2.5;
    assert!((out[0].parts[0].mesh.volume() - expected).abs() < expected * 0.01);
    let changes: Vec<_> = diagnostics
        .iter()
        .filter(|d| d.code() == codes::GEOM3D_CURVE_APPROXIMATED)
        .collect();
    assert_eq!(changes.len(), 1);
    assert_eq!(changes[0].count(), 1);
    assert_eq!(
        changes[0].param("chord"),
        Some(&zumen_base::Value::Float(0.001))
    );
}

#[test]
fn union_of_multiple_parts_does_not_duplicate_the_second_operand() {
    let mut b = Building::default();
    add(
        &mut b,
        data(
            vec![
                cuboid([0.0; 3], [1000.0; 3]),
                cuboid([2000.0, 0.0, 0.0], [3000.0, 1000.0, 1000.0]),
                node(
                    GeometryLinks::Set {
                        curves_only: false,
                        items: vec![GeometryRef(0), GeometryRef(1)],
                    },
                    GeometryDefinition::Links,
                ),
                cuboid([500.0, 0.0, 0.0], [2500.0, 1000.0, 1000.0]),
                node(
                    GeometryLinks::Boolean {
                        operation: BooleanOperation::Union,
                        first: GeometryRef(2),
                        second: GeometryRef(3),
                    },
                    GeometryDefinition::Links,
                ),
            ],
            &[4],
            "Body",
        ),
        "IfcBeam",
    );
    let out = with(|cx| evaluation::evaluate(&b, Options::default(), cx)).unwrap();
    assert!(out[0].complete);
    assert_eq!(out[0].parts.len(), 1);
    assert!((out[0].parts[0].mesh.volume() - 3.0).abs() < 1e-12);
}

#[test]
fn unsupported_operand_keeps_the_host_and_reports_source() {
    let mut b = Building::default();
    let mut unknown = node(
        GeometryLinks::Leaf,
        GeometryDefinition::Unsupported("CustomSurface".into()),
    );
    unknown.source.instance = Some(91);
    add(
        &mut b,
        data(
            vec![
                cuboid([0.0; 3], [1000.0; 3]),
                unknown,
                node(
                    GeometryLinks::Boolean {
                        operation: BooleanOperation::Difference,
                        first: GeometryRef(0),
                        second: GeometryRef(1),
                    },
                    GeometryDefinition::Links,
                ),
            ],
            &[2],
            "Body",
        ),
        "IfcSlab",
    );
    let mut diagnostics = Diagnostics::new();
    let out = evaluation::evaluate(
        &b,
        Options::default(),
        &mut Cx::new(
            &mut Budget::default(),
            &mut diagnostics,
            &CancelToken::new(),
        ),
    )
    .unwrap();
    assert!(!out[0].complete);
    assert_eq!(out[0].parts[0].mesh.volume(), 1.0);
    assert!(
        diagnostics
            .iter()
            .any(|d| d.code() == codes::BUILDING_SHAPE_DROPPED)
    );
    assert!(
        diagnostics
            .iter()
            .any(|d| d.code() == codes::BUILDING_BOOLEAN_FAILED)
    );
    assert!(format!("{diagnostics:?}").contains("91"));
}

#[test]
fn mapped_instances_share_the_mesh_but_keep_their_transforms() {
    let mut b = Building::default();
    let map = |origin: [f64; 3]| {
        node(
            GeometryLinks::Mapped(GeometryRef(0)),
            GeometryDefinition::Mapping {
                definition: ShapeMapping3 {
                    origin: axis([0.0; 3]),
                    target: CartesianMapping3 {
                        local_origin: origin,
                        axis1: None,
                        axis2: None,
                        axis3: None,
                        scale: MappingScale3::Uniform(None),
                    },
                },
                length_unit: Some(mm()),
            },
        )
    };
    add(
        &mut b,
        data(
            vec![
                cuboid([0.0; 3], [1000.0; 3]),
                map([0.0; 3]),
                map([2000.0, 0.0, 0.0]),
            ],
            &[1, 2],
            "Body",
        ),
        "IfcMember",
    );
    let out = with(|cx| evaluation::evaluate(&b, Options::default(), cx)).unwrap();
    assert!(out[0].complete);
    assert_eq!(out[0].parts.len(), 2);
    let [first, second] = out[0].parts.as_slice() else {
        panic!()
    };
    assert!(Arc::ptr_eq(&first.mesh, &second.mesh));
    assert_eq!(first.placement.point([0.0; 3]), [0.0; 3]);
    assert_eq!(second.placement.point([0.0; 3]), [2.0, 0.0, 0.0]);
}

#[test]
fn budget_boundaries_and_cancellation_do_not_return_partial_results() {
    let b = building();
    let mut budget = Budget::default();
    let cancel = CancelToken::new();
    let expected = evaluation::evaluate(
        &b,
        Options::default(),
        &mut Cx::new(&mut budget, &mut Diagnostics::new(), &cancel),
    )
    .unwrap();
    for kind in [Kind::Work, Kind::DecodedBytes, Kind::Segments] {
        let used = budget.used(kind);
        assert!(used > 0);
        for (limit, stops) in [(used, false), (used - 1, true), (0, true)] {
            let mut limited = Budget::new(Limits::default().with(kind, Limit::Finite(limit)));
            let result = evaluation::evaluate(
                &b,
                Options::default(),
                &mut Cx::new(&mut limited, &mut Diagnostics::new(), &cancel),
            );
            if stops {
                assert!(
                    matches!(result, Err(Failure::Stopped(Stop::Exceeded(e))) if e.kind == kind)
                );
            } else {
                assert_eq!(result.unwrap(), expected);
            }
            assert_eq!(limited.used(Kind::Depth), 0);
        }
    }
    cancel.cancel();
    let result = evaluation::evaluate(
        &b,
        Options::default(),
        &mut Cx::new(&mut Budget::default(), &mut Diagnostics::new(), &cancel),
    );
    assert_eq!(result, Err(Failure::Stopped(Stop::Cancelled)));
}
