//! 図面モデルの変更を、SFC のフィーチャのコメントに書き戻す（決定 0167）。入力は、どれも試験の中で作る。
#![cfg(feature = "drawing")]

use zumen_base::{Budget, CancelToken, Code, Cx, Diagnostics, codes};
use zumen_drawing::{
    Affine, Applied, Changes, Color, ColorCode, ColorSystem, Drawing2D, Edge, Edit, Element, Hash,
    Inherit, LayerId, Length, LineTypeId, LineWeight, PaperLength, PaperPoint, Placed, Restyle,
    Seen, Shape, Style, id,
};
use zumen_sdk::{Apply, Codec, Error, Lift, testkit};
use zumen_sxf::{Sxf, SxfFile};

/// レイヤ 2 つ・利用者定義色・利用者定義線幅と、複合曲線とその線分・線分・円・円弧・折線・文字を持ち、
/// 用紙のフィーチャで終わる SFC
#[cfg(test)]
fn src() -> Vec<u8> {
    let features: [&[u8]; 12] = [
        b"layer_feature(\\'A\\','1')",
        b"layer_feature(\\'B\\','1')",
        b"user_defined_colour_feature('10','20','30')",
        b"width_feature('0.3')",
        b"line_feature('2','1','1','1','20.0','0.0','30.0','0.0')",
        b"composite_curve_org_feature('1','1','1','1')",
        b"line_feature('1','1','1','1','0.0','0.0','10.0','0.0')",
        b"circle_feature('1','2','1','2','5.0','5.0','3.0')",
        b"arc_feature('1','1','1','1','0.0','0.0','5.0','0','0.0','90.0')",
        b"polyline_feature('1','1','1','1','3','(0,1,2)','(0,1,0)')",
        b"text_string_feature('1','1','1',\\'T\\','1.0','1.0','2.0','2.0','0.0','0.0','1','1')",
        b"drawing_sheet_feature(\\'S\\','3','1','0','0')",
    ];
    let mut src = b"ISO-10303-21;\r\nHEADER;\r\nFILE_DESCRIPTION(('SCADEC level2 feature_mode'),'2;1');\r\nFILE_SCHEMA(('ASSOCIATIVE_DRAUGHTING'));\r\nENDSEC;\r\nDATA;\r\n".to_vec();
    for (at, feature) in features.iter().enumerate() {
        src.extend_from_slice(format!("/*SXF\r\n#{} = ", (at + 1) * 10).as_bytes());
        src.extend_from_slice(feature);
        src.extend_from_slice(b"\r\nSXF*/\r\n");
    }
    src.extend_from_slice(b"ENDSEC;\r\nEND-ISO-10303-21;\r\n");
    src
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
        .find(|element| element.source.key() == Some(zumen_base::Key::Instance(instance)))
        .unwrap_or_else(|| panic!("#{instance} がない"))
        .clone();
    let id = format!("{}/i{instance}", id::sheet(&drawing.sheets[0]));
    let hash = Hash::element(&element);
    (Seen { id, hash }, element)
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
fn edits_are_written_into_the_feature_comments() {
    let src = src();
    let mut file = read(&src);
    let drawing = lift(&file);
    let (line, _) = seen(&drawing, 70);
    let (circle, circle_element) = seen(&drawing, 80);
    let (arc, arc_element) = seen(&drawing, 90);
    let (polyline, _) = seen(&drawing, 100);
    let (text, _) = seen(&drawing, 110);
    let replaced = Element {
        shape: Shape::Line {
            start: paper(1.5, -2.0),
            end: paper(1e-7, 123456.789),
        },
        layer: LayerId(2),
        ..seen(&drawing, 70).1
    };
    let added = Element {
        source: zumen_base::Ref::default(),
        layer: LayerId(1),
        style: Style {
            color: Inherit::Own(Color {
                code: None,
                rgb: Some([10, 20, 30]),
            }),
            line_type: Inherit::Own(LineTypeId(2)),
            line_type_scale: 1.0,
            line_weight: Some(Inherit::Own(LineWeight::Width(PaperLength(0.3)))),
        },
        shape: Shape::Circle {
            center: paper(7.0, 8.0),
            radius: Length::Paper(PaperLength(2.5)),
        },
    };
    let edits = vec![
        Edit::Replace {
            target: line,
            element: Box::new(Placed::Paper(replaced.clone())),
        },
        Edit::Restyle {
            target: circle,
            style: Restyle {
                layer: Some(LayerId(2)),
                color: Some(sxf_color(17)),
                line_weight: Some(Inherit::Own(LineWeight::Width(PaperLength(0.3)))),
                ..Restyle::default()
            },
        },
        Edit::Transform {
            target: arc,
            transform: Affine {
                m: [1.0, 0.0, 100.0, 0.0, 1.0, 50.0],
            },
        },
        Edit::Remove { target: polyline },
        Edit::Remove { target: text },
        Edit::Add {
            place: id::sheet(&drawing.sheets[0]),
            element: Box::new(Placed::Paper(added.clone())),
        },
    ];
    let (result, diag) = apply(&mut file, edits);
    let applied = result.unwrap_or_else(|error| panic!("{error:?} {diag:?}"));
    assert_eq!(applied.len(), 6);
    assert_eq!(applied[0].id.as_deref(), Some("sheet/0/i70"));
    assert_eq!(applied[3].id, None);
    assert_eq!(applied[5].id.as_deref(), Some("sheet/0/i121"));

    let bytes = write(&file);
    let text = String::from_utf8_lossy(&bytes);
    assert!(
        text.contains("#70 = line_feature('2','1','1','1','1.5','-2','0.0000001','123456.789')")
    );
    assert!(text.contains("#80 = circle_feature('2','17','1','11','5.0','5.0','3.0')"));
    assert!(text.contains("#90 = arc_feature('1','1','1','1','100','50','5','0','0','90')"));
    assert!(!text.contains("#100 ="));
    assert!(!text.contains("#110 ="));
    assert!(text.contains(
        "/*SXF\r\n#121 = circle_feature('1','17','2','11','7','8','2.5')\r\nSXF*/\r\n/*SXF\r\n#120 = drawing_sheet_feature"
    ));
    // 変えていないフィーチャは、元のバイトのまま
    assert!(text.contains(
        "/*SXF\r\n#50 = line_feature('2','1','1','1','20.0','0.0','30.0','0.0')\r\nSXF*/\r\n"
    ));
    // 除いたフィーチャは、後の改行とともに除く
    assert!(text.contains("'90')\r\nSXF*/\r\n/*SXF\r\n#121 ="));

    let drawing = lift(&read(&bytes));
    assert_eq!(seen(&drawing, 70).1.shape, replaced.shape);
    let (_, circle) = seen(&drawing, 80);
    assert_eq!(circle.layer, LayerId(2));
    assert_eq!(circle.shape, circle_element.shape);
    let (_, arc) = seen(&drawing, 90);
    let Shape::Curve(Edge::Arc { center, .. }) = arc.shape else {
        panic!("{:?}", arc.shape);
    };
    assert_eq!(center, paper(100.0, 50.0));
    assert_eq!(arc.style, arc_element.style);
    let (new, element) = seen(&drawing, 121);
    assert_eq!(element.shape, added.shape);
    assert_eq!(
        element.style.color,
        Inherit::Own(Color {
            code: Some(ColorCode {
                system: ColorSystem::Sxf,
                value: 17
            }),
            rgb: Some([10, 20, 30])
        })
    );
    assert_eq!(applied[5].hash, Some(new.hash));
}

#[test]
fn edits_that_cannot_be_written_change_nothing() {
    let src = src();
    let drawing = lift(&read(&src));
    let (line, _) = seen(&drawing, 70);
    let (member, _) = seen(&drawing, 50);
    let (circle, _) = seen(&drawing, 80);
    let cases: Vec<(Edit, Code)> = vec![
        // 複合曲線がまとめる線分
        (
            Edit::Remove {
                target: member.clone(),
            },
            codes::CHANGE_EDIT_UNSUPPORTED,
        ),
        // 定義のない色・レイヤ・線幅・線種の倍率
        (
            Edit::Restyle {
                target: circle.clone(),
                style: Restyle {
                    color: Some(sxf_color(18)),
                    ..Restyle::default()
                },
            },
            codes::CHANGE_EDIT_UNSUPPORTED,
        ),
        (
            Edit::Restyle {
                target: circle.clone(),
                style: Restyle {
                    layer: Some(LayerId(3)),
                    ..Restyle::default()
                },
            },
            codes::CHANGE_EDIT_UNSUPPORTED,
        ),
        (
            Edit::Restyle {
                target: circle.clone(),
                style: Restyle {
                    line_weight: Some(Inherit::Own(LineWeight::Width(PaperLength(0.4)))),
                    ..Restyle::default()
                },
            },
            codes::CHANGE_EDIT_UNSUPPORTED,
        ),
        (
            Edit::Restyle {
                target: circle.clone(),
                style: Restyle {
                    line_type_scale: Some(2.0),
                    ..Restyle::default()
                },
            },
            codes::CHANGE_EDIT_UNSUPPORTED,
        ),
        (
            Edit::Restyle {
                target: circle.clone(),
                style: Restyle {
                    line_type: Some(Inherit::Own(LineTypeId(17))),
                    ..Restyle::default()
                },
            },
            codes::CHANGE_EDIT_UNSUPPORTED,
        ),
        // 形の種類は変えない
        (
            Edit::Replace {
                target: line.clone(),
                element: Box::new(Placed::Paper(Element {
                    shape: Shape::Circle {
                        center: paper(0.0, 0.0),
                        radius: Length::Paper(PaperLength(1.0)),
                    },
                    ..seen(&drawing, 70).1
                })),
            },
            codes::CHANGE_EDIT_UNSUPPORTED,
        ),
        // 前提
        (
            Edit::Remove {
                target: Seen {
                    id: "sheet/0/i999".to_owned(),
                    hash: line.hash,
                },
            },
            codes::CHANGE_ELEMENT_MISSING,
        ),
        (
            Edit::Remove {
                target: Seen {
                    id: line.id.clone(),
                    hash: circle.hash,
                },
            },
            codes::CHANGE_ELEMENT_STALE,
        ),
        // 表の変更
        (
            Edit::RemoveLayer {
                target: Seen {
                    id: "layer/1".to_owned(),
                    hash: line.hash,
                },
            },
            codes::CHANGE_EDIT_UNSUPPORTED,
        ),
    ];
    for (edit, code) in cases {
        let mut file = read(&src);
        // 当てられる変更と一緒に送っても、何も変えない
        let (result, diag) = apply(
            &mut file,
            vec![
                Edit::Remove {
                    target: seen(&drawing, 100).0,
                },
                edit.clone(),
            ],
        );
        assert_eq!(result, Err(Error::Rejected), "{edit:?}");
        assert!(diag.contains(&code), "{edit:?} {diag:?}");
        assert_eq!(write(&file), src, "{edit:?}");
    }
    // 同じ要素を 2 度指す
    let mut file = read(&src);
    let (result, _) = apply(
        &mut file,
        vec![
            Edit::Remove {
                target: line.clone(),
            },
            Edit::Remove { target: line },
        ],
    );
    assert_eq!(result, Err(Error::Rejected));
}

#[test]
fn a_stale_document_and_p21_are_refused() {
    let src = src();
    let mut file = read(&src);
    let drawing = lift(&file);
    let mut budget = Budget::default();
    let mut diag = Diagnostics::new();
    let cancel = CancelToken::new();
    let changes = Changes {
        edits: vec![Edit::Remove {
            target: seen(&drawing, 70).0,
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

    let p21 = b"ISO-10303-21;\nHEADER;\nFILE_SCHEMA(('ASSOCIATIVE_DRAUGHTING'));\nENDSEC;\nDATA;\n#1=CARTESIAN_POINT('',(0.,0.));\nENDSEC;\nEND-ISO-10303-21;\n";
    let mut file = read(p21);
    let (result, diag) = apply(&mut file, Vec::new());
    assert_eq!(result, Err(Error::Rejected));
    assert_eq!(diag, vec![codes::CHANGE_FORMAT_UNSUPPORTED]);
}
