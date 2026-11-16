//! 公開 IFC の代表階を切り、壁の線から建物 ID・GlobalId・STEP と開口へ戻る。
#![cfg(feature = "drawing")]
use zumen_base::{Budget, CancelToken, Cx, Diagnostics, Kind, Limit, Limits};
use zumen_building::{
    Opening,
    drawing::{self, Options, Paper, Role, View},
};
use zumen_crs::ProjectPoint;
use zumen_drawing::{PaperLength, Shape};
use zumen_ifc::Ifc;
use zumen_sdk::Codec;

#[test]
fn public_second_floor_traces_a_wall_and_its_openings() {
    let bytes = include_bytes!("../../../samples/ifc/rinya/wood-office-architecture.ifc");
    let mut budget = Budget::new(
        Limits::default()
            .with(Kind::DecodedBytes, Limit::Finite(16 * 1024 * 1024 * 1024))
            .with(Kind::Work, Limit::Finite(10_000_000_000)),
    );
    let (mut diag, cancel) = (Diagnostics::new(), CancelToken::new());
    let mut cx = Cx::new(&mut budget, &mut diag, &cancel);
    let file = Ifc::read(bytes, &mut cx).unwrap();
    let building = file.lift_geometry(&mut cx).unwrap();
    let out = drawing::cut(
        &building,
        Options {
            view: View::Plan {
                floor: ProjectPoint {
                    x: 0.0,
                    y: 0.0,
                    z: 4300.0,
                },
                height: 1200.0,
                depth: 2000.0,
            },
            tolerance: 0.01,
            chord: 1.0,
            paper: Paper {
                denominator: 100.0,
                origin: [PaperLength(0.0); 2],
                size: None,
            },
            symbols: &[],
        },
        &mut cx,
    )
    .unwrap();
    let wall = out
        .sources
        .iter()
        .position(|s| s.instance == Some(13256) && s.shape_instance == Some(13253))
        .unwrap();
    let source = &out.sources[wall];
    assert_eq!(source.global_id.as_deref(), Some("3hYgN0Ix5829GlT_ttwWPW"));
    assert_eq!(source.id.to_string(), "3hYgN0Ix5829GlT_ttwWPW");
    let line = out
        .lines
        .iter()
        .find(|l| l.source == wall && l.role == Role::Cut)
        .unwrap();
    assert!(line.id.starts_with("building/3hYgN0Ix5829GlT_ttwWPW/cut/"));
    assert!(matches!(
        out.drawing.spaces[0].elements[line.index].shape,
        Shape::Line { .. }
    ));
    assert!(out.drawing.extensions[line.index].is_current(&out.drawing));
    let opening = building
        .components
        .opening
        .iter()
        .find_map(|(at, relation)| {
            (matches!(relation, Opening::Voids(host) if *host == source.element)
                && building
                    .element(at)
                    .is_some_and(|e| e.source().instance == Some(27507)))
            .then_some(at)
        })
        .unwrap();
    let element = building.element(opening).unwrap();
    assert_eq!(
        element.source().global_id.as_deref(),
        Some("1ol1N$i7$nTs9PKPhumpBV")
    );
    assert_eq!(element.id().to_string(), "1ol1N$i7$nTs9PKPhumpBV");
    assert!(
        building
            .geometry(opening)
            .unwrap()
            .data()
            .nodes
            .iter()
            .any(|n| n.source.instance == Some(27501))
    );
    assert!(
        out.lines
            .iter()
            .all(|l| out.sources[l.source].element != opening)
    );
    assert_eq!(out.statistics.projection_failures, 0);
    assert_eq!(out.statistics.evaluation_failures, 0);
}
