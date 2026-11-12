//! 通り芯から軸と曲線へ進む。入力は自作と既存の公開サンプルだけ。
#![cfg(feature = "building")]
#![cfg(test)]
use zumen_base::{Budget, CancelToken, Cx, Diagnostics, Kind, Limit, Limits, codes};
use zumen_building::{Building, Coordinates, CurveKind, Grid, GridLayout, GridSet, Hash};
use zumen_ifc::{GridAxes, Ifc, IfcFile};
use zumen_json::Writer;
use zumen_sdk::{Codec, Error, Lift};
#[path = "support/curve_json.rs"]
mod curve_json;

const VERSIONS: [&str; 3] = ["IFC2X3", "IFC4", "IFC4X3_ADD2"];
fn source(schema: &str) -> String {
    format!(
        "ISO-10303-21;\nHEADER;\nFILE_DESCRIPTION(('grids'),'2;1');\nFILE_NAME('','',(''),(''),'','','');\nFILE_SCHEMA(('{schema}'));\nENDSEC;\nDATA;\n\
    #1=IFCPROJECT('0000000000000000000001',$,'Project',$,$,$,$,(#8),#9);\n\
    #8=IFCGEOMETRICREPRESENTATIONCONTEXT($,'Model',3,1.E-5,#20,$);\n\
    #9=IFCUNITASSIGNMENT((#10,#11));\n\
    #10=IFCSIUNIT(*,.LENGTHUNIT.,$,.METRE.);\n\
    #11=IFCSIUNIT(*,.PLANEANGLEUNIT.,$,.RADIAN.);\n\
    #20=IFCAXIS2PLACEMENT3D(#21,$,$);\n#21=IFCCARTESIANPOINT((0.,0.,0.));\n\
    #40=IFCLOCALPLACEMENT($,#20);\n\
    #100=IFCGRID('0000000000000000000002',$,'Grid',$,$,#40,#101,(#300,#301,#302,#305{axis}),(#303),(#304){layout});\n\
    #101=IFCPRODUCTDEFINITIONSHAPE($,$,(#102));\n\
    #102=IFCSHAPEREPRESENTATION(#8,'FootPrint','GeometricCurveSet',(#103));\n\
    #103=IFCGEOMETRICCURVESET((#200,#201,#202,#203,#204));\n\
    #200=IFCPOLYLINE((#210,#211));\n\
    #201=IFCLINE(#210,#213);\n\
    #202=IFCCIRCLE(#215,2.);\n\
    #203=IFCCOMPOSITECURVE((#216,#217),.F.);\n\
    #204=IFCTRIMMEDCURVE(#202,(IFCPARAMETERVALUE(1.25)),(IFCPARAMETERVALUE(3.5)),.F.,.PARAMETER.);\n\
    #210=IFCCARTESIANPOINT((1.,2.));\n#211=IFCCARTESIANPOINT((4.,5.));\n\
    #213=IFCVECTOR(#214,3.);\n#214=IFCDIRECTION((2.,0.));\n\
    #215=IFCAXIS2PLACEMENT2D(#210,#214);\n\
    #216=IFCCOMPOSITECURVESEGMENT(.CONTINUOUS.,.T.,#200);\n\
    #217=IFCCOMPOSITECURVESEGMENT(.DISCONTINUOUS.,.F.,#200);\n\
    #300=IFCGRIDAXIS('U1',#200,.T.);\n#301=IFCGRIDAXIS($,#201,.F.);\n\
    #302=IFCGRIDAXIS('Ushared',#203,.T.);\n#303=IFCGRIDAXIS('V1',#202,.T.);\n\
    #304=IFCGRIDAXIS('W1',#204,.F.);\n#305=IFCGRIDAXIS('Uagain',#200,.F.);\n\
    {extra}ENDSEC;\nEND-ISO-10303-21;\n",
        axis = if schema == "IFC2X3" { "" } else { ",#306" },
        layout = if schema == "IFC2X3" {
            ""
        } else {
            ",.IRREGULAR."
        },
        extra = if schema == "IFC2X3" {
            ""
        } else {
            "#205=IFCINDEXEDPOLYCURVE(#218,(IFCLINEINDEX((1,2))),.F.);\n#218=IFCCARTESIANPOINTLIST2D(((1.,2.),(4.,5.)));\n#306=IFCGRIDAXIS('Indexed',#205,.T.);\n"
        }
    )
}
fn with_foreign_grid(schema: &str, first: bool, local: bool, duplicate: bool) -> String {
    let axes = if duplicate {
        "(#300),(#303),$"
    } else {
        "(#800),(#801),$"
    };
    let layout = if schema == "IFC2X3" {
        ""
    } else {
        ",.NOTDEFINED."
    };
    let args = if local {
        format!("{axes}{layout}")
    } else {
        format!("'0000000000000000000003',$,$,$,$,#40,$,{axes}{layout}")
    };
    let grid = format!("IFCGRID({args})");
    let inherited = format!(
        "IFCROOT('0000000000000000000003',$,$,$) IFCOBJECTDEFINITION() IFCOBJECT($) IFCPRODUCT(#40,$){}",
        if schema == "IFC4X3_ADD2" {
            " IFCPOSITIONINGELEMENT()"
        } else {
            ""
        }
    );
    let parts = if first {
        format!("{grid} {inherited}")
    } else {
        format!("{inherited} {grid}")
    };
    let extra = format!(
        "#900=({parts});\n#800=IFCGRIDAXIS('OtherU',#200,.T.);\n#801=IFCGRIDAXIS('OtherV',#201,.F.);\n"
    );
    source(schema).replace("ENDSEC;\nEND-ISO", &format!("{extra}ENDSEC;\nEND-ISO"))
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
fn axes(file: &IfcFile) -> GridAxes<'_> {
    let (r, d, _) = with(Limits::default(), |cx| file.grid_axes(100, cx));
    assert!(d.is_empty(), "{d:?}");
    r.unwrap()
}
#[test]
fn follows_the_grid_to_all_sets_and_keeps_raw_sense_units_and_shared_curves() {
    for schema in VERSIONS {
        let file = read(&source(schema));
        let result = axes(&file);
        assert_eq!(result.axes.len(), if schema == "IFC2X3" { 6 } else { 7 });
        assert_eq!(
            result.layout,
            if schema == "IFC2X3" {
                None
            } else {
                Some(GridLayout::Irregular)
            }
        );
        assert_eq!(result.source.grid.id(), Some(100));
        assert_eq!(result.source.placement.unwrap().id(), Some(40));
        assert_eq!(result.source.representation.unwrap().id(), Some(101));
        let first = &result.axes[0];
        assert_eq!(first.definition.set, GridSet::U);
        assert_eq!(first.definition.tag.as_deref(), Some("U1"));
        assert!(first.definition.same_sense);
        assert_eq!(first.source.id(), Some(300));
        assert_eq!(first.curve.items[0].source.instance, Some(200));
        assert_eq!(
            first.curve.items[0].kind,
            CurveKind::Polyline(vec![Coordinates::Two([1., 2.]), Coordinates::Two([4., 5.])])
        );
        assert_eq!(first.curve.length_unit.as_ref().unwrap().name, "METRE");
        assert_eq!(first.curve.angle_unit.as_ref().unwrap().name, "RADIAN");
        assert_eq!(result.axes[1].definition.tag, None);
        assert!(!result.axes[1].definition.same_sense);
        assert!(!result.axes[3].definition.same_sense);
        assert_eq!(
            first.curve.items[0].kind,
            result.axes[3].curve.items[0].kind
        );
        assert_ne!(
            first.curve.items.as_ptr(),
            result.axes[3].curve.items.as_ptr()
        );
        let shared = &result.axes[2].curve;
        assert_eq!(shared.items.len(), 2);
        let CurveKind::Composite { segments, .. } = &shared.items[0].kind else {
            panic!()
        };
        assert_eq!(segments[0].curve, segments[1].curve);
        assert!(segments[0].same_sense);
        assert!(!segments[1].same_sense);
        let last = result.axes.last().unwrap();
        assert_eq!(last.definition.set, GridSet::W);
        assert!(!last.definition.same_sense);
        assert_eq!(
            result.axes[result.axes.len() - 2].definition.set,
            GridSet::V
        );
        assert_eq!(
            file.kind(result.source.placement.unwrap()),
            b"IFCLOCALPLACEMENT"
        );
    }
}
#[test]
fn keeps_layout_variants_optional_w_unicode_and_versioned_placement_rules() {
    for (name, expected) in [
        ("RECTANGULAR", GridLayout::Rectangular),
        ("RADIAL", GridLayout::Radial),
        ("TRIANGULAR", GridLayout::Triangular),
        ("IRREGULAR", GridLayout::Irregular),
        ("USERDEFINED", GridLayout::UserDefined),
        ("NOTDEFINED", GridLayout::NotDefined),
    ] {
        let src = source("IFC4").replace(".IRREGULAR.", &format!(".{name}."));
        assert_eq!(axes(&read(&src)).layout, Some(expected));
    }
    for schema in VERSIONS {
        let src = source(schema)
            .replace("(#304)", "$")
            .replace("'U1'", "'\\X2\\901A308A\\X0\\'");
        let file = read(&src);
        let result = axes(&file);
        assert_eq!(result.axes[0].definition.tag.as_deref(), Some("通り"));
        assert!(
            result
                .axes
                .iter()
                .all(|axis| axis.definition.set != GridSet::W)
        );
        let absent = src.replace("#40,#101,", "$,$,");
        let file = read(&absent);
        let (r, _, _) = with(Limits::default(), |cx| file.grid_axes(100, cx));
        if schema == "IFC4X3_ADD2" {
            let result = r.unwrap();
            assert!(result.source.placement.is_none());
            assert!(result.source.representation.is_none());
        } else {
            assert!(matches!(r, Err(Error::Invalid)));
        }
    }
    let src = source("IFC4X3_ADD2").replace("#40,#101,", "$,#101,");
    let file = read(&src);
    assert!(matches!(
        with(Limits::default(), |cx| file.grid_axes(100, cx)).0,
        Err(Error::Invalid)
    ));
    for kind in ["IFCSTYLEDREPRESENTATION", "IFCTOPOLOGYREPRESENTATION"] {
        let file = read(&src.replace("IFCSHAPEREPRESENTATION", kind));
        assert!(axes(&file).source.placement.is_none());
    }
    for kind in ["IFCGRIDPLACEMENT", "IFCLINEARPLACEMENT"] {
        let file = read(&source("IFC4X3_ADD2").replace("IFCLOCALPLACEMENT", kind));
        assert_eq!(
            file.kind(axes(&file).source.placement.unwrap()),
            kind.as_bytes()
        );
    }
}
#[test]
fn rejects_invalid_lists_owners_types_dimensions_and_sense_without_partial_results() {
    let base = source("IFC4");
    for (from, to) in [
        ("(#303)", "()"),
        ("(#304)", "()"),
        ("#300,#301", "#300,#300"),
        ("(#303)", "(#300)"),
        ("(#303)", "(#999)"),
        ("#200,.T.", "#200,.U."),
        ("#200,.T.", "#200,$"),
        (".IRREGULAR.", ".UNKNOWN."),
        ("#40,#101,", "#210,#101,"),
        ("#40,#101,", "#40,#210,"),
        ("#200,.T.", "#999,.T."),
        ("(1.,2.)", "(1.,2.,3.)"),
        ("(4.,5.)", "(4.,5.,6.)"),
        ("'U1'", "42"),
    ] {
        let file = read(&base.replace(from, to));
        let (r, d, b) = with(Limits::default(), |cx| file.grid_axes(100, cx));
        assert!(matches!(r, Err(Error::Invalid)), "{from} => {to}: {r:?}");
        assert!(!d.is_empty());
        assert_eq!(b.used(Kind::Depth), 0);
    }
    let extra = "#900=IFCGRID('0000000000000000000003',$,$,$,$,#40,$,(#300),(#303),$,$);\n";
    let file = read(&base.replace("ENDSEC;\nEND-ISO", &format!("{extra}ENDSEC;\nEND-ISO")));
    let (r, d, _) = with(Limits::default(), |cx| file.grid_axes(100, cx));
    assert!(matches!(r, Err(Error::Invalid)));
    assert!(d.iter().any(|d| d.code() == codes::IFC_GRID_INVALID
        && d.places()[0].bytes().unwrap().start
            == file.instance(900).unwrap().span().start as u64));
    let file = read(&base);
    for id in [999, 200] {
        assert!(matches!(
            with(Limits::default(), |cx| file.grid_axes(id, cx)).0,
            Err(Error::Invalid)
        ));
    }
}
#[test]
fn checks_all_parts_and_local_grid_attributes_for_foreign_owners() {
    for schema in VERSIONS {
        let baseline = read(&source(schema));
        let expected = axes(&baseline);
        for first in [false, true] {
            for local in [false, true] {
                let file = read(&with_foreign_grid(schema, first, local, true));
                let (result, diag, budget) = with(Limits::default(), |cx| file.grid_axes(100, cx));
                assert!(
                    matches!(result, Err(Error::Invalid)),
                    "{schema}/first={first}/local={local}"
                );
                let span = file.instance(900).unwrap().span();
                let expected_place = zumen_base::Ref::at_bytes(span.start as u64, span.end as u64);
                assert!(diag.iter().any(|d| d.code() == codes::IFC_GRID_INVALID
                    && d.places() == [expected_place.clone()]));
                assert_eq!(budget.used(Kind::Depth), 0);
                let file = read(&with_foreign_grid(schema, first, local, false));
                let result = axes(&file);
                assert_eq!(result.layout, expected.layout);
                for (actual, expected) in result.axes.iter().zip(&expected.axes) {
                    assert_eq!(actual.definition, expected.definition);
                    assert_eq!(actual.curve, expected.curve);
                }
            }
        }
        // 拡張ヘッダーは Grid インスタンスではなく、所属の相手に数えない。
        let header =
            read(&source(schema).replace("FILE_SCHEMA(", "IFCGRID('vendor');\nFILE_SCHEMA("));
        assert_eq!(axes(&header).axes.len(), expected.axes.len());
    }
}

#[test]
fn unknown_owner_layouts_and_lists_fail_closed_and_part_scan_stops_have_a_span() {
    for schema in VERSIONS {
        for grid in ["IFCGRID($,$,$,$,$)", "IFCGRID(42,$,$)", "IFCGRID((42),$,$)"] {
            let extra = format!("#900=(IFCOBJECTDEFINITION() {grid});\n");
            let file = read(
                &source(schema).replace("ENDSEC;\nEND-ISO", &format!("{extra}ENDSEC;\nEND-ISO")),
            );
            let (result, diag, budget) = with(Limits::default(), |cx| file.grid_axes(100, cx));
            assert!(matches!(result, Err(Error::Invalid)));
            assert!(diag.iter().any(|d| d.code() == codes::IFC_GRID_INVALID));
            assert_eq!(budget.used(Kind::Depth), 0);
        }
        let file =
            read(&with_foreign_grid(schema, false, true, false).replace(",.IRREGULAR.", ",$"));
        let record = file.instance(900).unwrap();
        let index = file
            .records()
            .iter()
            .position(|r| std::ptr::eq(r, record))
            .unwrap();
        let limit = 4 + if schema == "IFC2X3" { 6 } else { 7 } + index as u64 + 1;
        let (result, diag, budget) = with(
            Limits::default().with(Kind::Work, Limit::Finite(limit)),
            |cx| file.grid_axes(100, cx),
        );
        assert!(
            matches!(result, Err(Error::Stopped(zumen_base::Stop::Exceeded(e))) if e.kind == Kind::Work && e.requested == limit + record.parts().len() as u64 - 1)
        );
        let span = record.span();
        assert_eq!(
            diag.iter().next().unwrap().places(),
            &[zumen_base::Ref::at_bytes(
                span.start as u64,
                span.end as u64
            )]
        );
        assert_eq!(budget.used(Kind::Work), limit);
        assert_eq!(budget.used(Kind::Depth), 0);
    }
}

#[test]
fn rejects_unknown_curve_kinds_and_cycles_and_restores_depth() {
    let base = source("IFC4");
    let file = read(&base.replace("IFCCIRCLE(#215,2.)", "IFCOFFSETCURVE2D(#200,2.,.F.)"));
    let (r, d, b) = with(Limits::default(), |cx| file.grid_axes(100, cx));
    assert!(matches!(r, Err(Error::Unsupported)));
    assert!(d.iter().any(|d| d.code() == codes::IFC_GRID_UNSUPPORTED));
    assert_eq!(b.used(Kind::Depth), 0);
    let file = read(&base.replace(".T.,#200)", ".T.,#203)"));
    let (r, _, b) = with(Limits::default(), |cx| file.grid_axes(100, cx));
    assert!(matches!(r, Err(Error::Invalid)));
    assert_eq!(b.used(Kind::Depth), 0);
}
#[test]
fn budgets_every_kind_and_signals_without_changing_existing_ids_or_hashes() {
    for schema in VERSIONS {
        let file = read(&source(schema));
        let before = with(Limits::default(), |cx| Lift::<Building>::lift(&file, cx))
            .0
            .unwrap();
        let (r, _, full) = with(Limits::default(), |cx| file.grid_axes(100, cx));
        assert!(r.is_ok());
        for kind in Kind::ALL {
            let limit = if kind == Kind::Depth {
                (0..32)
                    .find(|&n| {
                        with(Limits::default().with(kind, Limit::Finite(n)), |cx| {
                            file.grid_axes(100, cx)
                        })
                        .0
                        .is_ok()
                    })
                    .unwrap()
            } else {
                full.used(kind)
            };
            assert!(
                with(Limits::default().with(kind, Limit::Finite(limit)), |cx| {
                    file.grid_axes(100, cx)
                })
                .0
                .is_ok()
            );
            if limit > 0 {
                let (r, _, b) = with(
                    Limits::default().with(kind, Limit::Finite(limit - 1)),
                    |cx| file.grid_axes(100, cx),
                );
                assert!(matches!(r, Err(Error::Stopped(_))));
                assert_eq!(b.used(Kind::Depth), 0);
            }
        }
        for expired in [false, true] {
            let (r, _, b) = with(Limits::default(), |cx| {
                if expired {
                    cx.cancel.expire()
                } else {
                    cx.cancel.cancel()
                };
                file.grid_axes(100, cx)
            });
            assert!(matches!(r, Err(Error::Stopped(_))));
            assert_eq!(b.used(Kind::Work), 0);
            assert_eq!(b.used(Kind::DecodedBytes), 0);
        }
        let (_, _, b) = with(Limits::default(), |cx| {
            cx.enter().unwrap();
            file.grid_axes(100, cx).unwrap()
        });
        assert_eq!(b.used(Kind::Depth), 1);
        let after = with(Limits::default(), |cx| Lift::<Building>::lift(&file, cx))
            .0
            .unwrap();
        assert_eq!(
            Hash::document_values(&before),
            Hash::document_values(&after)
        );
        for i in 0..before.elements().len() {
            let e = zumen_building::ElementRef(i as u32);
            assert_eq!(
                before.element(e).unwrap().id(),
                after.element(e).unwrap().id()
            );
            assert_eq!(Hash::element(&before, e), Hash::element(&after, e));
        }
        assert!(
            after
                .components
                .grid
                .iter()
                .all(|(_, value)| !matches!(value, Grid::Axis { curve: Some(_), .. }))
        );
    }
}
#[test]
fn scan_stops_have_the_visited_span_and_keep_work_amounts() {
    for schema in VERSIONS {
        let file = read(&source(schema).replace(",.IRREGULAR.", ",$"));
        // 入口・Grid・配置・表現で 4、軸の列の長さで 6 または 7 Work を使ってから全レコードを走査する。
        let prefix = 4 + if schema == "IFC2X3" { 6 } else { 7 };
        for index in [0, file.records().len() / 2, file.records().len() - 1] {
            let limit = prefix + index as u64;
            let (r, d, b) = with(
                Limits::default().with(Kind::Work, Limit::Finite(limit)),
                |cx| file.grid_axes(100, cx),
            );
            assert!(
                matches!(r,Err(Error::Stopped(zumen_base::Stop::Exceeded(e))) if e.kind==Kind::Work&&e.requested==limit+1)
            );
            let span = file.records()[index].span();
            let expected = zumen_base::Ref::at_bytes(span.start as u64, span.end as u64);
            assert_eq!(
                d.iter().next().unwrap().places(),
                &[expected],
                "{schema}/{index}"
            );
            assert_eq!(b.used(Kind::Work), limit);
            assert_eq!(b.used(Kind::Depth), 0);
        }
    }
}
#[test]
fn gets_public_grid_axes_and_links_them_to_the_existing_product_shape() {
    let file = read(include_str!("../../../samples/ifc/gni/model_9.ifc"));
    let building = with(Limits::default(), |cx| Lift::<Building>::lift(&file, cx))
        .0
        .unwrap();
    let element = building
        .find(zumen_building::Id::Global(
            zumen_building::GlobalId::parse("2vVYk8VWENmwp2otf5sJ1q").unwrap(),
        ))
        .unwrap();
    let id = building
        .element(element)
        .unwrap()
        .source()
        .instance
        .unwrap();
    assert_eq!(id, 5535);
    let shape = with(Limits::default(), |cx| file.product_shape(id, cx))
        .0
        .unwrap();
    let result = with(Limits::default(), |cx| file.grid_axes(id, cx))
        .0
        .unwrap();
    assert_eq!(result.axes.len(), 21);
    let first = &result.axes[0];
    assert_eq!(first.definition.tag.as_deref(), Some("E"));
    assert_eq!(first.definition.set, GridSet::U);
    assert_eq!(first.source.id(), Some(5450));
    assert_eq!(first.curve.items[0].source.instance, Some(5449));
    assert!(
        shape
            .nodes
            .iter()
            .any(|node| node.source.id() == Some(5449))
    );
    let CurveKind::Indexed { points, .. } = &first.curve.items[0].kind else {
        panic!()
    };
    assert_eq!(
        points,
        &[
            Coordinates::Two([21.7569755485943, 10.7531498923766]),
            Coordinates::Two([-28.3832242413298, 10.7531498923767])
        ]
    );
    assert_eq!(result.axes[10].source.id(), Some(5490));
    assert_eq!(result.axes[20].source.id(), Some(5530));
    assert_eq!(result.axes[20].definition.set, GridSet::W);
}
#[test]
fn renumbering_curves_changes_sources_and_keeps_meaning() {
    let src = source("IFC4");
    let file = read(&src);
    let changed = read(&src.replace("#200", "#1200"));
    let a = axes(&file);
    let b = axes(&changed);
    for (a, b) in a.axes.iter().zip(&b.axes) {
        assert_eq!(a.definition, b.definition);
        assert_eq!(a.curve.length_unit, b.curve.length_unit);
        assert_eq!(a.curve.angle_unit, b.curve.angle_unit);
        assert_eq!(
            a.curve.items.iter().map(|v| &v.kind).collect::<Vec<_>>(),
            b.curve.items.iter().map(|v| &v.kind).collect::<Vec<_>>()
        );
    }
    assert_ne!(
        a.axes[0].curve.items[0].source,
        b.axes[0].curve.items[0].source
    );
}
#[test]
fn mutated_inputs_finish_with_shared_budgets_and_repeatable_results() {
    let src = source("IFC4");
    for at in (0..src.len()).step_by(41) {
        let mut input = src.as_bytes().to_vec();
        input[at] = b'$';
        zumen_sdk::testkit::survives_with(&input, |bytes, cx| {
            let file = Ifc::read(bytes, cx)?;
            file.grid_axes(100, cx).map(drop)
        })
        .unwrap();
    }
}

#[test]
fn reserves_long_axis_lists_and_unsupported_diagnostics_before_allocation() {
    let mut names = String::new();
    let mut records = String::new();
    for id in 1000..2000 {
        if !names.is_empty() {
            names.push(',')
        };
        names.push_str(&format!("#{id}"));
        records.push_str(&format!("#{id}=IFCGRIDAXIS($,#200,.T.);\n"));
    }
    let src = source("IFC4")
        .replace("#300,#301,#302,#305,#306", &names)
        .replace("ENDSEC;\nEND-ISO", &format!("{records}ENDSEC;\nEND-ISO"));
    let file = read(&src);
    for limit in [0, 10_000] {
        let (r, _, b) = with(
            Limits::default().with(Kind::DecodedBytes, Limit::Finite(limit)),
            |cx| file.grid_axes(100, cx),
        );
        assert!(matches!(r, Err(Error::Stopped(_))));
        assert_eq!(b.used(Kind::DecodedBytes), if limit == 0 { 0 } else { 256 });
        assert_eq!(b.used(Kind::Depth), 0);
    }
    let file = read(&source("IFC4").replace("IFCCIRCLE(#215,2.)", "IFCOFFSETCURVE2D(#200,2.,.F.)"));
    let (r, _, full) = with(Limits::default(), |cx| file.grid_axes(100, cx));
    assert!(matches!(r, Err(Error::Unsupported)));
    let (r, d, b) = with(
        Limits::default().with(
            Kind::DecodedBytes,
            Limit::Finite(full.used(Kind::DecodedBytes) - 1),
        ),
        |cx| file.grid_axes(100, cx),
    );
    assert!(matches!(r, Err(Error::Stopped(_))));
    assert!(d.iter().all(|d| d.code() != codes::IFC_GRID_UNSUPPORTED));
    assert_eq!(b.used(Kind::Depth), 0);
}

fn optional_text(out: &mut Writer, value: Option<&str>) {
    if let Some(value) = value {
        out.string(value)
    } else {
        out.null()
    }
}
fn record_id(out: &mut Writer, value: Option<&zumen_ifc::Record>) {
    if let Some(value) = value {
        out.uint(value.id().unwrap())
    } else {
        out.null()
    }
}
fn layout_name(value: GridLayout) -> &'static str {
    match value {
        GridLayout::Rectangular => "RECTANGULAR",
        GridLayout::Radial => "RADIAL",
        GridLayout::Triangular => "TRIANGULAR",
        GridLayout::Irregular => "IRREGULAR",
        GridLayout::UserDefined => "USERDEFINED",
        GridLayout::NotDefined => "NOTDEFINED",
    }
}
fn write_grid(out: &mut Writer, file: &IfcFile, id: u64) {
    let (result, diag, budget) = with(Limits::default(), |cx| file.grid_axes(id, cx));
    let result = result.unwrap();
    out.begin_object();
    out.key("instance");
    out.uint(id);
    out.key("placement");
    record_id(out, result.source.placement);
    out.key("representation");
    record_id(out, result.source.representation);
    out.key("layout");
    optional_text(out, result.layout.map(layout_name));
    out.key("axes");
    out.begin_array();
    for axis in &result.axes {
        out.begin_object();
        out.key("instance");
        out.uint(axis.source.id().unwrap());
        out.key("set");
        out.string(match axis.definition.set {
            GridSet::U => "U",
            GridSet::V => "V",
            GridSet::W => "W",
        });
        out.key("tag");
        optional_text(out, axis.definition.tag.as_deref());
        out.key("same_sense");
        out.bool(axis.definition.same_sense);
        out.key("root");
        out.uint(
            axis.curve
                .item(axis.curve.root)
                .unwrap()
                .source
                .instance
                .unwrap(),
        );
        out.key("items");
        out.begin_array();
        curve_json::write_graph(out, &axis.curve);
        out.end_array();
        out.key("sources");
        out.begin_array();
        for item in &axis.curve.items {
            out.begin_object();
            out.key("instance");
            out.uint(item.source.instance.unwrap());
            out.key("records");
            out.begin_array();
            for &id in &item.source.records {
                out.uint(id)
            }
            out.end_array();
            out.end_object();
        }
        out.end_array();
        out.key("units");
        out.string(&format!(
            "{:?}/{:?}",
            axis.curve.length_unit, axis.curve.angle_unit
        ));
        out.end_object();
    }
    out.end_array();
    out.key("budget");
    out.begin_array();
    for kind in Kind::ALL {
        out.uint(budget.used(kind))
    }
    out.end_array();
    out.key("diagnostics");
    out.string(&format!("{diag:?}"));
    out.end_object();
}
fn contract_line(file: &IfcFile, label: &str, limits: Limits, signal: u8) -> String {
    let (result, diag, budget) = with(limits, |cx| {
        match signal {
            1 => cx.cancel.cancel(),
            2 => cx.cancel.expire(),
            _ => {}
        }
        file.grid_axes(100, cx)
    });
    let status = match result {
        Ok(_) => "ok".to_owned(),
        Err(error) => format!("{error:?}"),
    };
    format!(
        "{label}\t{status}\t{diag:?}\t{:?}\n",
        Kind::ALL.map(|kind| budget.used(kind))
    )
}
fn contract_export() -> String {
    let mut output = String::new();
    for schema in VERSIONS {
        let src = source(schema);
        let file = read(&src);
        let (_, _, full) = with(Limits::default(), |cx| file.grid_axes(100, cx));
        output.push_str(&contract_line(
            &file,
            &format!("{schema}/full"),
            Limits::default(),
            0,
        ));
        for kind in Kind::ALL {
            let limit = if kind == Kind::Depth {
                (0..32)
                    .find(|&n| {
                        with(Limits::default().with(kind, Limit::Finite(n)), |cx| {
                            file.grid_axes(100, cx)
                        })
                        .0
                        .is_ok()
                    })
                    .unwrap()
            } else {
                full.used(kind)
            };
            for n in [Some(limit), limit.checked_sub(1)].into_iter().flatten() {
                output.push_str(&contract_line(
                    &file,
                    &format!("{schema}/{kind:?}/{n}"),
                    Limits::default().with(kind, Limit::Finite(n)),
                    0,
                ));
            }
        }
        for first in [false, true] {
            for local in [false, true] {
                for duplicate in [false, true] {
                    let foreign = read(&with_foreign_grid(schema, first, local, duplicate));
                    output.push_str(&contract_line(
                        &foreign,
                        &format!("{schema}/complex-owner/{first}/{local}/{duplicate}"),
                        Limits::default(),
                        0,
                    ));
                }
            }
        }
        for (label, grid) in [
            ("layout", "IFCGRID($,$,$,$,$)"),
            ("list", "IFCGRID(42,$,$)"),
            ("member", "IFCGRID((42),$,$)"),
        ] {
            let extra = format!("#900=(IFCOBJECTDEFINITION() {grid});\n");
            let foreign =
                read(&src.replace("ENDSEC;\nEND-ISO", &format!("{extra}ENDSEC;\nEND-ISO")));
            output.push_str(&contract_line(
                &foreign,
                &format!("{schema}/complex-owner/{label}"),
                Limits::default(),
                0,
            ));
        }
        let foreign =
            read(&with_foreign_grid(schema, false, true, false).replace(",.IRREGULAR.", ",$"));
        let record = foreign.instance(900).unwrap();
        let index = foreign
            .records()
            .iter()
            .position(|r| std::ptr::eq(r, record))
            .unwrap();
        let limit = 4 + if schema == "IFC2X3" { 6 } else { 7 } + index as u64 + 1;
        output.push_str(&contract_line(
            &foreign,
            &format!("{schema}/complex-owner/part-limit"),
            Limits::default().with(Kind::Work, Limit::Finite(limit)),
            0,
        ));
        let header = read(&src.replace("FILE_SCHEMA(", "IFCGRID('vendor');\nFILE_SCHEMA("));
        output.push_str(&contract_line(
            &header,
            &format!("{schema}/complex-owner/header"),
            Limits::default(),
            0,
        ));
        for signal in [1, 2] {
            output.push_str(&contract_line(
                &file,
                &format!("{schema}/signal/{signal}"),
                Limits::default(),
                signal,
            ));
        }
        let scan = read(&src.replace(",.IRREGULAR.", ",$"));
        let prefix = 4 + if schema == "IFC2X3" { 6 } else { 7 };
        for index in [0, scan.records().len() / 2, scan.records().len() - 1] {
            output.push_str(&contract_line(
                &scan,
                &format!("{schema}/scan/{index}"),
                Limits::default().with(Kind::Work, Limit::Finite(prefix + index as u64)),
                0,
            ));
        }
        for (name, from, to) in [
            ("dimension", "(1.,2.)", "(1.,2.,3.)"),
            ("cycle", ".T.,#200)", ".T.,#203)"),
            (
                "unsupported",
                "IFCCIRCLE(#215,2.)",
                "IFCOFFSETCURVE2D(#200,2.,.F.)",
            ),
            ("duplicate", "(#303)", "(#300)"),
            ("sense", "#200,.T.", "#200,.U."),
        ] {
            output.push_str(&contract_line(
                &read(&src.replace(from, to)),
                &format!("{schema}/{name}"),
                Limits::default(),
                0,
            ));
        }
    }
    output
}
// 環境変数は自作・公開の入力と照合記録の出力先だけに使う。
#[allow(clippy::disallowed_methods)]
#[test]
fn writes_grids_for_other_readers() {
    let Some(folder) = std::env::var_os("ZUMEN_IFC_GRIDS_OUT") else {
        return;
    };
    let folder = std::path::PathBuf::from(folder);
    std::fs::create_dir_all(&folder).unwrap();
    for schema in VERSIONS {
        let src = source(schema);
        let file = read(&src);
        let name = format!("{schema}.ifc");
        std::fs::write(folder.join(&name), src).unwrap();
        let mut out = Writer::new();
        out.begin_object();
        out.key("file");
        out.string(&name);
        out.key("grids");
        out.begin_array();
        write_grid(&mut out, &file, 100);
        out.end_array();
        out.end_object();
        std::fs::write(folder.join(format!("{schema}.json")), out.into_bytes()).unwrap();
    }
    let src = include_str!("../../../samples/ifc/gni/model_9.ifc");
    let file = read(src);
    std::fs::write(folder.join("public.ifc"), src).unwrap();
    let mut out = Writer::new();
    out.begin_object();
    out.key("file");
    out.string("public.ifc");
    out.key("grids");
    out.begin_array();
    write_grid(&mut out, &file, 5535);
    out.end_array();
    out.end_object();
    std::fs::write(folder.join("public.json"), out.into_bytes()).unwrap();
    std::fs::write(folder.join("contract.txt"), contract_export()).unwrap();
}
