//! 図面モデルの変更を、P21 の実体に書き戻す（決定 0169）。入力は、どれも試験の中で作る。
#![cfg(feature = "drawing")]

use zumen_base::{Budget, CancelToken, Code, Cx, Diagnostics, Key, codes};
use zumen_drawing::{
    Affine, Applied, Changes, Color, ColorCode, ColorSystem, Drawing2D, Edge, Edit, Element, Hash,
    Inherit, LayerId, Length, LineTypeId, LineWeight, PaperLength, PaperPoint, Placed, Restyle,
    Seen, Shape, Style, id,
};
use zumen_sdk::{Apply, Codec, Error, Lift, testkit};
use zumen_sxf::{Sxf, SxfFile};

/// レイヤ 2 つ・利用者定義色と、線分・円・円弧・折線・線分（ほかの線分と点を共有する）・複合曲線を持つ P21。
/// 見え方の割り当て #70 は、どの要素も共有する
#[cfg(test)]
fn src() -> Vec<u8> {
    let data = [
        "#10=(LENGTH_UNIT()NAMED_UNIT(*)SI_UNIT(.MILLI.,.METRE.));",
        "#20=DRAUGHTING_PRE_DEFINED_COLOUR('red');",
        "#30=DRAUGHTING_PRE_DEFINED_CURVE_FONT('continuous');",
        "#40=LENGTH_MEASURE_WITH_UNIT(POSITIVE_LENGTH_MEASURE(0.25),#10);",
        "#50=COLOUR_RGB('',0.2,0.4,0.6);",
        "#60=CURVE_STYLE(' ',#30,#40,#20);",
        "#70=PRESENTATION_STYLE_ASSIGNMENT((#60));",
        "#100=CARTESIAN_POINT(' ',(0.,0.));",
        "#110=CARTESIAN_POINT(' ',(10.,0.));",
        "#120=DIRECTION(' ',(1.,0.));",
        "#130=VECTOR(' ',#120,10.);",
        "#140=LINE(' ',#100,#130);",
        "#150=TRIMMED_CURVE(' ',#140,(#100),(#110),.T.,.CARTESIAN.);",
        "#160=(\nANNOTATION_CURVE_OCCURRENCE()\nANNOTATION_OCCURRENCE()\nDRAUGHTING_ANNOTATION_OCCURRENCE()\nGEOMETRIC_REPRESENTATION_ITEM()\nREPRESENTATION_ITEM(' ')\nSTYLED_ITEM((#70),#150)\n);",
        "#200=CARTESIAN_POINT(' ',(5.,5.));",
        "#210=AXIS2_PLACEMENT_2D(' ',#200,$);",
        "#220=CIRCLE(' ',#210,3.);",
        "#230=ANNOTATION_CURVE_OCCURRENCE(' ',(#70),#220);",
        "#300=CARTESIAN_POINT(' ',(0.,0.));",
        "#310=AXIS2_PLACEMENT_2D(' ',#300,$);",
        "#320=CIRCLE(' ',#310,5.);",
        "#330=TRIMMED_CURVE(' ',#320,(PARAMETER_VALUE(0.)),(PARAMETER_VALUE(1.5707963267948966)),.T.,.PARAMETER.);",
        "#340=ANNOTATION_CURVE_OCCURRENCE(' ',(#70),#330);",
        "#400=CARTESIAN_POINT(' ',(0.,0.));",
        "#410=CARTESIAN_POINT(' ',(1.,1.));",
        "#420=CARTESIAN_POINT(' ',(2.,0.));",
        "#430=POLYLINE(' ',(#400,#410,#420));",
        "#440=ANNOTATION_CURVE_OCCURRENCE(' ',(#70),#430);",
        "#500=CARTESIAN_POINT(' ',(0.,10.));",
        "#510=DIRECTION(' ',(0.,1.));",
        "#520=VECTOR(' ',#510,10.);",
        "#530=LINE(' ',#100,#520);",
        "#540=TRIMMED_CURVE(' ',#530,(#100),(#500),.T.,.CARTESIAN.);",
        "#560=ANNOTATION_CURVE_OCCURRENCE(' ',(#70),#540);",
        "#600=CARTESIAN_POINT(' ',(20.,0.));",
        "#610=CARTESIAN_POINT(' ',(30.,0.));",
        "#620=POLYLINE(' ',(#600,#610));",
        "#630=COMPOSITE_CURVE_SEGMENT(.CONTINUOUS.,.T.,#620);",
        "#640=COMPOSITE_CURVE(' ',(#630),.F.);",
        "#650=ANNOTATION_CURVE_OCCURRENCE(' ',(#70),#640);",
        "#900=PRESENTATION_LAYER_ASSIGNMENT('A',' ',(#160,#230,#340,#440,#650));",
        "#910=PRESENTATION_LAYER_ASSIGNMENT('B',' ',(#560));",
        "#950=DRAWING_SHEET_REVISION('A3_horizontal',(#160,#230,#340,#440,#560,#650),' ',' ');",
    ];
    let mut src = "ISO-10303-21;\nHEADER;\nFILE_DESCRIPTION(('SCADEC level2 AP202_mode'),'2;1');\nFILE_SCHEMA(('ASSOCIATIVE_DRAUGHTING'));\nENDSEC;\nDATA;\n".to_owned();
    for line in data {
        src.push_str(line);
        src.push('\n');
    }
    src.push_str("ENDSEC;\nEND-ISO-10303-21;\n");
    src.into_bytes()
}

#[cfg(test)]
fn read(src: &[u8]) -> SxfFile {
    testkit::read::<Sxf>(src).0.unwrap()
}

#[cfg(test)]
fn lift(file: &SxfFile) -> Drawing2D {
    let mut budget = Budget::default();
    let mut diag = Diagnostics::new();
    let cancel = CancelToken::new();
    file.lift(&mut Cx::new(&mut budget, &mut diag, &cancel))
        .unwrap()
}

#[cfg(test)]
fn write(file: &SxfFile) -> Vec<u8> {
    let mut budget = Budget::default();
    let mut diag = Diagnostics::new();
    let cancel = CancelToken::new();
    let mut out = Vec::new();
    Sxf::write(
        file,
        &mut out,
        &mut Cx::new(&mut budget, &mut diag, &cancel),
    )
    .unwrap();
    out
}

#[cfg(test)]
fn apply(file: &mut SxfFile, edits: Vec<Edit>) -> (Result<Vec<Applied>, Error>, Vec<Code>) {
    let mut budget = Budget::default();
    let mut diag = Diagnostics::new();
    let cancel = CancelToken::new();
    let changes = Changes {
        edits,
        document: None,
    };
    let result = file.apply(&changes, &mut Cx::new(&mut budget, &mut diag, &cancel));
    (result, diag.iter().map(|d| d.code()).collect())
}

/// 用紙の要素の ID とハッシュ
#[cfg(test)]
fn seen(drawing: &Drawing2D, instance: u64) -> (Seen, Element<PaperPoint>) {
    let element = drawing.sheets[0]
        .elements
        .iter()
        .find(|element| element.source.key() == Some(Key::Instance(instance)))
        .unwrap_or_else(|| panic!("#{instance} がない"))
        .clone();
    let id = format!("{}/i{instance}", id::sheet(&drawing.sheets[0]));
    let hash = Hash::element(&element);
    (Seen { id, hash }, element)
}

#[cfg(test)]
fn has(drawing: &Drawing2D, instance: u64) -> bool {
    drawing.sheets[0]
        .elements
        .iter()
        .any(|element| element.source.key() == Some(Key::Instance(instance)))
}

#[cfg(test)]
fn paper(x: f64, y: f64) -> PaperPoint {
    PaperPoint {
        sheet: zumen_drawing::SheetId(0),
        x,
        y,
    }
}

#[cfg(test)]
fn sxf_color(value: u32) -> Inherit<Color> {
    Inherit::Own(Color {
        code: Some(ColorCode {
            system: ColorSystem::Sxf,
            value,
        }),
        rgb: None,
    })
}

#[test]
fn comparing_unchanged_elements_counts_their_contents() {
    use zumen_base::{Kind, Limit, Limits};
    let points = (0..512)
        .map(|i| if i % 2 == 0 { "#400" } else { "#410" })
        .collect::<Vec<_>>()
        .join(",");
    let input = String::from_utf8(src()).unwrap().replace(
        "#430=POLYLINE(' ',(#400,#410,#420));",
        &format!("#430=POLYLINE(' ',({points}));"),
    );
    let run = |limit: Option<u64>| {
        let mut file = read(input.as_bytes());
        let mut budget = Budget::new(limit.map_or(Limits::default(), |limit| {
            Limits::default().with(Kind::Work, Limit::Finite(limit))
        }));
        let mut diagnostics = Diagnostics::new();
        let result = file.apply(
            &Changes {
                edits: Vec::new(),
                document: None,
            },
            &mut Cx::new(&mut budget, &mut diagnostics, &CancelToken::new()),
        );
        (result, budget.used(Kind::Work), write(&file))
    };
    let (result, used, _) = run(None);
    assert!(result.is_ok(), "{result:?}");
    // 読み直し後の、変更しない折線の複製と比較も数える。
    assert!(used > 8_000, "{used}");
    let (result, _, written) = run(Some(4_000));
    assert!(matches!(result, Err(Error::Stopped(_))));
    assert_eq!(written, input.as_bytes());
}

#[test]
fn edits_are_written_into_the_instances() {
    let src = src();
    let mut file = read(&src);
    let drawing = lift(&file);
    let (line, line_element) = seen(&drawing, 160);
    let (circle, circle_element) = seen(&drawing, 230);
    let (arc, _) = seen(&drawing, 340);
    let (polyline, _) = seen(&drawing, 440);
    let (removed, _) = seen(&drawing, 560);
    let replaced = Element {
        shape: Shape::Curve(Edge::Arc {
            center: paper(1.5, -2.0),
            radius: Length::Paper(PaperLength(4.0)),
            start: 30.0,
            end: 200.0,
            counterclockwise: false,
        }),
        ..seen(&drawing, 340).1
    };
    let added = Element {
        source: zumen_base::Ref::default(),
        layer: LayerId(1),
        style: Style {
            color: sxf_color(1),
            line_type: Inherit::Own(LineTypeId(1)),
            line_type_scale: 1.0,
            line_weight: Some(Inherit::Own(LineWeight::Width(PaperLength(0.25)))),
            transparency: Inherit::Layer,
        },
        shape: Shape::Line {
            start: paper(1e-7, 2.0),
            end: paper(123456.789, -3.5),
        },
    };
    let edits = vec![
        Edit::Restyle {
            target: line,
            style: Restyle {
                layer: Some(LayerId(2)),
                color: Some(sxf_color(17)),
                line_type: Some(Inherit::Own(LineTypeId(2))),
                line_weight: Some(Inherit::Own(LineWeight::Width(PaperLength(0.5)))),
                ..Restyle::default()
            },
        },
        Edit::Transform {
            target: circle,
            transform: Affine {
                m: [1.0, 0.0, 100.0, 0.0, 1.0, 50.0],
            },
        },
        Edit::Replace {
            target: arc,
            element: Box::new(Placed::Paper(replaced.clone())),
        },
        Edit::Transform {
            target: polyline,
            transform: Affine {
                m: [1.0, 0.0, 0.5, 0.0, 1.0, 0.0],
            },
        },
        Edit::Remove { target: removed },
        Edit::Add {
            place: id::sheet(&drawing.sheets[0]),
            element: Box::new(Placed::Paper(added.clone())),
        },
    ];
    let (result, diag) = apply(&mut file, edits);
    let applied = result.unwrap_or_else(|error| panic!("{error:?} {diag:?}"));
    assert_eq!(applied.len(), 6);
    assert_eq!(applied[0].id.as_deref(), Some("sheet/0/i160"));
    assert_eq!(applied[2].id.as_deref(), Some("sheet/0/i340"));
    assert_eq!(applied[4].id, None);
    let new = applied[5].id.clone().unwrap();

    let bytes = write(&file);
    let text = String::from_utf8_lossy(&bytes);
    // 共有する見え方と点は残す。除いた線分だけが使った幾何は除く
    assert!(text.contains("#70=PRESENTATION_STYLE_ASSIGNMENT((#60));"));
    assert!(text.contains("#100=CARTESIAN_POINT"));
    for gone in ["#500=", "#510=", "#520=", "#530=", "#540=", "#560="] {
        assert!(!text.contains(gone), "{gone}");
    }
    // 移した円の元の幾何は除く
    assert!(!text.contains("#220=CIRCLE"));
    assert!(text.contains("#910=PRESENTATION_LAYER_ASSIGNMENT('B',' ',(#160));"));
    assert!(text.contains("#900=PRESENTATION_LAYER_ASSIGNMENT('A',' ',(#230,#340,#440,#650,"));
    assert!(text.contains("'A3_horizontal',(#160,#230,#340,#440,#650,#"));
    assert!(text.contains("DRAUGHTING_PRE_DEFINED_CURVE_FONT('dashed')"));
    // 変えていない実体は、元のバイトのまま
    assert!(text.contains("#600=CARTESIAN_POINT(' ',(20.,0.));\n"));

    let drawing = lift(&read(&bytes));
    assert!(!has(&drawing, 560));
    let (_, line) = seen(&drawing, 160);
    assert_eq!(line.layer, LayerId(2));
    assert_eq!(line.shape, line_element.shape);
    assert_eq!(
        line.style.color,
        Inherit::Own(Color {
            code: Some(ColorCode {
                system: ColorSystem::Sxf,
                value: 17
            }),
            rgb: Some([51, 102, 153])
        })
    );
    assert_eq!(line.style.line_type, Inherit::Own(LineTypeId(2)));
    assert_eq!(
        line.style.line_weight,
        Some(Inherit::Own(LineWeight::Width(PaperLength(0.5))))
    );
    let (_, circle) = seen(&drawing, 230);
    assert_eq!(circle.style, circle_element.style);
    assert_eq!(
        circle.shape,
        Shape::Circle {
            center: paper(105.0, 55.0),
            radius: Length::Paper(PaperLength(3.0)),
        }
    );
    let (_, arc) = seen(&drawing, 340);
    let Shape::Curve(Edge::Arc {
        center,
        start,
        end,
        counterclockwise,
        ..
    }) = arc.shape
    else {
        panic!("{:?}", arc.shape);
    };
    assert_eq!(center, paper(1.5, -2.0));
    assert!(!counterclockwise);
    assert!((start - 30.0).abs() < 1e-9 && (end - 200.0).abs() < 1e-9);
    let instance: u64 = new.rsplit("/i").next().unwrap().parse().unwrap();
    let (seen_new, element) = seen(&drawing, instance);
    assert_eq!(element.shape, added.shape);
    assert_eq!(element.layer, LayerId(1));
    assert_eq!(applied[5].hash, Some(seen_new.hash));
}

#[test]
fn edits_that_cannot_be_written_change_nothing() {
    let src = src();
    let drawing = lift(&read(&src));
    let (line, _) = seen(&drawing, 160);
    let (circle, _) = seen(&drawing, 230);
    let member = drawing.sheets[0]
        .elements
        .iter()
        .find(|element| element.source.key() == Some(Key::Instance(620)));
    let mut cases: Vec<Edit> = vec![
        // 定義のない色・レイヤ・線種
        Edit::Restyle {
            target: circle.clone(),
            style: Restyle {
                color: Some(sxf_color(18)),
                ..Restyle::default()
            },
        },
        Edit::Restyle {
            target: circle.clone(),
            style: Restyle {
                layer: Some(LayerId(3)),
                ..Restyle::default()
            },
        },
        Edit::Restyle {
            target: circle.clone(),
            style: Restyle {
                line_type: Some(Inherit::Own(LineTypeId(17))),
                ..Restyle::default()
            },
        },
        // 形の種類は変えない
        Edit::Replace {
            target: line.clone(),
            element: Box::new(Placed::Paper(Element {
                shape: Shape::Circle {
                    center: paper(0.0, 0.0),
                    radius: Length::Paper(PaperLength(1.0)),
                },
                ..seen(&drawing, 160).1
            })),
        },
        // 表の変更
        Edit::RemoveLayer {
            target: Seen {
                id: "layer/1".to_owned(),
                hash: line.hash,
            },
        },
        // レイヤ B の、ただ 1 つの要素。並びは空にできない
        Edit::Remove {
            target: seen(&drawing, 560).0,
        },
    ];
    // 複合曲線がまとめる線分
    if let Some(member) = member {
        let target = drawing.sheets[0]
            .elements
            .iter()
            .position(|element| element == member)
            .unwrap();
        let place = id::sheet(&drawing.sheets[0]);
        let mut namer = id::Namer::new(&place);
        let mut found = None;
        for (at, element) in drawing.sheets[0].elements.iter().enumerate() {
            let id = namer.next(at, element);
            if at == target {
                found = Some(id);
            }
        }
        cases.push(Edit::Remove {
            target: Seen {
                id: found.unwrap(),
                hash: Hash::element(member),
            },
        });
    }
    for edit in cases {
        let mut file = read(&src);
        // 当てられる変更と一緒に送っても、何も変えない
        let (result, diag) = apply(
            &mut file,
            vec![
                Edit::Remove {
                    target: seen(&drawing, 440).0,
                },
                edit.clone(),
            ],
        );
        assert_eq!(result, Err(Error::Rejected), "{edit:?}");
        assert!(
            diag.contains(&codes::CHANGE_EDIT_UNSUPPORTED),
            "{edit:?} {diag:?}"
        );
        assert_eq!(write(&file), src, "{edit:?}");
    }
}

#[test]
fn a_stale_document_is_refused() {
    let src = src();
    let mut file = read(&src);
    let drawing = lift(&file);
    let mut budget = Budget::default();
    let mut diag = Diagnostics::new();
    let cancel = CancelToken::new();
    let changes = Changes {
        edits: vec![Edit::Remove {
            target: seen(&drawing, 160).0,
        }],
        document: Some(Hash::document(b"other")),
    };
    let result = file.apply(&changes, &mut Cx::new(&mut budget, &mut diag, &cancel));
    assert_eq!(result, Err(Error::Rejected));
    assert!(
        diag.iter()
            .any(|d| d.code() == codes::CHANGE_DOCUMENT_STALE)
    );
    assert_eq!(write(&file), src);
}

/// P21 の点マーカーは、置き換え・変換・追加・色の変更を書き戻さず、ファイルを変えない。P21 の点マーカーは、記号の
/// 実体（SYMBOL_TARGET・DEFINED_SYMBOL）とマーカーの名前で表し、それを組み直す書き方をまだ持たない（決定 0183）
#[test]
fn point_markers_are_not_written_into_p21() {
    use zumen_drawing::{Point, PointLook};
    let data = [
        "#2=DRAUGHTING_PRE_DEFINED_COLOUR('green');",
        "#30=SYMBOL_COLOUR(#2);",
        "#31=SYMBOL_STYLE(' ',#30);",
        "#32=PRESENTATION_STYLE_ASSIGNMENT((#31));",
        "#33=CARTESIAN_POINT(' ',(5.,6.));",
        "#34=DIRECTION(' ',(1.,0.));",
        "#35=AXIS2_PLACEMENT_2D(' ',#33,#34);",
        "#36=SYMBOL_TARGET(' ',#35,1.5,1.5);",
        "#37=PRE_DEFINED_POINT_MARKER_SYMBOL('circle');",
        "#38=DEFINED_SYMBOL(' ',#37,#36);",
        "#39=ANNOTATION_SYMBOL_OCCURRENCE(' ',(#32),#38);",
        "#60=PRESENTATION_LAYER_ASSIGNMENT('L',' ',(#39));",
        "#70=DRAWING_SHEET_REVISION('A3_horizontal',(#39),' ',' ');",
    ];
    let mut src = "ISO-10303-21;\nHEADER;\nFILE_DESCRIPTION(('SCADEC level2 AP202_mode'),'2;1');\nFILE_SCHEMA(('ASSOCIATIVE_DRAUGHTING'));\nENDSEC;\nDATA;\n".to_owned();
    for line in data {
        src.push_str(line);
        src.push('\n');
    }
    src.push_str("ENDSEC;\nEND-ISO-10303-21;\n");
    let src = src.into_bytes();
    let drawing = lift(&read(&src));
    let (target, marker) = seen(&drawing, 39);
    let Shape::Point(point) = &marker.shape else {
        panic!("{:?}", marker.shape);
    };
    assert!(matches!(point.look, PointLook::Marker { .. }));
    let moved = Element {
        shape: Shape::Point(Point {
            position: paper(50.0, 50.0),
            ..point.clone()
        }),
        ..marker.clone()
    };
    for edit in [
        Edit::Replace {
            target: target.clone(),
            element: Box::new(Placed::Paper(moved.clone())),
        },
        Edit::Transform {
            target: target.clone(),
            transform: Affine::translation(10.0, 0.0),
        },
        Edit::Restyle {
            target: target.clone(),
            style: Restyle {
                color: Some(sxf_color(1)),
                ..Restyle::default()
            },
        },
        Edit::Add {
            place: id::sheet(&drawing.sheets[0]),
            element: Box::new(Placed::Paper(moved)),
        },
    ] {
        let mut file = read(&src);
        let (result, diagnostics) = apply(&mut file, vec![edit]);
        assert_eq!(result, Err(Error::Rejected));
        assert_eq!(diagnostics, [codes::CHANGE_EDIT_UNSUPPORTED]);
        assert_eq!(write(&file), src);
    }
}
