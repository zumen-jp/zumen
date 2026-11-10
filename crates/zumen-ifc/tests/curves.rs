//! 必要な曲線だけを写す。入力は、テストで作るものと再配布できる公開サンプルだけ。
#![cfg(feature = "building")]
#![cfg(test)]

use zumen_base::{Budget, CancelToken, Cx, Diagnostics, Kind, Limit, Limits, codes};
use zumen_building::{
    Building, Coordinates, CurveGraph, CurveKind, CurveRef, CurveTransition, CurveTrim, Hash,
    IndexedSegment, TrimmingPreference,
};
use zumen_ifc::{Ifc, IfcFile};
use zumen_json::Writer;
use zumen_sdk::{Codec, Error, Lift};

const VERSIONS: [&str; 3] = ["IFC2X3", "IFC4", "IFC4X3_ADD2"];

fn source(schema: &str, extra: &str) -> String {
    format!(
        "ISO-10303-21;\nHEADER;\nFILE_DESCRIPTION(('curves'),'2;1');\nFILE_NAME('curves','',(),(),'','','');\nFILE_SCHEMA(('{schema}'));\nENDSEC;\nDATA;\n\
        #1=IFCPROJECT('0000000000000000000001',$,$,$,$,$,$,$,#2);\n\
        #2=IFCUNITASSIGNMENT((#3,#4));\n\
        #3=IFCSIUNIT(*,.LENGTHUNIT.,.MILLI.,.METRE.);\n\
        #4=IFCCONVERSIONBASEDUNIT(#8,.PLANEANGLEUNIT.,'degree',#5);\n\
        #5=IFCMEASUREWITHUNIT(IFCPLANEANGLEMEASURE(0.017453292519943295),#6);\n\
        #6=IFCSIUNIT(*,.PLANEANGLEUNIT.,$,.RADIAN.);\n\
        #8=IFCDIMENSIONALEXPONENTS(0,0,0,0,0,0,0);\n\
        #10=IFCCARTESIANPOINT((0.,0.));\n\
        #11=IFCCARTESIANPOINT((5.,0.));\n\
        #12=IFCCARTESIANPOINT((0.,5.));\n\
        #20=IFCDIRECTION((2.,0.));\n\
        #21=IFCVECTOR(#20,4.);\n\
        #22=IFCLINE(#10,#21);\n\
        #23=IFCAXIS2PLACEMENT2D(#10,#20);\n\
        #24=IFCCIRCLE(#23,5.);\n\
        #25=IFCTRIMMEDCURVE(#24,(IFCPARAMETERVALUE(0.),#11),(IFCPARAMETERVALUE(90.),#12),.F.,.PARAMETER.);\n\
        #26=IFCTRIMMEDCURVE(#22,(IFCPARAMETERVALUE(0.)),(IFCPARAMETERVALUE(2.)),.T.,.UNSPECIFIED.);\n\
        #30=IFCPOLYLINE((#10,#11,#12,#10));\n\
        #40=IFCCOMPOSITECURVESEGMENT(.CONTINUOUS.,.T.,#30);\n\
        #41=IFCCOMPOSITECURVESEGMENT(.DISCONTINUOUS.,.F.,#30);\n\
        #42=IFCCOMPOSITECURVE((#40,#41),.U.);\n\
        #60=IFCCARTESIANPOINT((0.,0.,4.));\n\
        #61=IFCDIRECTION((0.,0.,2.));\n\
        #62=IFCDIRECTION((1.,0.,0.));\n\
        #63=IFCAXIS2PLACEMENT3D(#60,#61,#62);\n\
        #64=IFCCIRCLE(#63,2.);\n\
        #70=IFCELLIPSE(#23,2.,3.);\n\
        {}\n{extra}\nENDSEC;\nEND-ISO-10303-21;\n",
        if schema == "IFC2X3" {
            ""
        } else {
            "#50=IFCCARTESIANPOINTLIST2D(((0.,0.),(5.,0.),(0.,5.),(0.,0.)));\n#51=IFCINDEXEDPOLYCURVE(#50,(IFCLINEINDEX((1,2)),IFCARCINDEX((2,3,4))),.F.);"
        }
    )
}

fn with<T>(limits: Limits, f: impl FnOnce(&mut Cx) -> T) -> (T, Diagnostics, Budget) {
    let mut budget = Budget::new(limits);
    let mut diag = Diagnostics::new();
    let cancel = CancelToken::new();
    let value = f(&mut Cx::new(&mut budget, &mut diag, &cancel));
    (value, diag, budget)
}

fn read(src: &str) -> IfcFile {
    with(Limits::default(), |cx| Ifc::read(src.as_bytes(), cx))
        .0
        .unwrap()
}

fn graph(file: &IfcFile, id: u64) -> CurveGraph {
    let (value, diag, _) = with(Limits::default(), |cx| file.curve(id, cx));
    assert!(diag.is_empty(), "{diag:?}");
    value.unwrap()
}

#[test]
fn keeps_units_dimensions_definitions_and_sources_in_three_versions() {
    for schema in VERSIONS {
        let file = read(&source(schema, ""));
        let line = graph(&file, 22);
        assert_eq!(
            line.item(line.root).unwrap().kind,
            CurveKind::Line {
                point: Coordinates::Two([0., 0.]),
                direction: Coordinates::Two([2., 0.]),
                magnitude: 4.,
            }
        );
        assert_eq!(line.items[0].source.instance, Some(22));
        assert_eq!(line.items[0].source.records, [10, 21, 20]);
        assert_eq!(
            line.length_unit.as_ref().unwrap().prefix.as_deref(),
            Some("MILLI")
        );
        let circle = graph(&file, 64);
        let CurveKind::Circle { position, radius } = &circle.items[0].kind else {
            panic!()
        };
        assert_eq!(*radius, 2.);
        assert_eq!(position.location, Coordinates::Three([0., 0., 4.]));
        assert_eq!(position.axis, Some([0., 0., 2.]));
        assert_eq!(
            position.reference_direction,
            Some(Coordinates::Three([1., 0., 0.]))
        );
        assert_eq!(circle.items[0].source.records, [63, 60, 61, 62]);
        let trimmed = graph(&file, 25);
        let CurveKind::Trimmed {
            basis,
            trim1,
            trim2,
            sense_agreement,
            preference,
        } = &trimmed.items[0].kind
        else {
            panic!()
        };
        assert_eq!(trimmed.item(*basis).unwrap().source.instance, Some(24));
        assert_eq!(
            trim1,
            &[
                CurveTrim::Parameter(0.),
                CurveTrim::Point(Coordinates::Two([5., 0.]))
            ]
        );
        assert_eq!(
            trim2,
            &[
                CurveTrim::Parameter(90.),
                CurveTrim::Point(Coordinates::Two([0., 5.]))
            ]
        );
        assert!(!sense_agreement);
        assert_eq!(*preference, TrimmingPreference::Parameter);
        assert_eq!(trimmed.angle_unit.as_ref().unwrap().name, "degree");
        assert_eq!(
            trimmed.angle_unit.as_ref().unwrap().scale,
            Some(0.017453292519943295)
        );
        assert_eq!(trimmed.items[0].source.records, [11, 12]);
        assert_eq!(trimmed.items[1].source.records, [23, 10, 20]);
        let composite = graph(&file, 42);
        let CurveKind::Composite {
            segments,
            self_intersect,
        } = &composite.items[0].kind
        else {
            panic!()
        };
        assert_eq!(composite.items.len(), 2);
        assert_eq!(segments[0].curve, segments[1].curve);
        assert!(segments[0].same_sense);
        assert!(!segments[1].same_sense);
        assert_eq!(segments[1].transition, CurveTransition::Discontinuous);
        assert_eq!(*self_intersect, None);
        assert_eq!(composite.items[0].source.records, [40, 41]);
        assert_eq!(composite.items[1].source.records, [10, 11, 12, 10]);
        let CurveKind::Polyline(points) = &composite.items[1].kind else {
            panic!()
        };
        assert_eq!(points.len(), 4);
        assert_eq!(points.first(), points.last());
        assert_eq!(composite.item(CurveRef(u32::MAX)), None);
        if schema != "IFC2X3" {
            let indexed = graph(&file, 51);
            let CurveKind::Indexed {
                points,
                segments,
                self_intersect,
            } = &indexed.items[0].kind
            else {
                panic!()
            };
            assert_eq!(points.len(), 4);
            assert_eq!(
                *segments,
                Some(vec![
                    IndexedSegment::Line(vec![1, 2]),
                    IndexedSegment::Arc([2, 3, 4])
                ])
            );
            assert_eq!(*self_intersect, Some(false));
            assert_eq!(indexed.items[0].source.records, [50]);
        }
    }
}

#[test]
fn keeps_radian_values_and_reports_unknown_units_without_assuming_degrees() {
    let src = source("IFC4", "").replace("(#3,#4)", "(#3,#6)");
    let curve = graph(&read(&src), 25);
    assert_eq!(curve.angle_unit.unwrap().name, "RADIAN");
    let file = read(&src.replace("(#3,#6)", "(#3)"));
    let (curve, diag, _) = with(Limits::default(), |cx| file.curve(25, cx));
    assert_eq!(curve.unwrap().angle_unit, None);
    assert!(diag.iter().any(|d| d.code() == codes::IFC_UNIT_MISSING));
}

#[test]
fn reports_unsupported_kinds_and_rejects_invalid_values_without_partial_graphs() {
    let file = read(&source("IFC4", ""));
    let (curve, diag, _) = with(Limits::default(), |cx| file.curve(70, cx));
    let curve = curve.unwrap();
    assert_eq!(curve.items[0].source.instance, Some(70));
    assert_eq!(
        curve.items[0].kind,
        CurveKind::Unsupported("IFCELLIPSE".to_owned())
    );
    assert!(
        diag.iter()
            .any(|d| d.code() == codes::IFC_CURVE_UNSUPPORTED)
    );
    for definition in [
        "IFCPOLYLINE((#10))",
        "IFCPOLYLINE((#10,#60))",
        "IFCPOLYLINE((#10,#999))",
        "IFCCIRCLE(#23,-1.)",
        "IFCCIRCLE(#23,1.E999)",
        "IFCLINE(#10,#20)",
        "IFCINDEXEDPOLYCURVE(#50,(IFCLINEINDEX((0,2))),.F.)",
        "IFCINDEXEDPOLYCURVE(#50,(IFCARCINDEX((1,2))),.F.)",
        "IFCINDEXEDPOLYCURVE(#50,$,.U.)",
        "IFCINDEXEDPOLYCURVE(#50,(IFCLINEINDEX((1,2),3)),.F.)",
        "IFCINDEXEDPOLYCURVE(#50,(IFCLINEINDEX((1,2)),IFCLINEINDEX((3,4))),.F.)",
        "IFCTRIMMEDCURVE(#24,(),(IFCPARAMETERVALUE(1.)),.T.,.PARAMETER.)",
        "IFCTRIMMEDCURVE(#24,(IFCPARAMETERVALUE(0.),IFCPARAMETERVALUE(1.)),(#12),.T.,.PARAMETER.)",
        "IFCCOMPOSITECURVE((#91),.F.)",
        "IFCCOMPOSITECURVE((#40),$)",
    ] {
        let file = read(&source(
            "IFC4",
            &format!("#90={definition};\n#91=IFCCOMPOSITECURVESEGMENT(.CONTINUOUS.,.T.,#90);"),
        ));
        let (curve, diag, budget) = with(Limits::default(), |cx| file.curve(90, cx));
        assert_eq!(curve, Err(Error::Invalid), "{definition}");
        assert!(
            diag.iter().any(|d| d.code() == codes::IFC_CURVE_INVALID),
            "{definition}"
        );
        assert_eq!(budget.used(Kind::Depth), 0);
    }
    let (curve, diag, _) = with(Limits::default(), |cx| file.curve(999, cx));
    assert_eq!(curve, Err(Error::Invalid));
    assert!(diag.iter().any(|d| d.code() == codes::IFC_CURVE_INVALID));
    assert_eq!(
        with(Limits::default(), |cx| read(&source("IFC4X2", ""))
            .curve(22, cx))
        .0,
        Err(Error::Unsupported)
    );
}

#[test]
fn mutated_curve_inputs_finish_with_repeatable_diagnostics_and_budget_usage() {
    let src = source("IFC4", "");
    for at in (0..src.len()).step_by(31) {
        let mut input = src.as_bytes().to_vec();
        input[at] = b'$';
        zumen_sdk::testkit::survives_with(&input, |src, cx| {
            let file = Ifc::read(src, cx)?;
            for id in [22, 25, 30, 42, 51, 64] {
                if let Err(Error::Stopped(stop)) = file.curve(id, cx) {
                    return Err(Error::Stopped(stop));
                }
            }
            Ok(())
        })
        .unwrap();
    }
    let src = src.replace(
        "IFCPLANEANGLEMEASURE(0.017453292519943295)",
        "IFCPLANEANGLEMEASURE(IFCPLANEANGLEMEASURE(1.))",
    );
    assert_eq!(
        with(Limits::default(), |cx| read(&src).curve(25, cx)).0,
        Err(Error::Invalid)
    );
}

#[test]
fn stops_on_budgets_and_cancel_and_restores_depth() {
    let file = read(&source("IFC4", ""));
    for kind in [Kind::Work, Kind::DecodedBytes, Kind::Depth] {
        let (curve, _, budget) = with(Limits::default().with(kind, Limit::Finite(0)), |cx| {
            file.curve(42, cx)
        });
        assert!(matches!(curve, Err(Error::Stopped(_))));
        assert_eq!(budget.used(Kind::Depth), 0);
        if kind == Kind::DecodedBytes {
            assert_eq!(budget.used(kind), 0);
        }
    }
    let (curve, _, budget) = with(Limits::default(), |cx| {
        cx.cancel.cancel();
        file.curve(42, cx)
    });
    assert_eq!(curve, Err(Error::Stopped(zumen_base::Stop::Cancelled)));
    assert_eq!(budget.used(Kind::DecodedBytes), 0);
    let (_, _, budget) = with(Limits::default(), |cx| file.curve(42, cx));
    let used = budget.used(Kind::DecodedBytes);
    assert!(used > 0);
    assert!(
        with(
            Limits::default().with(Kind::DecodedBytes, Limit::Finite(used - 1)),
            |cx| file.curve(42, cx)
        )
        .0
        .is_err()
    );
    assert!(
        with(
            Limits::default().with(Kind::DecodedBytes, Limit::Finite(used)),
            |cx| file.curve(42, cx)
        )
        .0
        .is_ok()
    );
}

#[test]
#[cfg(not(target_arch = "wasm32"))]
fn deep_curves_are_flat_and_do_not_recurse_when_read_cloned_compared_or_dropped() {
    std::thread::Builder::new().stack_size(1 << 20).spawn(|| {
        let mut extra = String::new();
        for n in 1000..2024 {
            extra.push_str(&format!("#{n}=IFCTRIMMEDCURVE(#{},(IFCPARAMETERVALUE(0.)),(IFCPARAMETERVALUE(1.)),.T.,.PARAMETER.);\n",if n == 2023 {22} else {n+1}));
        }
        let file = read(&source("IFC4",&extra));
        let (curve,_,budget) = with(Limits::default().with(Kind::Depth,Limit::Unlimited),|cx|file.curve(1000,cx));
        let curve = curve.unwrap();
        assert_eq!(curve.items.len(),1025);
        assert_eq!(curve,curve.clone());
        assert_eq!(budget.used(Kind::Depth),0);
        assert!(matches!(with(Limits::default(),|cx|file.curve(1000,cx)).0,Err(Error::Stopped(_))));
    }).unwrap().join().unwrap();
}

#[test]
fn retrieval_order_renumbering_and_changes_keep_sources_separate_from_meaning_and_existing_hashes()
{
    let src = source("IFC4", "");
    let file = read(&src);
    let building: Building = with(Limits::default(), |cx| file.lift(cx)).0.unwrap();
    let hashes: Vec<_> = building
        .elements()
        .iter()
        .enumerate()
        .map(|(i, e)| {
            (
                e.id(),
                Hash::element(&building, zumen_building::ElementRef(i as u32)),
            )
        })
        .collect();
    let first = graph(&file, 25);
    graph(&file, 42);
    graph(&file, 22);
    assert_eq!(first, graph(&file, 25));
    let again: Building = with(Limits::default(), |cx| file.lift(cx)).0.unwrap();
    assert_eq!(building, again);
    assert_eq!(
        hashes,
        again
            .elements()
            .iter()
            .enumerate()
            .map(|(i, e)| (
                e.id(),
                Hash::element(&again, zumen_building::ElementRef(i as u32))
            ))
            .collect::<Vec<_>>()
    );
    let renumbered = read(&src.replace("#24", "#924"));
    let second = graph(&renumbered, 25);
    assert_eq!(
        first.items.iter().map(|v| &v.kind).collect::<Vec<_>>(),
        second.items.iter().map(|v| &v.kind).collect::<Vec<_>>()
    );
    assert_ne!(first.items[1].source, second.items[1].source);
    let changed = graph(
        &read(&src.replace("IFCCIRCLE(#23,5.)", "IFCCIRCLE(#23,6.)")),
        25,
    );
    assert_ne!(first.items[1].kind, changed.items[1].kind);
    assert_eq!(Hash::VERSION, 1);
}

#[test]
fn composite_segment_sources_are_separate_from_semantics() {
    for schema in VERSIONS {
        let src = source(schema, "");
        let original = graph(&read(&src), 42);
        let renumbered = graph(&read(&src.replace("#40", "#940")), 42);
        assert_eq!(
            original
                .items
                .iter()
                .map(|item| &item.kind)
                .collect::<Vec<_>>(),
            renumbered
                .items
                .iter()
                .map(|item| &item.kind)
                .collect::<Vec<_>>()
        );
        assert_eq!(original.items[0].source.records, [40, 41]);
        assert_eq!(renumbered.items[0].source.records, [940, 41]);
        assert_ne!(original.items[0].source, renumbered.items[0].source);
        let changed = graph(
            &read(&src.replace(".CONTINUOUS.,.T.,#30", ".CONTINUOUS.,.F.,#30")),
            42,
        );
        assert_ne!(original.items[0].kind, changed.items[0].kind);
    }
}

#[test]
fn rejects_inconsistent_dimensions_through_nested_children_and_trim_points() {
    let extra = "#80=IFCPOLYLINE((#60,#81));\n\
        #81=IFCCARTESIANPOINT((1.,0.,4.));\n\
        #82=IFCCOMPOSITECURVESEGMENT(.DISCONTINUOUS.,.T.,#80);\n\
        #83=IFCCOMPOSITECURVE((#40,#82),.F.);\n\
        #84=IFCTRIMMEDCURVE(#22,(#60),(#81),.T.,.CARTESIAN.);\n\
        #85=IFCTRIMMEDCURVE(#64,(#10),(#11),.T.,.CARTESIAN.);\n\
        #86=IFCTRIMMEDCURVE(#22,(IFCPARAMETERVALUE(0.)),(#81),.T.,.PARAMETER.);\n\
        #90=IFCCOMPOSITECURVE((#91,#92),.F.);\n\
        #91=IFCCOMPOSITECURVESEGMENT(.CONTINUOUS.,.T.,#42);\n\
        #92=IFCCOMPOSITECURVESEGMENT(.DISCONTINUOUS.,.T.,#96);\n\
        #94=IFCCOMPOSITECURVESEGMENT(.CONTINUOUS.,.T.,#80);\n\
        #95=IFCCOMPOSITECURVE((#94,#92),.F.);\n\
        #96=IFCTRIMMEDCURVE(#64,(#97),(#98),.T.,.CARTESIAN.);\n\
        #97=IFCCARTESIANPOINT((2.,0.,4.));\n\
        #98=IFCCARTESIANPOINT((0.,2.,4.));\n\
        #99=IFCTRIMMEDCURVE(#70,(#10),(#60),.T.,.CARTESIAN.);\n\
        #100=IFCCOMPOSITECURVE((#40,#101),.F.);\n\
        #101=IFCCOMPOSITECURVESEGMENT(.DISCONTINUOUS.,.T.,#70);";
    for schema in VERSIONS {
        let file = read(&source(schema, extra));
        for id in [83, 84, 85, 86, 90, 99] {
            let (result, diag, budget) = with(Limits::default(), |cx| file.curve(id, cx));
            assert_eq!(result, Err(Error::Invalid), "{schema} #{id}");
            assert!(diag.iter().any(|d| d.code() == codes::IFC_CURVE_INVALID));
            assert_eq!(budget.used(Kind::Depth), 0);
        }
        for id in [42, 95, 96] {
            graph(&file, id);
        }
        let (result, diag, _) = with(Limits::default(), |cx| file.curve(100, cx));
        assert!(result.is_ok());
        assert!(
            diag.iter()
                .any(|d| d.code() == codes::IFC_CURVE_UNSUPPORTED)
        );
        assert!(!diag.iter().any(|d| d.code() == codes::IFC_CURVE_INVALID));
    }
}

#[test]
fn validates_3d_axes_without_changing_or_rounding_the_ratios() {
    let extra = "#80=IFCDIRECTION((0.,0.,1.));\n\
        #81=IFCDIRECTION((0.,0.,-1.));\n\
        #82=IFCAXIS2PLACEMENT3D(#60,#61,#80);\n\
        #83=IFCCIRCLE(#82,2.);\n\
        #84=IFCAXIS2PLACEMENT3D(#60,#61,#81);\n\
        #85=IFCCIRCLE(#84,2.);\n\
        #86=IFCAXIS2PLACEMENT3D(#60,#61,$);\n\
        #87=IFCCIRCLE(#86,2.);\n\
        #88=IFCAXIS2PLACEMENT3D(#60,$,#62);\n\
        #89=IFCCIRCLE(#88,2.);\n\
        #90=IFCAXIS2PLACEMENT3D(#60,$,$);\n\
        #91=IFCCIRCLE(#90,2.);\n\
        #92=IFCDIRECTION((1.,0.,1.));\n\
        #93=IFCAXIS2PLACEMENT3D(#60,#61,#92);\n\
        #94=IFCCIRCLE(#93,2.);\n\
        #95=IFCDIRECTION((1.E300,1.E300,1.E-300));\n\
        #96=IFCDIRECTION((2.E300,2.E300,2.E-300));\n\
        #97=IFCDIRECTION((2.E300,2.E300,3.E-300));\n\
        #98=IFCDIRECTION((5.E-324,0.,0.));\n\
        #99=IFCDIRECTION((0.,5.E-324,0.));\n\
        #100=IFCDIRECTION((1.E-323,0.,0.));\n\
        #101=IFCAXIS2PLACEMENT3D(#60,#95,#96);\n\
        #102=IFCCIRCLE(#101,2.);\n\
        #103=IFCAXIS2PLACEMENT3D(#60,#95,#97);\n\
        #104=IFCCIRCLE(#103,2.);\n\
        #105=IFCAXIS2PLACEMENT3D(#60,#98,#99);\n\
        #106=IFCCIRCLE(#105,2.);\n\
        #107=IFCAXIS2PLACEMENT3D(#60,#98,#100);\n\
        #108=IFCCIRCLE(#107,2.);\n\
        #109=IFCDIRECTION((1.,1.,0.));\n\
        #110=IFCDIRECTION((1.,1.0000000000000002,0.));\n\
        #111=IFCAXIS2PLACEMENT3D(#60,#109,#110);\n\
        #112=IFCCIRCLE(#111,2.);";
    for schema in VERSIONS {
        let file = read(&source(schema, extra));
        for id in [83, 85, 87, 89, 102, 108] {
            let (result, diag, budget) = with(Limits::default(), |cx| file.curve(id, cx));
            assert_eq!(result, Err(Error::Invalid), "{schema} #{id}");
            assert!(diag.iter().any(|d| d.code() == codes::IFC_CURVE_INVALID));
            assert_eq!(budget.used(Kind::Depth), 0);
        }
        for id in [64, 91, 94, 104, 106, 112] {
            graph(&file, id);
        }
        let curve = graph(&file, 94);
        let CurveKind::Circle { position, .. } = &curve.items[0].kind else {
            panic!()
        };
        assert_eq!(position.axis, Some([0., 0., 2.]));
        assert_eq!(
            position.reference_direction,
            Some(Coordinates::Three([1., 0., 1.]))
        );
    }
}

#[test]
fn follows_public_column_mapping_to_its_profile_curve() {
    let file = read(include_str!("../../../samples/ifc/gni/model_9.ifc"));
    let column = file.instance(131).unwrap();
    assert_eq!(file.kind(column), b"IFCCOLUMN");
    let building: Building = with(Limits::default(), |cx| file.lift(cx)).0.unwrap();
    let element = building
        .find(zumen_building::Id::Global(
            zumen_building::GlobalId::parse("2q2CBrfQTCKPY0zxCdZygb").unwrap(),
        ))
        .unwrap();
    assert_eq!(
        building.element(element).unwrap().source().instance,
        Some(131)
    );
    assert!(file.references(column).contains(&127));
    for (from, to) in [
        (127, 126),
        (126, 125),
        (125, 121),
        (121, 119),
        (119, 114),
        (114, 112),
        (112, 111),
    ] {
        assert!(
            file.references(file.instance(from).unwrap()).contains(&to),
            "#{from} -> #{to}"
        );
    }
    let curve = graph(&file, 111);
    assert_eq!(curve.items[0].source.instance, Some(111));
    assert_eq!(curve.items[0].source.records, [110]);
    let CurveKind::Indexed {
        points,
        segments,
        self_intersect,
    } = &curve.items[0].kind
    else {
        panic!()
    };
    assert_eq!(
        points,
        &[
            Coordinates::Two([-0.15, -0.15]),
            Coordinates::Two([0.15, -0.15]),
            Coordinates::Two([0.15, 0.15]),
            Coordinates::Two([-0.15, 0.15]),
            Coordinates::Two([-0.15, -0.15])
        ]
    );
    assert_eq!(segments, &None);
    assert_eq!(self_intersect, &Some(false));
}

fn coordinates(out: &mut Writer, value: Coordinates) {
    out.begin_array();
    match value {
        Coordinates::Two(v) => {
            for x in v {
                out.float(x);
            }
        }
        Coordinates::Three(v) => {
            for x in v {
                out.float(x);
            }
        }
    }
    out.end_array();
}

fn flag(out: &mut Writer, value: Option<bool>) {
    if let Some(v) = value {
        out.bool(v);
    } else {
        out.null();
    }
}

fn trimmed(out: &mut Writer, values: &[CurveTrim]) {
    out.begin_array();
    for value in values {
        out.begin_object();
        match value {
            CurveTrim::Point(v) => {
                out.key("point");
                coordinates(out, *v);
            }
            CurveTrim::Parameter(v) => {
                out.key("parameter");
                out.float(*v);
            }
        }
        out.end_object();
    }
    out.end_array();
}

fn write_graph(out: &mut Writer, graph: &CurveGraph) {
    for item in &graph.items {
        out.begin_object();
        out.key("instance");
        out.uint(item.source.instance.unwrap());
        out.key("kind");
        match &item.kind {
            CurveKind::Polyline(points) => {
                out.string("polyline");
                out.key("points");
                out.begin_array();
                for p in points {
                    coordinates(out, *p);
                }
                out.end_array();
            }
            CurveKind::Line {
                point,
                direction,
                magnitude,
            } => {
                out.string("line");
                out.key("point");
                coordinates(out, *point);
                out.key("direction");
                coordinates(out, *direction);
                out.key("magnitude");
                out.float(*magnitude);
            }
            CurveKind::Circle { position, radius } => {
                out.string("circle");
                out.key("location");
                coordinates(out, position.location);
                out.key("axis");
                if let Some(v) = position.axis {
                    coordinates(out, Coordinates::Three(v));
                } else {
                    out.null();
                }
                out.key("reference_direction");
                if let Some(v) = position.reference_direction {
                    coordinates(out, v);
                } else {
                    out.null();
                }
                out.key("radius");
                out.float(*radius);
            }
            CurveKind::Indexed {
                points,
                segments,
                self_intersect,
            } => {
                out.string("indexed");
                out.key("points");
                out.begin_array();
                for p in points {
                    coordinates(out, *p);
                }
                out.end_array();
                out.key("segments");
                if let Some(segments) = segments {
                    out.begin_array();
                    for segment in segments {
                        out.begin_object();
                        out.key("kind");
                        out.string(if matches!(segment, IndexedSegment::Line(_)) {
                            "line"
                        } else {
                            "arc"
                        });
                        out.key("indices");
                        out.begin_array();
                        let values: &[u32] = match segment {
                            IndexedSegment::Line(v) => v,
                            IndexedSegment::Arc(v) => v,
                        };
                        for n in values {
                            out.uint(u64::from(*n));
                        }
                        out.end_array();
                        out.end_object();
                    }
                    out.end_array();
                } else {
                    out.null();
                }
                out.key("self_intersect");
                flag(out, *self_intersect);
            }
            CurveKind::Composite {
                segments,
                self_intersect,
            } => {
                out.string("composite");
                out.key("segments");
                out.begin_array();
                for (at, segment) in segments.iter().enumerate() {
                    out.begin_object();
                    out.key("instance");
                    out.uint(item.source.records[at]);
                    out.key("curve");
                    out.uint(graph.item(segment.curve).unwrap().source.instance.unwrap());
                    out.key("same_sense");
                    out.bool(segment.same_sense);
                    out.key("transition");
                    out.string(match segment.transition {
                        CurveTransition::Discontinuous => "DISCONTINUOUS",
                        CurveTransition::Continuous => "CONTINUOUS",
                        CurveTransition::SameGradient => "CONTSAMEGRADIENT",
                        CurveTransition::SameGradientSameCurvature => {
                            "CONTSAMEGRADIENTSAMECURVATURE"
                        }
                    });
                    out.end_object();
                }
                out.end_array();
                out.key("self_intersect");
                flag(out, *self_intersect);
            }
            CurveKind::Trimmed {
                basis,
                trim1,
                trim2,
                sense_agreement,
                preference,
            } => {
                out.string("trimmed");
                out.key("basis");
                out.uint(graph.item(*basis).unwrap().source.instance.unwrap());
                out.key("trim1");
                trimmed(out, trim1);
                out.key("trim2");
                trimmed(out, trim2);
                out.key("sense_agreement");
                out.bool(*sense_agreement);
                out.key("preference");
                out.string(match preference {
                    TrimmingPreference::Cartesian => "CARTESIAN",
                    TrimmingPreference::Parameter => "PARAMETER",
                    TrimmingPreference::Unspecified => "UNSPECIFIED",
                });
            }
            CurveKind::Unsupported(kind) => {
                out.string("unsupported");
                out.key("name");
                out.string(kind);
            }
        }
        out.end_object();
    }
}

fn write_budgets(out: &mut Writer, file: &IfcFile, ids: &[u64]) {
    out.key("budgets");
    out.begin_array();
    for &id in ids {
        let (value, diag, budget) = with(Limits::default(), |cx| file.curve(id, cx));
        assert!(value.is_ok());
        assert!(diag.is_empty());
        out.begin_array();
        out.uint(id);
        for kind in Kind::ALL {
            out.uint(budget.used(kind));
        }
        out.end_array();
    }
    out.end_array();
}

#[test]
#[allow(clippy::disallowed_methods)] // 外部の実装と照らすテストの出力先だけ、環境変数で受け取る
fn writes_curves_for_other_readers() {
    let Some(folder) = std::env::var_os("ZUMEN_IFC_CURVES_OUT") else {
        return;
    };
    let folder = std::path::PathBuf::from(folder);
    std::fs::create_dir_all(&folder).unwrap();
    for schema in VERSIONS {
        let src = source(schema, "");
        let file = read(&src);
        let name = format!("{schema}.ifc");
        std::fs::write(folder.join(&name), src).unwrap();
        let mut out = Writer::new();
        out.begin_object();
        out.key("file");
        out.string(&name);
        write_budgets(&mut out, &file, &[22, 25, 26, 30, 42, 64]);
        out.key("items");
        out.begin_array();
        for id in [22, 25, 26, 30, 42, 64] {
            write_graph(&mut out, &graph(&file, id));
        }
        if schema != "IFC2X3" {
            write_graph(&mut out, &graph(&file, 51));
        }
        out.end_array();
        out.end_object();
        std::fs::write(folder.join(format!("{schema}.json")), out.into_bytes()).unwrap();
    }
    let src = include_str!("../../../samples/ifc/gni/model_9.ifc");
    std::fs::write(folder.join("public.ifc"), src).unwrap();
    let mut out = Writer::new();
    out.begin_object();
    out.key("file");
    out.string("public.ifc");
    write_budgets(&mut out, &read(src), &[111]);
    out.key("items");
    out.begin_array();
    write_graph(&mut out, &graph(&read(src), 111));
    out.end_array();
    out.end_object();
    std::fs::write(folder.join("public.json"), out.into_bytes()).unwrap();
}
