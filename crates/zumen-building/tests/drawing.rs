//! 合成した建物の平断面・断面・立面と、元の要素への対応。
#![cfg(feature = "drawing")]
#![allow(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    clippy::panic,
    reason = "試験用に作る既知の建物と、その期待値の検査"
)]

#[path = "support/evaluation.rs"]
mod fixtures;

#[path = "support/drawing.rs"]
mod drawing_fixtures;
use drawing_fixtures::*;
use fixtures::*;
use zumen_base::{Budget, CancelToken, Cx, Diagnostics, Kind, Limit, Limits, Stop};
use zumen_building::{
    drawing::{self, Options, Role, Side, View},
    *,
};

use zumen_drawing::{ModelLength, PaperLength, Shape, SpaceId};
use zumen_geom3d::Failure;

#[test]
fn plan_section_and_elevation_have_roles_scale_and_original_sources() {
    let b = cube();
    let views = [
        View::Plan {
            floor: point(0.0, 0.0, 0.0),
            height: 500.0,
            depth: 1000.0,
        },
        View::Vertical {
            start: point(0.0, 500.0, 0.0),
            end: point(1000.0, 500.0, 0.0),
            side: Side::Right,
            depth: 1000.0,
            section: true,
        },
        View::Vertical {
            start: point(0.0, -100.0, 0.0),
            end: point(1000.0, -100.0, 0.0),
            side: Side::Right,
            depth: 1200.0,
            section: false,
        },
    ];
    for view in views {
        let out = with(|cx| drawing::cut(&b, options(view), cx)).unwrap();
        assert!(out.complete);
        assert!(!out.lines.is_empty());
        assert_eq!(
            out.lines.iter().any(|l| l.role == Role::Cut),
            view.frame().unwrap().is_section()
        );
        assert_eq!(out.drawing.spaces[0].unit, zumen_drawing::Unit::Millimeter);
        let view = &out.drawing.sheets[0].views[0];
        assert_eq!(
            view.paper_length(
                ModelLength {
                    space: SpaceId(0),
                    value: 1000.0
                },
                [1.0, 0.0]
            ),
            Some(PaperLength(10.0))
        );
        let mut names = zumen_drawing::id::Namer::new("space/0");
        for (i, line) in out.lines.iter().enumerate() {
            let source = &out.sources[line.source];
            assert_eq!(source.id, b.id(ElementRef(0)).unwrap());
            assert_eq!(source.item, Some(GeometryRef(0)));
            assert_eq!(source.shape_instance, Some(42));
            assert_eq!(source.global_id.as_deref(), Some("0000000000000000000001"));
            assert_eq!(
                line.drawing_id,
                names.next(i, &out.drawing.spaces[0].elements[i])
            );
            assert_eq!(
                out.drawing.spaces[0].elements[i].source.id(),
                Some(line.id.as_str())
            );
            assert!(out.drawing.extensions[i].is_current(&out.drawing));
        }
        let again = with(|cx| drawing::cut(&b, options(views[0]), cx)).unwrap();
        let repeat = with(|cx| drawing::cut(&b, options(views[0]), cx)).unwrap();
        assert_eq!(again, repeat);
    }
}

#[test]
fn opening_remains_a_gap_in_the_wall_cut_at_large_coordinates() {
    let b = building();
    let original = b.clone();
    let out = with(|cx| {
        drawing::cut(
            &b,
            options(View::Plan {
                floor: point(1e9, 2e9, 3000.0),
                height: 1000.0,
                depth: 1500.0,
            }),
            cx,
        )
    })
    .unwrap();
    assert_eq!(b, original);
    let mut wall_lines = 0;
    for line in &out.lines {
        let source = &out.sources[line.source];
        assert!(!matches!(
            b.components.opening.get(source.element),
            Some(Opening::Voids(_))
        ));
        if source.element != ElementRef(0) || line.role != Role::Cut {
            continue;
        }
        wall_lines += 1;
        let Shape::Line { start, end } = out.drawing.spaces[0].elements[line.index].shape else {
            panic!()
        };
        let middle = (start.x + end.x) / 2.0;
        assert!(
            middle <= 500.0 || middle >= 1500.0,
            "opening gap: {start:?} {end:?}"
        );
    }
    assert!(wall_lines >= 8);
}

#[test]
fn drawing_stops_on_shared_budgets_and_rejects_unknown_units() {
    let b = cube();
    let options = options(View::Plan {
        floor: point(0.0, 0.0, 0.0),
        height: 500.0,
        depth: 1000.0,
    });
    for kind in [Kind::Work, Kind::DecodedBytes, Kind::Segments] {
        let mut budget = Budget::new(Limits::default().with(kind, Limit::Finite(0)));
        let mut diag = Diagnostics::new();
        let cancel = CancelToken::new();
        let result = drawing::cut(&b, options, &mut Cx::new(&mut budget, &mut diag, &cancel));
        assert!(matches!(result, Err(Failure::Stopped(Stop::Exceeded(e))) if e.kind == kind));
    }
    let mut unknown = b.clone();
    unknown.document.units.clear();
    assert!(matches!(
        with(|cx| drawing::cut(&unknown, options, cx)),
        Err(Failure::Invalid("drawing_unit"))
    ));
}

#[test]
fn joinery_uses_plan_then_typed_operation_symbol_then_cut_geometry() {
    let symbol_curves = [line_graph(321.0)];
    let rules = [drawing::JoinerySymbol {
        element: None,
        set: "panel",
        property: "PanelOperation",
        type_name: "IfcDoorPanelOperationEnum",
        operation: "SWINGING",
        curves: &symbol_curves,
    }];
    let mut opts = options(View::Plan {
        floor: point(0.0, 0.0, 0.0),
        height: 500.0,
        depth: 1000.0,
    });
    // 生存期間が異なる記号の列を、呼び出しごとの設定に渡す。
    let symbol_options = Options {
        symbols: &rules,
        ..opts
    };
    let mut b = door();
    let symbol = with(|cx| drawing::cut(&b, symbol_options, cx)).unwrap();
    assert!(symbol.complete);
    assert_eq!(symbol.lines.len(), 1);
    assert_eq!(symbol.lines[0].role, Role::Annotation);
    assert_eq!(symbol.sources[0].item, None);
    let Shape::Line { end, .. } = symbol.drawing.spaces[0].elements[0].shape else {
        panic!()
    };
    assert_eq!(end.x, 321.0);
    // 幅のある建具記号が格子で消えた場合は、母体の線を描けても不完全にする。
    let tiny = [line_graph(1.0)];
    let small_rules = [drawing::JoinerySymbol {
        curves: &tiny,
        ..rules[0].clone()
    }];
    let mut diagnostics = Diagnostics::new();
    let small = drawing::cut(
        &b,
        Options {
            symbols: &small_rules,
            tolerance: 10.0,
            ..opts
        },
        &mut Cx::new(
            &mut Budget::default(),
            &mut diagnostics,
            &CancelToken::new(),
        ),
    )
    .unwrap();
    assert!(!small.complete);
    assert_eq!(small.statistics.collapsed, 1);
    assert!(
        diagnostics
            .iter()
            .any(|d| d.code() == zumen_base::codes::BUILDING_LINE_DROPPED
                && d.param("reason")
                    == Some(&zumen_base::Value::Text("projected_curve".to_owned())))
    );

    add_plan(&mut b);
    let plan = with(|cx| drawing::cut(&b, symbol_options, cx)).unwrap();
    assert!(plan.complete);
    assert_eq!(plan.lines.len(), 1);
    assert_eq!(plan.sources[0].shape_instance, Some(99));
    let Shape::Line { end, .. } = plan.drawing.spaces[0].elements[0].shape else {
        panic!()
    };
    assert_eq!(end.x, 123.0);
    let mut diag = Diagnostics::new();
    opts.symbols = &[];
    let fallback = drawing::cut(
        &door(),
        opts,
        &mut Cx::new(&mut Budget::default(), &mut diag, &CancelToken::new()),
    )
    .unwrap();
    assert!(fallback.lines.iter().any(|l| l.role == Role::Cut));
    assert!(
        diag.iter()
            .any(|d| d.code() == zumen_base::codes::BUILDING_JOINERY_DRAWN)
    );
}

#[test]
fn failed_geometry_reports_its_element_and_has_no_invented_lines() {
    let mut b = cube();
    let mut geometry = b.geometry(ElementRef(0)).unwrap().data().clone();
    geometry.nodes[0].definition = GeometryDefinition::Unsupported("test_surface".into());
    with(|cx| b.insert_geometry(ElementRef(0), Geometry::new(geometry, cx).unwrap(), cx)).unwrap();
    let mut diag = Diagnostics::new();
    let out = drawing::cut(
        &b,
        options(View::Plan {
            floor: point(0.0, 0.0, 0.0),
            height: 500.0,
            depth: 1000.0,
        }),
        &mut Cx::new(&mut Budget::default(), &mut diag, &CancelToken::new()),
    )
    .unwrap();
    assert!(!out.complete);
    assert!(out.lines.is_empty());
    assert!(
        diag.iter()
            .any(|d| d.code() == zumen_base::codes::BUILDING_SHAPE_DROPPED
                && d.places().iter().any(|p| p.id() == Some("i1")))
    );
}

#[test]
fn grid_lines_keep_axis_and_curve_sources_and_define_a_vertical_view() {
    let mut b = cube();
    let (owner, axis) = add_grid(&mut b);
    let out = with(|cx| {
        drawing::cut(
            &b,
            options(View::Plan {
                floor: point(0.0, 0.0, 0.0),
                height: 500.0,
                depth: 1000.0,
            }),
            cx,
        )
    })
    .unwrap();
    assert!(out.complete);
    let lines = out
        .lines
        .iter()
        .filter(|l| l.role == Role::Grid)
        .collect::<Vec<_>>();
    assert_eq!(lines.len(), 1);
    let source = &out.sources[lines[0].source];
    assert_eq!(source.element, axis);
    assert_eq!(source.instance, Some(202));
    assert_eq!(source.shape_instance, Some(203));
    let request = drawing::GridView {
        grid: owner,
        axis,
        elevation: 3000.0,
        side: Side::Right,
        depth: 5000.0,
        section: true,
    };
    let view = with(|cx| request.view(&b, cx)).unwrap();
    assert_eq!(
        view,
        View::Vertical {
            start: point(0.0, 0.0, 3000.0),
            end: point(2000.0, 0.0, 3000.0),
            side: Side::Right,
            depth: 5000.0,
            section: true
        }
    );
    let mut geometry = b.geometry(owner).unwrap().data().clone();
    let Some(Acquired::Value(grid)) = &mut geometry.grid else {
        panic!()
    };
    grid.axes[0].definition.same_sense = false;
    with(|cx| b.insert_geometry(owner, Geometry::new(geometry, cx).unwrap(), cx)).unwrap();
    let reversed = with(|cx| request.view(&b, cx)).unwrap();
    assert_eq!(
        reversed,
        View::Vertical {
            start: point(2000.0, 0.0, 3000.0),
            end: point(0.0, 0.0, 3000.0),
            side: Side::Right,
            depth: 5000.0,
            section: true
        }
    );
    let invalid = drawing::GridView {
        axis: ElementRef(0),
        ..request
    };
    assert!(with(|cx| invalid.view(&b, cx)).is_err());
}

#[test]
fn joinery_symbol_retains_the_body_as_an_occluder() {
    let mut b = door();
    add_plan(&mut b);
    let behind = add(
        &mut b,
        data(
            vec![cuboid([200.0, 200.0, 0.0], [800.0, 800.0, 100.0])],
            &[0],
            "Body",
        ),
        "IfcSlab",
    );
    let out = with(|cx| {
        drawing::cut(
            &b,
            options(View::Plan {
                floor: point(0.0, 0.0, 0.0),
                height: 500.0,
                depth: 1000.0,
            }),
            cx,
        )
    })
    .unwrap();
    let lines = out
        .lines
        .iter()
        .filter(|l| out.sources[l.source].element == behind)
        .collect::<Vec<_>>();
    assert!(!lines.is_empty());
    assert!(lines.iter().all(|l| l.role == Role::Hidden));
    assert_eq!(
        out.lines
            .iter()
            .filter(|l| out.sources[l.source].element == ElementRef(0))
            .count(),
        1
    );
}

#[test]
fn spatial_volume_does_not_hide_the_physical_elements_inside_it() {
    let mut b = cube();
    b.components.spatial.insert(
        ElementRef(0),
        Spatial {
            kind: SpatialKind::Space,
            elevation: None,
            ref_elevation: None,
        },
    );
    let inside = add(
        &mut b,
        data(
            vec![cuboid([200.0, 200.0, 0.0], [800.0, 800.0, 100.0])],
            &[0],
            "Body",
        ),
        "IfcSlab",
    );
    let out = with(|cx| {
        drawing::cut(
            &b,
            options(View::Plan {
                floor: point(0.0, 0.0, 0.0),
                height: 500.0,
                depth: 1000.0,
            }),
            cx,
        )
    })
    .unwrap();
    assert!(
        out.lines
            .iter()
            .any(|line| out.sources[line.source].element == inside && line.role == Role::Visible)
    );
    assert!(out.lines.iter().any(|line| out.sources[line.source].element == ElementRef(0) && line.role == Role::Cut));
}
