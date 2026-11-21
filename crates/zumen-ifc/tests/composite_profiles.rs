//! 直径 4 m の半円を 3 m 押し出し、円弧の向き、閉鎖、出所を 3D と平断面で照らす。
#![cfg(feature = "drawing")]
#![cfg(test)]

use zumen_base::{Budget, CancelToken, Cx, Diagnostics, Key, codes};
use zumen_building::{
    drawing::{self, Paper, Role, View},
    evaluation,
};
use zumen_crs::ProjectPoint;
use zumen_drawing::PaperLength;
use zumen_ifc::Ifc;
use zumen_sdk::Codec;

fn input() -> &'static str {
    "ISO-10303-21;
HEADER;
FILE_DESCRIPTION(('fixture'),'2;1');
FILE_NAME('fixture','',(),(),'','','');
FILE_SCHEMA(('IFC2X3'));
ENDSEC;
DATA;
#1=IFCPROJECT('0000000000000000000001',#7,$,$,$,$,$,(#8),#2);
#2=IFCUNITASSIGNMENT((#3,#4));
#3=IFCSIUNIT(*,.LENGTHUNIT.,$,.METRE.);
#4=IFCSIUNIT(*,.PLANEANGLEUNIT.,$,.RADIAN.);
#7=IFCOWNERHISTORY($,$,$,.ADDED.,$,$,$,0);
#8=IFCGEOMETRICREPRESENTATIONCONTEXT($,'Model',3,0.000001,#11,$);
#10=IFCCARTESIANPOINT((0.,0.,0.));
#11=IFCAXIS2PLACEMENT3D(#10,$,$);
#12=IFCDIRECTION((0.,0.,1.));
#13=IFCLOCALPLACEMENT($,#11);
#14=IFCCARTESIANPOINT((0.,0.));
#15=IFCAXIS2PLACEMENT2D(#14,$);
#16=IFCCIRCLE(#15,2.);
#17=IFCTRIMMEDCURVE(#16,(IFCPARAMETERVALUE(0.)),(IFCPARAMETERVALUE(3.141592653589793)),.T.,.PARAMETER.);
#18=IFCCARTESIANPOINT((2.,0.));
#19=IFCCARTESIANPOINT((-2.,0.));
#20=IFCPOLYLINE((#18,#19));
#21=IFCCOMPOSITECURVESEGMENT(.CONTINUOUS.,.T.,#17);
#22=IFCCOMPOSITECURVESEGMENT(.CONTINUOUS.,.F.,#20);
#23=IFCCOMPOSITECURVE((#21,#22),.F.);
#24=IFCARBITRARYCLOSEDPROFILEDEF(.AREA.,$,#23);
#25=IFCEXTRUDEDAREASOLID(#24,#11,#12,3.);
#26=IFCSHAPEREPRESENTATION(#8,'Body','SweptSolid',(#25));
#27=IFCPRODUCTDEFINITIONSHAPE($,$,(#26));
#28=IFCFURNISHINGELEMENT('0000000000000000000002',#7,$,$,$,#13,#27,$);
ENDSEC;
END-ISO-10303-21;"
}

fn check(input: &str, side: Option<f64>) {
    let (mut budget, mut diag, cancel) =
        (Budget::default(), Diagnostics::new(), CancelToken::new());
    let mut cx = Cx::new(&mut budget, &mut diag, &cancel);
    let file = Ifc::read(input.as_bytes(), &mut cx).unwrap();
    let building = file.lift_geometry(&mut cx).unwrap();
    let meshes = evaluation::evaluate(&building, evaluation::Options::default(), &mut cx).unwrap();
    let furniture = meshes
        .iter()
        .find(|m| m.id.to_string() == "0000000000000000000002")
        .unwrap();
    assert_eq!(furniture.complete, side.is_some());
    if let Some(side) = side {
        let [part] = furniture.parts.as_slice() else {
            panic!("家具の形の件数が違う")
        };
        // 半円柱の体積は 6π。弦で近似した底面がこの値に収まることを確かめる。
        assert!((part.mesh.volume() - 6. * core::f64::consts::PI).abs() < 0.02);
        let mut min = [f64::INFINITY; 3];
        let mut max = [f64::NEG_INFINITY; 3];
        for point in &part.mesh.positions {
            let [x, y, z] = part.placement.point(*point);
            for (i, v) in [x, side * y, z].into_iter().enumerate() {
                min[i] = min[i].min(v);
                max[i] = max[i].max(v);
            }
        }
        for (i, (lo, hi)) in [(-2., 2.), (0., 2.), (0., 3.)].into_iter().enumerate() {
            assert!(
                (min[i] - lo).abs() <= 0.001 && (max[i] - hi).abs() <= 0.001,
                "{min:?} {max:?}"
            );
        }
    } else {
        assert!(furniture.parts.is_empty());
    }
    let plan = drawing::cut(
        &building,
        drawing::Options {
            view: View::Plan {
                floor: ProjectPoint {
                    x: 0.,
                    y: 0.,
                    z: 0.,
                },
                height: 1.2,
                depth: 2.,
            },
            tolerance: 0.00001,
            chord: 0.02,
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
    assert_eq!(plan.complete, side.is_some(), "{diag:?}");
    if let Some(side) = side {
        let index = plan
            .sources
            .iter()
            .position(|s| {
                s.global_id.as_deref() == Some("0000000000000000000002")
                    && s.instance == Some(28)
                    && s.shape_instance == Some(25)
            })
            .unwrap();
        let mut min = [f64::INFINITY; 2];
        let mut max = [f64::NEG_INFINITY; 2];
        for line in plan
            .lines
            .iter()
            .filter(|l| l.source == index && l.role == Role::Cut)
        {
            let zumen_drawing::Shape::Line { start, end } =
                plan.drawing.spaces[0].elements[line.index].shape
            else {
                panic!("断面線の種類が違う")
            };
            for p in [start, end] {
                for (axis, value) in [p.x, side * p.y].into_iter().enumerate() {
                    min[axis] = min[axis].min(value);
                    max[axis] = max[axis].max(value);
                }
            }
        }
        assert!((max[0] - min[0] - 4.).abs() < 1e-5, "{min:?} {max:?}");
        assert!(
            min[1].abs() < 1e-5 && (max[1] - 2.).abs() <= 0.021,
            "{min:?} {max:?}"
        );
    } else {
        assert!(plan.lines.is_empty());
        assert!(diag.iter().any(|d| {
            d.code() == codes::BUILDING_SHAPE_DROPPED
                && d.places().iter().any(|p| {
                    p.id() == Some("0000000000000000000002") && p.key() == Some(Key::Instance(25))
                })
        }));
    }
}

#[test]
fn circular_trims_and_segment_direction_preserve_the_half_disk() {
    check(input(), Some(1.));
    check(
        &input().replace(".T.,.PARAMETER.", ".F.,.PARAMETER."),
        Some(-1.),
    );
    check(
        &input().replace(
            "(IFCPARAMETERVALUE(0.)),(IFCPARAMETERVALUE(3.141592653589793)),.T.,.PARAMETER.",
            "(#18),(#19),.T.,.CARTESIAN.",
        ),
        Some(1.),
    );
    let backwards = input()
        .replace("((#18,#19))", "((#19,#18))")
        .replace(".T.,#17", ".F.,#17");
    check(&backwards, Some(1.));
    let trimmed_line = "#29=IFCDIRECTION((1.,0.));
#30=IFCVECTOR(#29,1.);
#34=IFCLINE(#19,#30);
#20=IFCTRIMMEDCURVE(#34,(IFCPARAMETERVALUE(4.)),(IFCPARAMETERVALUE(0.)),.F.,.PARAMETER.);";
    check(
        &input().replace("#20=IFCPOLYLINE((#18,#19));", trimmed_line),
        Some(1.),
    );
    let degree = "#31=IFCDIMENSIONALEXPONENTS(0,0,0,0,0,0,0);
#32=IFCMEASUREWITHUNIT(IFCPLANEANGLEMEASURE(0.017453292519943295),#4);
#33=IFCCONVERSIONBASEDUNIT(#31,.PLANEANGLEUNIT.,'degree',#32);
#7=IFCOWNER";
    let degrees = input()
        .replace("((#3,#4))", "((#3,#33))")
        .replace("#7=IFCOWNER", degree)
        .replace("3.141592653589793", "180.");
    check(&degrees, Some(1.));
}

#[test]
fn a_gap_off_circle_trimming_point_or_unbounded_segment_has_no_outline() {
    check(&input().replace("((-2.,0.))", "((-1.7,0.))"), None);
    check(
        &input()
            .replace(
                "(IFCPARAMETERVALUE(0.)),(IFCPARAMETERVALUE(3.141592653589793)),.T.,.PARAMETER.",
                "(#18),(#19),.T.,.CARTESIAN.",
            )
            .replace("((-2.,0.))", "((-1.7,0.))"),
        None,
    );
    check(
        &input().replace("(IFCPARAMETERVALUE(0.))", "(#19,IFCPARAMETERVALUE(0.))"),
        None,
    );
    check(&input().replace(".T.,#17", ".T.,#16"), None);
}

#[test]
fn an_explicit_break_is_rejected_at_the_source_curve() {
    let input = input().replace(".CONTINUOUS.,.F.,#20", ".DISCONTINUOUS.,.F.,#20");
    let (mut budget, mut diag, cancel) =
        (Budget::default(), Diagnostics::new(), CancelToken::new());
    let mut cx = Cx::new(&mut budget, &mut diag, &cancel);
    let file = Ifc::read(input.as_bytes(), &mut cx).unwrap();
    assert!(matches!(
        file.lift_geometry(&mut cx),
        Err(zumen_sdk::Error::Invalid)
    ));
    assert!(
        diag.iter()
            .any(|d| (d.code() == codes::IFC_EXTRUSION_INVALID
                || d.code() == codes::IFC_SHAPE_INVALID)
                && d.places()
                    .iter()
                    .any(|p| p.id() == Some("i23") && p.bytes().is_some())),
        "{diag:?}"
    );
}
