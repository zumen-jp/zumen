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
fn explicit_door_attributes_reach_profile_symbols() {
    use zumen_profile::building_drawing::DrawingProfile;
    const BUILDING_DRAWING: &[u8] =
        include_bytes!("../../../profiles/zumen-building-drawing/2026-11/profile.json");
    for schema in ["IFC2X3", "IFC4", "IFC4X3_ADD2"] {
        let old = schema == "IFC2X3";
        let door_tail = if old {
            ""
        } else {
            ",.DOOR.,.SINGLE_SWING_RIGHT.,$"
        };
        let type_row = if old {
            "IFCDOORSTYLE('0000000000000000000011',$,$,$,$,$,$,$,.SINGLE_SWING_LEFT.,.NOTDEFINED.,.F.,.F.)"
        } else {
            "IFCDOORTYPE('0000000000000000000011',$,$,$,$,$,$,$,$,.DOOR.,.SINGLE_SWING_LEFT.,.F.,$)"
        };
        let bytes = format!("ISO-10303-21;HEADER;FILE_DESCRIPTION((''),'2;1');FILE_NAME('','',(''),(''),'','','');FILE_SCHEMA(('{schema}'));ENDSEC;DATA;
            #1=IFCPROJECT('0000000000000000000001',$,$,$,$,$,$,$,#2);
            #2=IFCUNITASSIGNMENT((#3));#3=IFCSIUNIT(*,.LENGTHUNIT.,.MILLI.,.METRE.);
            #10=IFCDOOR('0000000000000000000010',$,$,$,$,$,$,$,2100.,900.{door_tail});
            #11={type_row};#12=IFCRELDEFINESBYTYPE('0000000000000000000012',$,$,$,(#10),#11);
            ENDSEC;END-ISO-10303-21;");
        let (mut budget, mut diagnostics, cancel) =
            (Budget::default(), Diagnostics::new(), CancelToken::new());
        let mut cx = Cx::new(&mut budget, &mut diagnostics, &cancel);
        let file = Ifc::read(bytes.as_bytes(), &mut cx).unwrap();
        let building = file.lift_geometry(&mut cx).unwrap();
        let p = DrawingProfile::load(
            &[("base", BUILDING_DRAWING)],
            "zumen-building-drawing",
            "2026-11",
            &mut cx,
        )
        .unwrap();
        let view = p
            .default_plan(
                &building,
                ProjectPoint {
                    x: 0.0,
                    y: 0.0,
                    z: 0.0,
                },
                &mut cx,
            )
            .unwrap();
        let out = p.cut(&building, view, 0.01, 1.0, None, &mut cx).unwrap();
        assert!(!out.output.lines.is_empty());
        assert!(out.output.lines.iter().all(|l| l.role == Role::Annotation));
        // 型の左ヒンジを使い、occurrence にある右ヒンジで上書きしない。
        assert!(out.output.drawing.spaces[0].elements.iter().any(|e| matches!(e.shape, Shape::Line { start, end } if start.x == 0.0 && end.x == 0.0 && end.y == 900.0)));
        assert!(
            out.output
                .lines
                .iter()
                .all(|l| out.output.sources[l.source].instance == Some(10))
        );
    }
}

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
