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

#[test]
fn a_depth_clipped_upright_wall_keeps_its_outline_and_a_subgrid_wall_reports_loss() {
    // 小数の位置で深さをまたぐ直立の壁。幅だけを格子より細くした入力とも照らす。
    for (width, touching, complete) in [
        ("2.", false, true),
        ("2.", true, true),
        ("0.0001", false, false),
    ] {
        // 切断面より低い第 2 の部材は、平面では主材と一つの角だけで接する。
        let (extra, items) = if touching {
            (
                "#31=IFCCARTESIANPOINT((14.341,4.456));
#32=IFCAXIS2PLACEMENT2D(#31,$);
#33=IFCRECTANGLEPROFILEDEF(.AREA.,$,#32,2.,1.);
#34=IFCEXTRUDEDAREASOLID(#33,#11,#12,0.8);",
                "#20,#34",
            )
        } else {
            ("", "#20")
        };
        let input = format!(
            "ISO-10303-21;
HEADER;
FILE_DESCRIPTION(('plan'),'2;1');
FILE_NAME('plan','',(),(),'','','');
FILE_SCHEMA(('IFC4'));
ENDSEC;
DATA;
#1=IFCPROJECT('0000000000000000000001',$,$,$,$,$,$,(#8),#2);
#2=IFCUNITASSIGNMENT((#3));
#3=IFCSIUNIT(*,.LENGTHUNIT.,$,.METRE.);
#8=IFCGEOMETRICREPRESENTATIONCONTEXT($,'Model',3,0.000001,#11,$);
#10=IFCCARTESIANPOINT((0.,0.,0.));
#11=IFCAXIS2PLACEMENT3D(#10,$,$);
#12=IFCDIRECTION((0.,0.,1.));
#13=IFCLOCALPLACEMENT($,#11);
#14=IFCCARTESIANPOINT((12.341,3.456));
#15=IFCAXIS2PLACEMENT2D(#14,$);
#16=IFCRECTANGLEPROFILEDEF(.AREA.,$,#15,{width},1.);
#20=IFCEXTRUDEDAREASOLID(#16,#11,#12,3.);
{extra}
#21=IFCSHAPEREPRESENTATION(#8,'Body','SweptSolid',({items}));
#22=IFCPRODUCTDEFINITIONSHAPE($,$,(#21));
#23=IFCWALL('0000000000000000000002',$,$,$,$,#13,#22,$,$);
ENDSEC;
END-ISO-10303-21;"
        );
        let (mut budget, mut diagnostics, cancel) =
            (Budget::default(), Diagnostics::new(), CancelToken::new());
        let mut cx = Cx::new(&mut budget, &mut diagnostics, &cancel);
        let file = Ifc::read(input.as_bytes(), &mut cx).unwrap();
        let building = file.lift_geometry(&mut cx).unwrap();
        let output = drawing::cut(
            &building,
            Options {
                view: View::Plan {
                    floor: ProjectPoint {
                        x: 0.,
                        y: 0.,
                        z: 0.,
                    },
                    height: 1.2,
                    depth: 2.,
                },
                tolerance: 0.01,
                chord: 0.001,
                paper: Paper {
                    denominator: 100.,
                    origin: [PaperLength(0.); 2],
                    size: None,
                },
                symbols: &[],
            },
            &mut cx,
        )
        .unwrap();
        assert_eq!(output.complete, complete, "{diagnostics:?}");
        assert_eq!(output.statistics.collapsed == 0, complete);
        assert_eq!(output.statistics.evaluation_failures, 0);
        assert_eq!(output.statistics.projection_failures, 0);
        assert!(!output.lines.is_empty());
        if !complete {
            assert!(diagnostics.iter().any(|d| {
                d.code() == zumen_base::codes::BUILDING_LINE_DROPPED
                    && d.places()
                        .iter()
                        .any(|p| p.id() == Some("0000000000000000000002"))
            }));
        }
    }
}
