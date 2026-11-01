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
fn apply_counts_copying_the_input_and_stops_without_changes() {
    use zumen_base::{Kind, Limit, Limits};
    let mut large = src();
    large.extend(vec![b' '; 1024 * 1024]);
    let run = |input: &[u8], limit: Option<u64>| {
        let mut file = read(input);
        let mut budget = Budget::new(limit.map_or(Limits::default(), |limit| {
            Limits::default().with(Kind::Work, Limit::Finite(limit))
        }));
        let mut diagnostics = Diagnostics::new();
        let changes = Changes {
            edits: Vec::new(),
            document: None,
        };
        let result = file.apply(
            &changes,
            &mut Cx::new(&mut budget, &mut diagnostics, &CancelToken::new()),
        );
        (result, budget.used(Kind::Work), write(&file))
    };
    let (_, small, _) = run(&src(), None);
    let (result, large_work, _) = run(&large, None);
    assert!(result.is_ok());
    assert!(large_work >= small + 1024, "{small}, {large_work}");
    let (result, _, written) = run(&large, Some(small + 100));
    assert!(matches!(result, Err(Error::Stopped(_))));
    assert_eq!(written, large);
}

#[test]
fn apply_counts_each_search_for_user_colors_and_widths() {
    use zumen_base::{Kind, Limit, Limits};
    let extra = (0..200)
        .map(|at| format!("/*SXF\r\n#{} = user_defined_colour_feature('1','2','3')\r\nSXF*/\r\n/*SXF\r\n#{} = width_feature('0.4')\r\nSXF*/\r\n", 2000 + at * 2, 2001 + at * 2))
        .collect::<String>();
    let large = String::from_utf8(src())
        .unwrap()
        .replace("/*SXF\r\n#30 =", &format!("{extra}/*SXF\r\n#30 ="));
    let run = |input: &[u8], edit: bool, limit: Option<u64>| {
        let mut file = read(input);
        let drawing = lift(&file);
        let edits = if edit {
            [70, 80, 90, 100]
                .into_iter()
                .map(|instance| Edit::Restyle {
                    target: seen(&drawing, instance).0,
                    style: Restyle {
                        color: Some(Inherit::Own(Color {
                            code: None,
                            rgb: Some([10, 20, 30]),
                        })),
                        line_weight: Some(Inherit::Own(LineWeight::Width(PaperLength(0.3)))),
                        ..Restyle::default()
                    },
                })
                .collect()
        } else {
            Vec::new()
        };
        let mut budget = Budget::new(limit.map_or(Limits::default(), |limit| {
            Limits::default().with(Kind::Work, Limit::Finite(limit))
        }));
        let mut diagnostics = Diagnostics::new();
        let result = file.apply(
            &Changes {
                edits,
                document: None,
            },
            &mut Cx::new(&mut budget, &mut diagnostics, &CancelToken::new()),
        );
        (result, budget.used(Kind::Work), write(&file))
    };
    let (_, small_base, _) = run(&src(), false, None);
    let (small_result, small_edit, _) = run(&src(), true, None);
    assert!(small_result.is_ok(), "{small_result:?}");
    let (_, large_base, _) = run(large.as_bytes(), false, None);
    let (large_result, large_edit, _) = run(large.as_bytes(), true, None);
    assert!(large_result.is_ok(), "{large_result:?}");
    assert!(
        large_edit - large_base >= small_edit - small_base + 1600,
        "{small_base}, {small_edit}, {large_base}, {large_edit}"
    );
    let (result, _, written) = run(large.as_bytes(), true, Some(large_edit - 1));
    assert!(matches!(result, Err(Error::Stopped(_))));
    assert_eq!(written, large.as_bytes());
}

#[test]
fn apply_counts_new_polyline_vertices_before_writing() {
    use zumen_base::{Kind, Limit, Limits};
    let input = src();
    let drawing = lift(&read(&input));
    let base = seen(&drawing, 100).1;
    let mut missing = seen(&drawing, 70).0;
    missing.id = "sheet/0/i9999".to_owned();
    let run = |count: usize, replace: bool, limit: Option<u64>| {
        let mut element = base.clone();
        let Shape::Polyline(line) = &mut element.shape else {
            panic!()
        };
        line.vertices.resize(count, line.vertices[0]);
        let first = if replace {
            Edit::Replace {
                target: seen(&drawing, 100).0,
                element: Box::new(Placed::Paper(element)),
            }
        } else {
            Edit::Add {
                place: id::sheet(&drawing.sheets[0]),
                element: Box::new(Placed::Paper(element)),
            }
        };
        let edits = vec![
            first,
            Edit::Remove {
                target: missing.clone(),
            },
        ];
        let mut file = read(&input);
        let mut budget = Budget::new(limit.map_or(Limits::default(), |limit| {
            Limits::default().with(Kind::Work, Limit::Finite(limit))
        }));
        let mut diag = Diagnostics::new();
        let result = file.apply(
            &Changes {
                edits,
                document: None,
            },
            &mut Cx::new(&mut budget, &mut diag, &CancelToken::new()),
        );
        (result, budget.used(Kind::Work), write(&file))
    };
    for replace in [false, true] {
        let (result, small, _) = run(3, replace, None);
        assert!(matches!(result, Err(Error::Rejected)));
        let (result, large, _) = run(8192, replace, None);
        assert!(matches!(result, Err(Error::Rejected)));
        assert!(large >= small + 8190, "{small}, {large}");
        let (result, _, written) = run(8192, replace, Some(small + 1));
        assert!(matches!(result, Err(Error::Stopped(_))));
        assert_eq!(written, input);
    }
}

#[test]
fn transforming_polyline_counts_its_vertices_and_stops_without_changes() {
    use zumen_base::{Kind, Limit, Limits};
    let xs = vec!["0"; 8192].join(",");
    let ys = vec!["1"; 8192].join(",");
    let input = String::from_utf8(src()).unwrap().replace(
        "'3','(0,1,2)','(0,1,0)'",
        &format!("'8192','({xs})','({ys})'"),
    );
    let drawing = lift(&read(input.as_bytes()));
    let run = |edit: bool, limit: Option<u64>| {
        let mut file = read(input.as_bytes());
        let mut missing = seen(&drawing, 70).0;
        missing.id = "sheet/0/i9999".to_owned();
        let edits = if edit {
            vec![
                Edit::Transform {
                    target: seen(&drawing, 100).0,
                    transform: Affine {
                        m: [1.0, 0.0, 1.0, 0.0, 1.0, 0.0],
                    },
                },
                Edit::Remove { target: missing },
            ]
        } else {
            vec![Edit::Remove { target: missing }]
        };
        let mut budget = Budget::new(limit.map_or(Limits::default(), |limit| {
            Limits::default().with(Kind::Work, Limit::Finite(limit))
        }));
        let mut diag = Diagnostics::new();
        let result = file.apply(
            &Changes {
                edits,
                document: None,
            },
            &mut Cx::new(&mut budget, &mut diag, &CancelToken::new()),
        );
        (result, budget.used(Kind::Work), write(&file))
    };
    let (result, baseline, _) = run(false, None);
    assert!(matches!(result, Err(Error::Rejected)));
    let (result, changed, _) = run(true, None);
    assert!(matches!(result, Err(Error::Rejected)));
    assert!(changed >= baseline + 8192, "{baseline}, {changed}");
    let (result, _, written) = run(true, Some(changed - 1));
    assert!(matches!(result, Err(Error::Stopped(_))));
    assert_eq!(written, input.as_bytes());
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
            transparency: Inherit::Layer,
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
fn a_stale_document_is_refused() {
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
}

#[test]
fn consecutive_composites_keep_their_members_protected() {
    let original = String::from_utf8(src()).unwrap();
    let extra: String = (1000..1032)
        .map(|id| {
            format!("/*SXF\r\n#{id} = composite_curve_org_feature('1','1','1','1')\r\nSXF*/\r\n")
        })
        .collect();
    let input = original.replace("/*SXF\r\n#60 =", &format!("{extra}/*SXF\r\n#60 ="));
    let mut file = read(input.as_bytes());
    let drawing = lift(&file);
    let (result, diagnostics) = apply(
        &mut file,
        vec![Edit::Remove {
            target: seen(&drawing, 50).0,
        }],
    );
    assert_eq!(result, Err(Error::Rejected));
    assert!(diagnostics.contains(&codes::CHANGE_EDIT_UNSUPPORTED));
    assert_eq!(write(&file), input.as_bytes());
    let (result, _) = apply(
        &mut file,
        vec![Edit::Remove {
            target: seen(&drawing, 70).0,
        }],
    );
    assert!(result.is_ok());
}

/// 点マーカーの位置・コード・回転・尺度・色を置き換え、変換で動かし、点マーカーを足す。読み直すと同じ点になる。
/// 鏡に映す変換と、名前空間 `sxf.marker` でない点・文書の点の表し方に従う点・線種を持つ点は断り、ファイルを変えない
/// （決定 0183）
#[test]
fn point_markers_are_replaced_transformed_and_added() {
    use zumen_drawing::{MarkerKey, Point, PointLook};
    let features: [&[u8]; 3] = [
        b"layer_feature(\\'A\\','1')",
        b"point_marker_feature('1','1','5.0','6.0','2','0.0','1.0')",
        b"drawing_sheet_feature(\\'S\\','3','1','0','0')",
    ];
    let mut src = b"ISO-10303-21;\r\nHEADER;\r\nFILE_DESCRIPTION(('SCADEC level2 feature_mode'),'2;1');\r\nFILE_SCHEMA(('ASSOCIATIVE_DRAUGHTING'));\r\nENDSEC;\r\nDATA;\r\n".to_vec();
    for (at, feature) in features.iter().enumerate() {
        src.extend_from_slice(format!("/*SXF\r\n#{} = ", (at + 1) * 10).as_bytes());
        src.extend_from_slice(feature);
        src.extend_from_slice(b"\r\nSXF*/\r\n");
    }
    src.extend_from_slice(b"ENDSEC;\r\nEND-ISO-10303-21;\r\n");
    let drawing = lift(&read(&src));
    let (target, marker) = seen(&drawing, 20);
    let Shape::Point(point) = &marker.shape else {
        panic!("{:?}", marker.shape);
    };
    let with = |dot: Point<PaperPoint>| Element {
        shape: Shape::Point(dot),
        ..marker.clone()
    };
    let points = |file: &SxfFile| -> Vec<Element<PaperPoint>> {
        lift(file).sheets[0]
            .elements
            .iter()
            .filter(|element| matches!(element.shape, Shape::Point(_)))
            .cloned()
            .collect()
    };
    let key = |namespace, code: &str| MarkerKey {
        namespace,
        code: code.to_owned(),
    };

    let changed = Point {
        position: paper(50.0, 60.0),
        rotation: 30.0,
        look: PointLook::Marker {
            key: key("sxf.marker".into(), "4"),
            scale: 2.5,
        },
        construction: false,
    };
    let replaced = Element {
        style: Style {
            color: sxf_color(2),
            ..marker.style
        },
        ..with(changed.clone())
    };
    let mut file = read(&src);
    let (result, diagnostics) = apply(
        &mut file,
        vec![Edit::Replace {
            target: target.clone(),
            element: Box::new(Placed::Paper(replaced.clone())),
        }],
    );
    assert!(result.is_ok(), "{diagnostics:?}");
    let found = points(&file);
    assert_eq!(found[0].shape, replaced.shape);
    assert_eq!(found[0].style, replaced.style);

    let mut file = read(&src);
    let (result, diagnostics) = apply(
        &mut file,
        vec![Edit::Transform {
            target: target.clone(),
            transform: Affine::translation(10.0, 0.0),
        }],
    );
    assert!(result.is_ok(), "{diagnostics:?}");
    assert_eq!(
        points(&file)[0].shape,
        Shape::Point(Point {
            position: paper(15.0, 6.0),
            ..point.clone()
        })
    );

    let mut file = read(&src);
    let (result, diagnostics) = apply(
        &mut file,
        vec![Edit::Add {
            place: id::sheet(&drawing.sheets[0]),
            element: Box::new(Placed::Paper(with(changed.clone()))),
        }],
    );
    assert!(result.is_ok(), "{diagnostics:?}");
    let found = points(&file);
    assert_eq!(found.len(), 2);
    assert_eq!(found[1].shape, Shape::Point(changed.clone()));
    assert_eq!(found[1].style, marker.style);

    // X を反転する鏡映。既定義の印は鏡映を表せない
    let mirror = Affine {
        m: [-1.0, 0.0, 0.0, 0.0, 1.0, 0.0],
    };
    let document = Point {
        look: PointLook::Document,
        ..changed.clone()
    };
    let foreign = Point {
        look: PointLook::Marker {
            key: key("jww.point".into(), "4"),
            scale: 1.0,
        },
        ..changed.clone()
    };
    let lined = Element {
        style: Style {
            line_type: Inherit::Own(LineTypeId(1)),
            ..marker.style
        },
        ..with(changed)
    };
    for edit in [
        Edit::Transform {
            target: target.clone(),
            transform: mirror,
        },
        Edit::Replace {
            target: target.clone(),
            element: Box::new(Placed::Paper(with(document.clone()))),
        },
        Edit::Replace {
            target: target.clone(),
            element: Box::new(Placed::Paper(lined)),
        },
        Edit::Add {
            place: id::sheet(&drawing.sheets[0]),
            element: Box::new(Placed::Paper(with(document))),
        },
        Edit::Add {
            place: id::sheet(&drawing.sheets[0]),
            element: Box::new(Placed::Paper(with(foreign))),
        },
    ] {
        let mut file = read(&src);
        let (result, diagnostics) = apply(&mut file, vec![edit]);
        assert_eq!(result, Err(Error::Rejected));
        assert_eq!(diagnostics, [codes::CHANGE_EDIT_UNSUPPORTED]);
        assert_eq!(write(&file), src);
    }
}

/// SXF は文書の点の表し方（DXF・DWG の $PDMODE・$PDSIZE）に当たるものを持たない。点は既定義の印だけなので、その
/// 変更を断り、ファイルを変えない（決定 0190）
#[test]
fn point_styles_are_refused() {
    use zumen_drawing::{PointMark, PointSize, PointStyle};
    let src = src();
    let mut file = read(&src);
    let (result, diagnostics) = apply(
        &mut file,
        vec![Edit::SetPointStyle {
            style: PointStyle {
                mark: PointMark::Plus,
                circle: false,
                square: false,
                size: PointSize::Length(1.0),
            },
        }],
    );
    assert_eq!(result, Err(Error::Rejected));
    assert_eq!(diagnostics, [codes::CHANGE_EDIT_UNSUPPORTED]);
    assert_eq!(write(&file), src);
}
