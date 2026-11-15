//! 文字の変更と、P21 で共有する幾何・書式の保持を、合成入力で確かめる。
#![cfg(feature = "drawing")]

use zumen_base::{Budget, CancelToken, Code, Cx, Diagnostics, Key, codes};
use zumen_drawing::{
    Affine, Changes, Color, ColorCode, ColorSystem, Drawing2D, Edit, Element, Hash, Height,
    HeightMeaning, Horizontal, Inherit, LayerId, Length, PaperLength, PaperPoint, Piece, Placed,
    Placement, Restyle, Seen, Shape, Text, Vertical, id,
};
use zumen_sdk::{Apply, Codec, Error, Lift, testkit};
use zumen_sxf::{Sxf, SxfFile};

#[cfg(test)]
fn source(p21: bool) -> Vec<u8> {
    let mut out =
        b"ISO-10303-21;\nHEADER;\nFILE_SCHEMA(('ASSOCIATIVE_DRAUGHTING'));\nENDSEC;\nDATA;\n"
            .to_vec();
    if p21 {
        out.extend_from_slice(concat!(
            "#1=DRAUGHTING_PRE_DEFINED_COLOUR('black');\n",
            "#2=EXTERNALLY_DEFINED_TEXT_FONT('MS',$);\n",
            "#3=TEXT_STYLE_FOR_DEFINED_FONT(#1);\n",
            "#4=(TEXT_STYLE('style name',#3)TEXT_STYLE_WITH_BOX_CHARACTERISTICS((BOX_HEIGHT(4.),BOX_WIDTH(8.),BOX_SLANT_ANGLE(0.),BOX_ROTATE_ANGLE(0.)))TEXT_STYLE_WITH_SPACING(LENGTH_MEASURE(0.25)));\n",
            "#5=PRESENTATION_STYLE_ASSIGNMENT((#4));\n",
            "#6=CARTESIAN_POINT('',(1.,3.));\n",
            "#7=AXIS2_PLACEMENT_2D('',#6,$);\n",
            "#8=PLANAR_EXTENT('',8.,4.);\n",
            "#9=TEXT_LITERAL_WITH_EXTENT('$$SXF_baseline left','old',#7,'baseline left',.RIGHT.,#2,#8);\n",
            "#10=ANNOTATION_TEXT_OCCURRENCE('',(#5),#9);\n",
            "#11=ANNOTATION_TEXT_OCCURRENCE('',(#5),#9);\n",
            "#20=PRESENTATION_LAYER_ASSIGNMENT('A','',(#10,#11));\n",
            "#21=PRESENTATION_LAYER_ASSIGNMENT('B','',(#6));\n",
            "#100=DRAWING_SHEET_REVISION('A4_horizontal',(#10,#11),$,'');\n",
        ).as_bytes());
    } else {
        for (id, body) in [
            (1, "layer_feature(\\'A\\','1')"),
            (2, "layer_feature(\\'B\\','1')"),
            (3, "text_font_feature(\\'MS\\')"),
            (
                10,
                "text_string_feature('1','1','1',\\'old\\','1','1','4','8','0.25','0','0','1','1')",
            ),
            (
                11,
                "text_string_feature('1','1','1',\\'old\\','1','1','4','8','0.25','0','0','1','1')",
            ),
            (100, "drawing_sheet_feature(\\'S\\','3','1','0','0')"),
        ] {
            out.extend_from_slice(format!("/*SXF\n#{id}={body}\nSXF*/\n").as_bytes());
        }
    }
    out.extend_from_slice(b"ENDSEC;\nEND-ISO-10303-21;\n");
    out
}

#[cfg(test)]
fn read(src: &[u8]) -> SxfFile {
    testkit::read::<Sxf>(src).0.unwrap()
}

#[cfg(test)]
fn lift(file: &SxfFile) -> Drawing2D {
    let (mut budget, mut diag, cancel) =
        (Budget::default(), Diagnostics::new(), CancelToken::new());
    file.lift(&mut Cx::new(&mut budget, &mut diag, &cancel))
        .unwrap()
}

#[cfg(test)]
fn write(file: &SxfFile) -> Vec<u8> {
    let (mut budget, mut diag, cancel) =
        (Budget::default(), Diagnostics::new(), CancelToken::new());
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
fn apply(
    file: &mut SxfFile,
    edits: Vec<Edit>,
) -> (Result<Vec<zumen_drawing::Applied>, Error>, Vec<Code>) {
    let (mut budget, mut diag, cancel) =
        (Budget::default(), Diagnostics::new(), CancelToken::new());
    let result = file.apply(
        &Changes {
            edits,
            document: None,
        },
        &mut Cx::new(&mut budget, &mut diag, &cancel),
    );
    (result, diag.iter().map(|d| d.code()).collect())
}

#[cfg(test)]
fn seen(drawing: &Drawing2D, instance: u64) -> (Seen, Element<PaperPoint>) {
    let sheet = &drawing.sheets[0];
    let element = sheet
        .elements
        .iter()
        .find(|e| e.source.key() == Some(Key::Instance(instance)))
        .unwrap();
    (
        Seen {
            id: format!("{}/i{instance}", id::sheet(sheet)),
            hash: Hash::element(element),
        },
        element.clone(),
    )
}

#[cfg(test)]
fn color(value: u32) -> Inherit<Color> {
    Inherit::Own(Color {
        code: Some(ColorCode {
            system: ColorSystem::Sxf,
            value,
        }),
        rgb: None,
    })
}

#[test]
fn replacing_and_moving_text_keeps_shared_text_and_other_layers() {
    for p21 in [false, true] {
        let mut file = read(&source(p21));
        let before = lift(&file);
        let (target, mut element) = seen(&before, 10);
        let Shape::Formatted(text) = &mut element.shape else {
            panic!()
        };
        text.content = "変更'\\表記".into();
        text.spans[0].piece = Piece::Text(text.content.clone());
        text.height.value = Length::Paper(PaperLength(8.0));
        text.spans[0].format.oblique = Some(15.0);
        element.layer = LayerId(2);
        element.style.color = color(3);
        let (result, diag) = apply(
            &mut file,
            vec![Edit::Replace {
                target,
                element: Box::new(Placed::Paper(element)),
            }],
        );
        assert!(result.is_ok(), "p21={p21} {result:?} {diag:?}");
        assert!(diag.is_empty());
        let after = lift(&read(&write(&file)));
        assert_eq!(seen(&before, 11).0, seen(&after, 11).0);
        assert_eq!(before.layers, after.layers);
        let (target, changed) = seen(&after, 10);
        let Shape::Formatted(text) = &changed.shape else {
            panic!()
        };
        assert_eq!(text.content, "変更'\\表記");
        assert_eq!(text.height.value, Length::Paper(PaperLength(8.0)));
        assert_eq!(changed.layer, LayerId(2));
        assert_eq!(changed.style.color, color(3));
        let movement = Affine {
            m: [0.0, -1.0, 20.0, 1.0, 0.0, 30.0],
        };
        let (result, diag) = apply(
            &mut file,
            vec![Edit::Transform {
                target,
                transform: movement,
            }],
        );
        assert!(result.is_ok(), "p21={p21} {result:?} {diag:?}");
        let moved = lift(&file);
        assert_eq!(seen(&before, 11).0, seen(&moved, 11).0);
        let Shape::Formatted(text) = seen(&moved, 10).1.shape else {
            panic!()
        };
        assert!((text.point.x - 19.0).abs() < 1e-9);
        assert!((text.point.y - 31.0).abs() < 1e-9);
        assert_eq!(text.rotation, zumen_drawing::Rotation::Angle(90.0));
    }
}

#[test]
fn replacing_text_preserves_unknown_appearance_parts() {
    let source = String::from_utf8(source(true)).unwrap().replace(
        "#3=TEXT_STYLE_FOR_DEFINED_FONT(#1);",
        "#3=(TEXT_STYLE_FOR_DEFINED_FONT(#1)EXTRA('keep'));",
    );
    let mut file = read(source.as_bytes());
    let before = lift(&file);
    let (target, mut element) = seen(&before, 10);
    let Shape::Formatted(text) = &mut element.shape else {
        panic!()
    };
    text.content = "Revised".into();
    text.spans[0].piece = Piece::Text(text.content.clone());
    let (result, diag) = apply(
        &mut file,
        vec![Edit::Replace {
            target,
            element: Box::new(Placed::Paper(element)),
        }],
    );
    assert!(result.is_ok(), "{result:?} {diag:?}");
    let bytes = write(&file);
    assert_eq!(
        String::from_utf8_lossy(&bytes)
            .matches("EXTRA('keep')")
            .count(),
        2
    );
    let after = lift(&read(&bytes));
    assert_eq!(seen(&before, 11).0, seen(&after, 11).0);
}

#[test]
fn adding_plain_text_defines_the_font_and_reports_the_estimated_width() {
    for p21 in [false, true] {
        let mut file = read(&source(p21));
        let before = lift(&file);
        let (_, mut added) = seen(&before, 10);
        added.shape = Shape::Text(Text {
            content: if p21 {
                "日本'\\語😀"
            } else {
                "日本'\\語"
            }
            .into(),
            height: Height {
                value: Length::Paper(PaperLength(3.5)),
                meaning: HeightMeaning::Frame,
            },
            placement: Placement::Anchor {
                point: PaperPoint {
                    sheet: before.sheets[0].id,
                    x: 10.0,
                    y: 20.0,
                },
                horizontal: Horizontal::Center,
                vertical: Vertical::Top,
            },
            rotation: 30.0,
            width_factor: 1.0,
            oblique: 10.0,
            backward: false,
            upside_down: false,
            style: "ＭＳ ゴシック".into(),
        });
        let (result, diag) = apply(
            &mut file,
            vec![Edit::Add {
                place: id::sheet(&before.sheets[0]),
                element: Box::new(Placed::Paper(added)),
            }],
        );
        assert!(result.is_ok(), "p21={p21} {result:?} {diag:?}");
        assert_eq!(diag, [codes::SXF_TEXT_WIDTH_APPROXIMATED]);
        let after = lift(&read(&write(&file)));
        for id in [10, 11] {
            assert_eq!(seen(&before, id).0, seen(&after, id).0);
        }
        assert_eq!(after.sheets[0].elements.len(), 3);
        let added_id = result.unwrap()[0].id.clone().unwrap();
        let new = after.sheets[0]
            .elements
            .iter()
            .find(|e| {
                format!(
                    "{}/i{}",
                    id::sheet(&after.sheets[0]),
                    match e.source.key().unwrap() {
                        Key::Instance(id) => id,
                        _ => panic!(),
                    }
                ) == added_id
            })
            .unwrap();
        let Shape::Formatted(text) = &new.shape else {
            panic!()
        };
        assert_eq!(
            text.spans[0].format.font.as_ref().unwrap().name,
            "ＭＳ ゴシック"
        );
        assert_eq!(text.horizontal, Horizontal::Center);
        assert_eq!(text.vertical, Vertical::Top);
        assert!((text.point.x - 10.0).abs() < 1e-9 && (text.point.y - 20.0).abs() < 1e-9);
    }
}

#[test]
fn restyling_one_shared_text_preserves_the_other_text_and_geometry() {
    for p21 in [false, true] {
        let mut file = read(&source(p21));
        let before = lift(&file);
        let (target, original) = seen(&before, 10);
        let (result, diag) = apply(
            &mut file,
            vec![Edit::Restyle {
                target,
                style: Restyle {
                    color: Some(color(4)),
                    ..Restyle::default()
                },
            }],
        );
        assert!(result.is_ok(), "p21={p21} {result:?} {diag:?}");
        let after = lift(&file);
        assert_eq!(seen(&after, 10).1.shape, original.shape);
        assert_eq!(seen(&after, 10).1.style.color, color(4));
        assert_eq!(seen(&after, 11).0, seen(&before, 11).0);
    }
}

#[test]
fn unrepresentable_sfc_text_rejects_all_edits() {
    for value in ["未対応😀", "改行\n文字", "SXF*/", "¥"] {
        let source = source(false);
        let mut file = read(&source);
        let before = lift(&file);
        let (target, mut element) = seen(&before, 10);
        let Shape::Formatted(text) = &mut element.shape else {
            panic!()
        };
        text.content = value.into();
        text.spans[0].piece = Piece::Text(value.into());
        let (result, diag) = apply(
            &mut file,
            vec![
                Edit::Remove {
                    target: seen(&before, 11).0,
                },
                Edit::Replace {
                    target,
                    element: Box::new(Placed::Paper(element)),
                },
            ],
        );
        assert!(matches!(result, Err(Error::Rejected)), "{value} {result:?}");
        assert_eq!(diag, [codes::CHANGE_EDIT_UNSUPPORTED]);
        assert_eq!(write(&file), source);
    }
}

#[test]
fn layers_are_added_renamed_hidden_and_shown_without_changing_elements() {
    for p21 in [false, true] {
        let mut file = read(&source(p21));
        let before = lift(&file);
        let was = &before.layers[0];
        let mut renamed = was.clone();
        renamed.name = "改名'\\図面".into();
        renamed.state.as_mut().unwrap().visible = false;
        renamed.state.as_mut().unwrap().printable = false;
        let mut added = renamed.clone();
        added.name = "追加レイヤ".into();
        let (result, diag) = apply(
            &mut file,
            vec![
                Edit::ReplaceLayer {
                    target: Seen {
                        id: id::layer(was),
                        hash: Hash::layer(was),
                    },
                    layer: Box::new(renamed.clone()),
                },
                Edit::AddLayer {
                    layer: Box::new(added.clone()),
                },
            ],
        );
        assert!(result.is_ok(), "p21={p21} {result:?} {diag:?}");
        assert!(diag.is_empty());
        let after = lift(&read(&write(&file)));
        assert_eq!(after.layers.len(), 3);
        assert_eq!(after.layers[0].name, renamed.name);
        assert_eq!(Hash::layer(&after.layers[0]), Hash::layer(&renamed));
        assert_eq!(
            Hash::layer(&after.layers[1]),
            Hash::layer(&before.layers[1])
        );
        assert_eq!(after.layers[2].name, added.name);
        assert_eq!(after.layers[2].state, added.state);
        assert_eq!(after.layers[2].id, LayerId(3));
        for n in [10, 11] {
            assert_eq!(seen(&before, n).0, seen(&after, n).0);
        }
        let applied = result.unwrap();
        assert_eq!(applied[1].id, Some(id::layer(&after.layers[2])));
        assert_eq!(applied[1].hash, Some(Hash::layer(&after.layers[2])));
        let edits = [0, 2]
            .into_iter()
            .map(|at| {
                let mut layer = after.layers[at].clone();
                layer.state.as_mut().unwrap().visible = true;
                layer.state.as_mut().unwrap().printable = true;
                Edit::ReplaceLayer {
                    target: Seen {
                        id: id::layer(&after.layers[at]),
                        hash: Hash::layer(&after.layers[at]),
                    },
                    layer: Box::new(layer),
                }
            })
            .collect();
        let (result, diag) = apply(&mut file, edits);
        assert!(result.is_ok(), "p21={p21} {result:?} {diag:?}");
        assert!(lift(&file).layers.iter().all(|l| l.state.unwrap().visible));
    }
}

#[test]
fn moving_the_last_element_keeps_the_empty_layer_definition() {
    for p21 in [false, true] {
        let mut file = read(&source(p21));
        let before = lift(&file);
        let edits = [10, 11]
            .into_iter()
            .map(|n| Edit::Restyle {
                target: seen(&before, n).0,
                style: Restyle {
                    layer: Some(LayerId(2)),
                    ..Restyle::default()
                },
            })
            .collect();
        let (result, diag) = apply(&mut file, edits);
        assert!(result.is_ok(), "p21={p21} {result:?} {diag:?}");
        let after = lift(&read(&write(&file)));
        assert_eq!(after.sheets[0].elements.len(), 2);
        for (was, now) in before.layers.iter().zip(&after.layers) {
            assert_eq!(Hash::layer(was), Hash::layer(now));
        }
        for n in [10, 11] {
            assert_eq!(seen(&after, n).1.layer, LayerId(2));
            assert_eq!(seen(&after, n).1.shape, seen(&before, n).1.shape);
            assert_eq!(seen(&after, n).0.id, seen(&before, n).0.id);
        }
    }
}

#[test]
fn stale_or_unrepresentable_layers_reject_all_edits() {
    for p21 in [false, true] {
        let source = source(p21);
        let before = lift(&read(&source));
        let original = &before.layers[0];
        let target = Seen {
            id: id::layer(original),
            hash: Hash::layer(original),
        };
        let mut changed = original.clone();
        changed.name = "renamed".into();
        for (target, layer, code) in [
            (
                Seen {
                    hash: Hash::layer(&changed),
                    ..target.clone()
                },
                changed.clone(),
                codes::CHANGE_ELEMENT_STALE,
            ),
            (
                target.clone(),
                {
                    let mut l = changed.clone();
                    l.state.as_mut().unwrap().editable = false;
                    l
                },
                codes::CHANGE_EDIT_UNSUPPORTED,
            ),
            (
                target,
                {
                    let mut l = changed;
                    l.name = "$$SXF_dummyHidden".into();
                    l
                },
                codes::CHANGE_EDIT_UNSUPPORTED,
            ),
        ] {
            let mut file = read(&source);
            let (result, diag) = apply(
                &mut file,
                vec![
                    Edit::Restyle {
                        target: seen(&before, 10).0,
                        style: Restyle {
                            color: Some(color(4)),
                            ..Restyle::default()
                        },
                    },
                    Edit::ReplaceLayer {
                        target,
                        layer: Box::new(layer),
                    },
                ],
            );
            assert!(matches!(result, Err(Error::Rejected)));
            assert_eq!(diag, [code]);
            assert_eq!(write(&file), source);
        }
    }
}
