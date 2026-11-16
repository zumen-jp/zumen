//! 建物の選択と開口・配置・共有・単位・停止の契約。
mod support;
use support::*;
use zumen_base::{Budget, CancelToken, Cx, Diagnostics, Kind, Limit, Limits, codes};
use zumen_building::{self as b, *};
use zumen_scene3d::{Error, Options, build, encode, read};

#[test]
fn maps_share_meshes_keep_model_ids_and_use_document_units() {
    let b = building();
    let before = b.clone();
    let scene = with(|cx| {
        build(
            &b,
            Options {
                tolerance: Some(0.1),
                ..Options::default()
            },
            cx,
        )
    })
    .unwrap();
    assert_eq!(b, before);
    assert_eq!(scene.data.meshes.len(), 1);
    assert_eq!(scene.data.instances.len(), 3);
    assert_eq!(scene.data.chunks.len(), 1);
    assert_eq!(scene.data.elements.len(), 3);
    let mesh = &scene.data.meshes[0];
    assert!(mesh.vertices.iter().any(|p| p[0] == 500.0));
    assert_eq!(mesh.triangles.len(), 12);
    assert_eq!(mesh.edges.len(), 12);
    assert_eq!(scene.data.elements[1].name, "壁");
    assert_eq!(scene.data.elements[1].storey, Some(0));
    assert_eq!(scene.data.elements[0].storey, None);
    assert_eq!(scene.data.colors[0].rgba, [0.25, 0.5, 1.0, 0.75]);
    assert!(scene.data.colors[1].unresolved);
    assert!(scene.data.chunks[0].origin[0] > 1e11);
    for element in &scene.data.elements {
        assert!(b.find(element.id.parse().unwrap()).is_some());
    }
    with(|cx| {
        let blob = encode(&scene.data, cx).unwrap();
        let reader = read(&blob, cx).unwrap();
        assert_eq!(reader.element(1).unwrap().id, "i2");
        assert_eq!(
            reader.instance(0).unwrap().mesh,
            reader.instance(2).unwrap().mesh
        );
    });
}

#[test]
fn filters_intersect_and_retain_referenced_storeys_and_missing_geometry() {
    let mut b = building();
    let missing = add(&mut b, "IfcDoor", None);
    let ids = [b.id(ElementRef(1)).unwrap(), b.id(missing).unwrap()];
    let classes = ["IfcWall".to_string()];
    let floors = [b.id(ElementRef(0)).unwrap()];
    let options = Options {
        ids: Some(&ids),
        classes: Some(&classes),
        storeys: Some(&floors),
        tolerance: Some(0.1),
        ..Options::default()
    };
    let scene = with(|cx| build(&b, options, cx)).unwrap();
    assert_eq!(
        scene
            .data
            .elements
            .iter()
            .map(|e| e.id.as_str())
            .collect::<Vec<_>>(),
        ["i1", "i2"]
    );
    assert_eq!(scene.data.instances.len(), 2);
    let empty = with(|cx| {
        build(
            &b,
            Options {
                ids: Some(&[]),
                ..options
            },
            cx,
        )
    })
    .unwrap();
    assert!(empty.data.elements.is_empty());
    let boxed = with(|cx| {
        build(
            &b,
            Options {
                region: Some([[1e12 - 100.0, 1900.0, 0.0], [1e12 + 100.0, 2100.0, 1000.0]]),
                tolerance: Some(0.1),
                ..Options::default()
            },
            cx,
        )
    })
    .unwrap();
    assert_eq!(boxed.data.instances.len(), 2); // 要素単位。箱の外にある同じ要素の写像も保つ。
    assert!(boxed.data.elements.iter().any(|e| e.id == "i4"));
    assert!(!boxed.data.elements.iter().any(|e| e.id == "i3"));
}

#[test]
fn chunks_never_split_an_element_even_above_the_target_size() {
    let b = building();
    let scene = with(|cx| {
        build(
            &b,
            Options {
                chunk_instances: 1,
                tolerance: Some(0.1),
                ..Options::default()
            },
            cx,
        )
    })
    .unwrap();
    assert_eq!(scene.data.chunks.len(), 2);
    assert_eq!(scene.data.chunks[1].instances, 2);
    with(|cx| {
        let blob = encode(&scene.data, cx).unwrap();
        read(&blob, cx).unwrap();
    });
}

#[test]
fn openings_remove_volume_and_do_not_fill_the_hole_in_the_scene() {
    let mut b = building();
    let opening = add(
        &mut b,
        "IfcOpeningElement",
        Some(geometry(
            AreaProfileKind::Rectangle {
                x_dim: 500.0,
                y_dim: 400.0,
            },
            1,
        )),
    );
    b.components
        .opening
        .insert(opening, Opening::Voids(ElementRef(1)));
    b.components.placement.insert(
        opening,
        Placement {
            relative_to: None,
            local: Local::Transform(Transform::translation(1e12, 2000.0, 0.0)),
        },
    );
    let scene = with(|cx| {
        build(
            &b,
            Options {
                tolerance: Some(0.1),
                ..Options::default()
            },
            cx,
        )
    })
    .unwrap();
    assert_eq!(scene.data.elements.len(), 4);
    assert_eq!(scene.data.instances.len(), 3);
    assert!(scene.data.instances.iter().all(|i| i.element != 3));
    let cut = &scene.data.meshes[scene.data.instances[0].mesh as usize];
    let mesh = zumen_geom3d::Mesh {
        positions: cut.vertices.iter().map(|p| p.map(f64::from)).collect(),
        triangles: cut.triangles.clone(),
        edges: cut.edges.clone(),
    };
    assert!((mesh.volume() - 100_000_000.0).abs() < 1.0);
}

#[test]
fn tolerance_controls_curves_and_default_comes_from_selected_extent() {
    let b = building();
    let scene = with(|cx| build(&b, Options::default(), cx)).unwrap();
    let diagonal = libm::hypot(libm::hypot(7000.0, 200.0), 1000.0);
    assert_eq!(scene.tolerance, diagonal / 50000.0);
    let mut b = Building::new(Document {
        units: vec![unit()],
        ..Document::default()
    });
    add(
        &mut b,
        "IfcColumn",
        Some(geometry(AreaProfileKind::Circle { radius: 1000.0 }, 1)),
    );
    let fine = with(|cx| {
        build(
            &b,
            Options {
                tolerance: Some(0.1),
                ..Options::default()
            },
            cx,
        )
    })
    .unwrap();
    let coarse = with(|cx| {
        build(
            &b,
            Options {
                tolerance: Some(10.0),
                ..Options::default()
            },
            cx,
        )
    })
    .unwrap();
    assert!(fine.data.meshes[0].vertices.len() > coarse.data.meshes[0].vertices.len());
}

#[test]
fn budget_cancel_invalid_units_and_unsupported_shapes_have_no_partial_result() {
    let mut b = building();
    for kind in [
        Kind::Work,
        Kind::DecodedBytes,
        Kind::Segments,
        Kind::Depth,
        Kind::Elements,
    ] {
        let mut budget = Budget::new(Limits::default().with(kind, Limit::Finite(0)));
        let mut diagnostics = Diagnostics::new();
        let cancel = CancelToken::new();
        assert!(matches!(
            build(
                &b,
                Options::default(),
                &mut Cx::new(&mut budget, &mut diagnostics, &cancel)
            ),
            Err(Error::Stopped(_))
        ));
    }
    let cancel = CancelToken::new();
    cancel.cancel();
    assert!(matches!(
        build(
            &b,
            Options::default(),
            &mut Cx::new(&mut Budget::default(), &mut Diagnostics::new(), &cancel)
        ),
        Err(Error::Stopped(_))
    ));
    let mut input = geometry(
        AreaProfileKind::Rectangle {
            x_dim: 1000.0,
            y_dim: 200.0,
        },
        1,
    );
    input.nodes[0].definition = GeometryDefinition::Unsupported("synthetic".into());
    add(&mut b, "IfcMember", Some(input));
    let mut diagnostics = Diagnostics::new();
    let scene = build(
        &b,
        Options {
            tolerance: Some(0.1),
            ..Options::default()
        },
        &mut Cx::new(
            &mut Budget::default(),
            &mut diagnostics,
            &CancelToken::new(),
        ),
    )
    .unwrap();
    assert_eq!(scene.data.elements.len(), 4);
    assert!(
        diagnostics
            .iter()
            .any(|d| d.code() == codes::BUILDING_SHAPE_DROPPED)
    );
    b.document.units.clear();
    assert!(matches!(
        with(|cx| build(&b, Options::default(), cx)),
        Err(Error::Invalid("unit_missing"))
    ));
    assert_eq!(b::evaluation::length_scale(&unit()).unwrap(), 0.001);
}
