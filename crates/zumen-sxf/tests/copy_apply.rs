//! SFC と P21 の要素を複写し（`Edit::Copy` / `SxfFile::applied_with_copies`）、写しが元の属性・拡張・関連情報を
//! 保つことと、保てない場合（複合曲線・インライン属性・SAF・共有点など）の理由付き全件拒否・予算停止時の原子性を確かめる（#889）。
#![cfg(feature = "drawing")]

use zumen_base::{Budget, CancelToken, Cx, Diagnostics, Kind, Limit, Limits, codes};
use zumen_drawing::{
    Affine, Applied, Changes, Drawing2D, Edit, Element, Field, FieldValue, Hash, PaperPoint, Seen,
    Shape, Target, id, transform,
};
use zumen_sdk::{Apply, Codec, Lift, testkit};
use zumen_sxf::{CopyFrom, Sxf, SxfFile, SxfLive};

const SHIFT: Affine = Affine {
    m: [1.0, 0.0, 12.0, 0.0, 1.0, 18.0],
};

const SCALE_SHIFT: Affine = Affine {
    m: [2.0, 0.0, 10.0, 0.0, 2.0, 20.0],
};

/// SFC テスト入力: レイヤ・利用者定義色・線幅・線種・フォント・部分図定義と、
/// 線分・円・円弧・折線・点マーカー・文字（幅と字間つき）・部分図配置・複合曲線の構成要素・インライン属性要素・SAF 結合要素を持つ
fn source_sfc() -> Vec<u8> {
    let features: &[&[u8]] = &[
        b"layer_feature(\\'A\\','1')",
        b"layer_feature(\\'B\\','1')",
        b"user_defined_colour_feature('10','20','30')",
        b"width_feature('0.35')",
        b"user_defined_font_feature(\\'dash\\','2','(2.0,1.0)')",
        b"text_font_feature(\\'MS\\')",
        b"line_feature('1','1','1','1','0.0','0.0','5.0','0.0')",
        b"sfig_org_feature(\\'PART\\','4')",
        b"line_feature('1','1','1','1','0.0','0.0','1.0','1.0')",
        b"sfig_org_feature(\\'$$ATRU$$1$$k=v\\','3')",
        b"line_feature('1','1','1','1','0.0','0.0','2.0','2.0')",
        b"sfig_org_feature(\\'$$ATRF$$10$$A.SAF\\','3')",
        // 複合曲線の構成要素（拒否対象、用紙要素より先に定義）
        b"line_feature('1','1','1','1','20.0','0.0','30.0','0.0')",
        b"composite_curve_org_feature('1','1','1','1')",
        // 用紙上の複写可能要素（7 件: index 1..=7）
        b"line_feature('1','17','17','11','0.0','0.0','10.0','0.0')",
        b"circle_feature('2','2','1','2','5.0','5.0','3.0')",
        b"arc_feature('1','1','1','1','0.0','0.0','5.0','0','0.0','90.0')",
        b"polyline_feature('1','1','1','1','3','(0,1,2)','(0,1,0)')",
        b"point_marker_feature('1','2','1.0','2.0','3','15.0','2.5')",
        b"text_string_feature('1','1','1',\\'SFC\\','2.0','3.0','4.0','12.0','0.5','0.0','0.0','1','1')",
        b"sfig_locate_feature('1',\\'PART\\','4.0','6.0','0.0','1.0','1.0')",
        // インライン属性 sfig_locate（拒否対象: index 8）
        b"sfig_locate_feature('1',\\'$$ATRU$$1$$k=v\\','0.0','0.0','0.0','1.0','1.0')",
        // SAF 属性結合 sfig_locate（拒否対象: index 9）
        b"sfig_locate_feature('1',\\'$$ATRF$$10$$A.SAF\\','0.0','0.0','0.0','1.0','1.0')",
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

/// P21 テスト入力: 線分・円・円弧・折線・文字（幅と字間つき）・共有点を持つ線分・複合曲線を持つ
fn source_p21() -> Vec<u8> {
    let data = [
        "#10=(LENGTH_UNIT()NAMED_UNIT(*)SI_UNIT(.MILLI.,.METRE.));",
        "#20=DRAUGHTING_PRE_DEFINED_COLOUR('red');",
        "#30=DRAUGHTING_PRE_DEFINED_CURVE_FONT('continuous');",
        "#40=LENGTH_MEASURE_WITH_UNIT(POSITIVE_LENGTH_MEASURE(0.25),#10);",
        "#60=CURVE_STYLE(' ',#30,#40,#20);",
        "#70=PRESENTATION_STYLE_ASSIGNMENT((#60));",
        "#80=EXTERNALLY_DEFINED_TEXT_FONT('MS',$);",
        "#81=TEXT_STYLE_FOR_DEFINED_FONT(#20);",
        "#82=(TEXT_STYLE('style name',#81)TEXT_STYLE_WITH_BOX_CHARACTERISTICS((BOX_HEIGHT(4.),BOX_WIDTH(12.),BOX_SLANT_ANGLE(0.),BOX_ROTATE_ANGLE(0.)))TEXT_STYLE_WITH_SPACING(LENGTH_MEASURE(0.5)));",
        "#83=PRESENTATION_STYLE_ASSIGNMENT((#82));",
        // #160: 線分（独立点 #100, #110）
        "#100=CARTESIAN_POINT(' ',(0.,0.));",
        "#110=CARTESIAN_POINT(' ',(10.,0.));",
        "#120=DIRECTION(' ',(1.,0.));",
        "#130=VECTOR(' ',#120,10.);",
        "#140=LINE(' ',#100,#130);",
        "#150=TRIMMED_CURVE(' ',#140,(#100),(#110),.T.,.CARTESIAN.);",
        "#160=ANNOTATION_CURVE_OCCURRENCE(' ',(#70),#150);",
        // #230: 円
        "#200=CARTESIAN_POINT(' ',(5.,5.));",
        "#210=AXIS2_PLACEMENT_2D(' ',#200,$);",
        "#220=CIRCLE(' ',#210,3.);",
        "#230=ANNOTATION_CURVE_OCCURRENCE(' ',(#70),#220);",
        // #340: 円弧
        "#300=CARTESIAN_POINT(' ',(0.,0.));",
        "#310=AXIS2_PLACEMENT_2D(' ',#300,$);",
        "#320=CIRCLE(' ',#310,5.);",
        "#330=TRIMMED_CURVE(' ',#320,(PARAMETER_VALUE(0.)),(PARAMETER_VALUE(1.5707963267948966)),.T.,.PARAMETER.);",
        "#340=ANNOTATION_CURVE_OCCURRENCE(' ',(#70),#330);",
        // #440: 折線
        "#400=CARTESIAN_POINT(' ',(0.,0.));",
        "#410=CARTESIAN_POINT(' ',(1.,1.));",
        "#420=CARTESIAN_POINT(' ',(2.,0.));",
        "#430=POLYLINE(' ',(#400,#410,#420));",
        "#440=ANNOTATION_CURVE_OCCURRENCE(' ',(#70),#430);",
        // #490: 文字（幅 12, 字間 0.5）
        "#460=CARTESIAN_POINT('',(2.,3.));",
        "#470=AXIS2_PLACEMENT_2D('',#460,$);",
        "#480=PLANAR_EXTENT('',12.,4.);",
        "#485=TEXT_LITERAL_WITH_EXTENT('$$SXF_baseline left','P21',#470,'baseline left',.RIGHT.,#80,#480);",
        "#490=ANNOTATION_TEXT_OCCURRENCE('',(#83),#485);",
        // #560, #590: 始点 #500 を共有する 2 本の線分（複写時は新規インスタンスを作るので元を壊さず複写できる）
        "#500=CARTESIAN_POINT(' ',(1.,1.));",
        "#510=CARTESIAN_POINT(' ',(1.,11.));",
        "#520=DIRECTION(' ',(0.,1.));",
        "#530=VECTOR(' ',#520,10.);",
        "#540=LINE(' ',#500,#530);",
        "#550=TRIMMED_CURVE(' ',#540,(#500),(#510),.T.,.CARTESIAN.);",
        "#560=ANNOTATION_CURVE_OCCURRENCE(' ',(#70),#550);",
        "#570=CARTESIAN_POINT(' ',(11.,1.));",
        "#575=LINE(' ',#500,#130);",
        "#580=TRIMMED_CURVE(' ',#575,(#500),(#570),.T.,.CARTESIAN.);",
        "#590=ANNOTATION_CURVE_OCCURRENCE(' ',(#70),#580);",
        // #650: 複合曲線（拒否対象）
        "#600=CARTESIAN_POINT(' ',(20.,0.));",
        "#610=CARTESIAN_POINT(' ',(30.,0.));",
        "#620=POLYLINE(' ',(#600,#610));",
        "#630=COMPOSITE_CURVE_SEGMENT(.CONTINUOUS.,.T.,#620);",
        "#640=COMPOSITE_CURVE(' ',(#630),.F.);",
        "#650=ANNOTATION_CURVE_OCCURRENCE(' ',(#70),#640);",
        "#700=GEOMETRIC_REPRESENTATION_CONTEXT(' ',' ',2);",
        "#710=DRAUGHTING_SUBFIGURE_REPRESENTATION('$$SXF_P_Part',(#210),#700);",
        "#720=SYMBOL_REPRESENTATION_MAP(#210,#710);",
        "#730=SYMBOL_TARGET(' ',#210,1.,1.);",
        "#740=(ANNOTATION_SYMBOL()GEOMETRIC_REPRESENTATION_ITEM()MAPPED_ITEM(#720,#730)REPRESENTATION_ITEM(' '));",
        "#750=PRESENTATION_STYLE_ASSIGNMENT((NULL_STYLE(.NULL.)));",
        "#760=(ANNOTATION_OCCURRENCE()ANNOTATION_SUBFIGURE_OCCURRENCE()ANNOTATION_SYMBOL_OCCURRENCE()DRAUGHTING_ANNOTATION_OCCURRENCE()GEOMETRIC_REPRESENTATION_ITEM()REPRESENTATION_ITEM(' ')STYLED_ITEM((#750),#740));",
        "#900=PRESENTATION_LAYER_ASSIGNMENT('A',' ',(#160,#230,#340,#440,#490,#650,#760));",
        "#910=PRESENTATION_LAYER_ASSIGNMENT('B',' ',(#560,#590));",
        "#950=DRAWING_SHEET_REVISION('A3_horizontal',(#160,#230,#340,#440,#490,#560,#590,#650,#760),' ',' ');",
    ];
    let mut src = "ISO-10303-21;\nHEADER;\nFILE_DESCRIPTION(('SCADEC level2 AP202_mode'),'2;1');\nFILE_SCHEMA(('ASSOCIATIVE_DRAUGHTING'));\nENDSEC;\nDATA;\n".to_owned();
    for line in data {
        src.push_str(line);
        src.push('\n');
    }
    src.push_str("ENDSEC;\nEND-ISO-10303-21;\n");
    src.into_bytes()
}

fn cx_run<T>(f: impl FnOnce(&mut Cx<'_>) -> T) -> (T, Diagnostics) {
    let mut budget = Budget::default();
    let mut diag = Diagnostics::new();
    let cancel = CancelToken::new();
    let value = f(&mut Cx::new(&mut budget, &mut diag, &cancel));
    (value, diag)
}

#[cfg(test)]
fn read(src: &[u8]) -> SxfFile {
    testkit::read::<Sxf>(src).0.unwrap()
}

#[cfg(test)]
fn write(file: &SxfFile) -> Vec<u8> {
    cx_run(|cx| {
        let mut out = Vec::new();
        Sxf::write(file, &mut out, cx).unwrap();
        out
    })
    .0
}

#[cfg(test)]
fn lift(file: &SxfFile) -> (Drawing2D, Diagnostics) {
    cx_run(|cx| file.lift(cx).unwrap())
}

#[cfg(test)]
fn paper_elements(drawing: &Drawing2D) -> Vec<(String, Hash, Element<PaperPoint>)> {
    let mut out = Vec::new();
    id::each_element(drawing, |_, elem_id, found| {
        if let id::Found::Paper(el) = found {
            out.push((elem_id, Hash::element(el), el.clone()));
        }
        Ok::<_, ()>(())
    })
    .unwrap();
    out
}

fn element_extensions(drawing: &Drawing2D, elem_id: &str) -> Vec<(String, Vec<Field>)> {
    drawing
        .extensions
        .iter()
        .filter(|ext| ext.target == Target::Element(elem_id.to_owned()))
        .map(|ext| (ext.namespace.clone().into_owned(), ext.fields.clone()))
        .collect()
}

#[cfg(test)]
fn compare_live(
    bytes: &[u8],
    changes: &Changes,
    expected: &[Applied],
    diag: &Diagnostics,
    out: &[u8],
) {
    for threads in [1, 4] {
        let mut live = cx_run(|cx| SxfLive::open_with_threads(bytes, threads, cx).unwrap()).0;
        let (result, diagnostics) = cx_run(|cx| live.apply(changes, cx));
        assert_eq!(
            result.unwrap_or_else(|e| panic!("{e:?}: {diagnostics:?}")),
            expected
        );
        assert_eq!(
            diagnostics.iter().collect::<Vec<_>>(),
            diag.iter().collect::<Vec<_>>()
        );
        assert_eq!(cx_run(|cx| live.save(cx).unwrap()).0, out);
        assert_eq!(cx_run(|cx| live.drawing(cx).unwrap()).0, lift(&read(out)).0);
        assert!(cx_run(|cx| live.undo(cx).unwrap()).0);
        assert_eq!(cx_run(|cx| live.save(cx).unwrap()).0, bytes);
        assert!(cx_run(|cx| live.redo(cx).unwrap()).0);
        assert_eq!(cx_run(|cx| live.save(cx).unwrap()).0, out);
    }
}

#[test]
fn copies_keep_shapes_styles_and_extensions_in_sfc_and_p21() {
    // 1. SFC の 7 要素（index 1..8: 線・円・円弧・折線・点マーカー・文字・部分図配置）を複写
    let sfc = source_sfc();
    let mut sfc_file = read(&sfc);
    let (sfc_before, _) = lift(&sfc_file);
    let sfc_was = paper_elements(&sfc_before);
    let sfc_targets: Vec<(String, Hash, Affine)> = sfc_was[1..8]
        .iter()
        .map(|(id, hash, el)| {
            let tf = if matches!(el.shape, Shape::Formatted(_)) {
                SCALE_SHIFT
            } else {
                SHIFT
            };
            (id.clone(), *hash, tf)
        })
        .collect();
    let sfc_edits: Vec<Edit> = sfc_targets
        .iter()
        .map(|(id, hash, tf)| Edit::Copy {
            target: Seen {
                id: id.clone(),
                hash: *hash,
            },
            transform: *tf,
        })
        .collect();
    let sfc_changes = Changes {
        document: None,
        edits: sfc_edits,
    };
    let (sfc_res, sfc_diag) = cx_run(|cx| sfc_file.apply(&sfc_changes, cx));
    let sfc_applied = sfc_res.unwrap_or_else(|e| panic!("{e:?}: {sfc_diag:?}"));
    // 文字幅の近似診断（SXF_TEXT_WIDTH_APPROXIMATED）を出さずに元の幅・字間を拡大して保つ
    assert!(
        !sfc_diag
            .iter()
            .any(|d| d.code() == codes::SXF_TEXT_WIDTH_APPROXIMATED),
        "{sfc_diag:?}"
    );
    assert_eq!(sfc_applied.len(), 7);

    let sfc_out = write(&sfc_file);
    compare_live(&sfc, &sfc_changes, &sfc_applied, &sfc_diag, &sfc_out);
    let (sfc_after, _) = lift(&read(&sfc_out));
    let sfc_now = paper_elements(&sfc_after);
    for (id, hash, _) in &sfc_was {
        assert!(
            sfc_now.iter().any(|(i, h, _)| i == id && h == hash),
            "changed: {id}"
        );
    }
    assert_eq!(sfc_now.len(), sfc_was.len() + 7);
    for (result, (src_id, _, tf)) in sfc_applied.iter().zip(&sfc_targets) {
        let (_, _, orig) = sfc_was.iter().find(|(i, ..)| i == src_id).unwrap();
        let new_id = result.id.as_deref().unwrap();
        let (_, new_hash, copied) = sfc_now.iter().find(|(i, ..)| i == new_id).unwrap();
        assert_eq!(result.hash, Some(*new_hash), "{new_id}");
        assert_eq!(copied.layer, orig.layer, "{new_id}");
        assert_eq!(copied.style, orig.style, "{new_id}");
        assert_eq!(
            Some(copied.shape.clone()),
            transform::transform(&orig.shape, tf),
            "{new_id}"
        );
    }
    // 文字（index 5）の sxf.text 拡張（width=24.0, spacing=1.0）が保たれている
    let copied_sfc_text = sfc_applied[5].id.as_deref().unwrap();
    let sfc_text_exts = element_extensions(&sfc_after, copied_sfc_text);
    let sxf_text = sfc_text_exts
        .iter()
        .find(|(ns, _)| ns == "sxf.text")
        .unwrap();
    assert_eq!(sxf_text.1[0].value, FieldValue::Real(24.0));
    assert_eq!(sxf_text.1[1].value, FieldValue::Real(1.0));

    // 2. P21 の線・円・円弧・折線・文字・共有点を持つ線と作図部品の配置を複写
    let p21 = source_p21();
    let mut p21_file = read(&p21);
    let (p21_before, _) = lift(&p21_file);
    let p21_was = paper_elements(&p21_before);
    let p21_targets: Vec<(String, Hash, Affine)> = p21_was[1..7]
        .iter()
        .chain(
            p21_was
                .iter()
                .filter(|(_, _, e)| matches!(e.shape, Shape::Symbol(_))),
        )
        .map(|(id, hash, el)| {
            let tf = if matches!(el.shape, Shape::Formatted(_)) {
                SCALE_SHIFT
            } else {
                SHIFT
            };
            (id.clone(), *hash, tf)
        })
        .collect();
    let p21_edits: Vec<Edit> = p21_targets
        .iter()
        .map(|(id, hash, tf)| Edit::Copy {
            target: Seen {
                id: id.clone(),
                hash: *hash,
            },
            transform: *tf,
        })
        .collect();
    let p21_changes = Changes {
        document: None,
        edits: p21_edits,
    };
    let (p21_res, p21_diag) = cx_run(|cx| p21_file.apply(&p21_changes, cx));
    let p21_applied = p21_res.unwrap_or_else(|e| panic!("{e:?}: {p21_diag:?}"));
    assert!(
        !p21_diag
            .iter()
            .any(|d| d.code() == codes::SXF_TEXT_WIDTH_APPROXIMATED),
        "{p21_diag:?}"
    );
    assert_eq!(p21_applied.len(), 7);
    let p21_out = write(&p21_file);
    compare_live(&p21, &p21_changes, &p21_applied, &p21_diag, &p21_out);
    let (p21_after, _) = lift(&read(&p21_out));
    let p21_now = paper_elements(&p21_after);
    for (id, hash, _) in &p21_was {
        assert!(
            p21_now.iter().any(|(i, h, _)| i == id && h == hash),
            "changed: {id}"
        );
    }
    let copied_p21_text = p21_applied[4].id.as_deref().unwrap();
    let p21_text_exts = element_extensions(&p21_after, copied_p21_text);
    let p21_sxf_text = p21_text_exts
        .iter()
        .find(|(ns, _)| ns == "sxf.text")
        .unwrap();
    assert_eq!(p21_sxf_text.1[0].value, FieldValue::Real(24.0));
    assert_eq!(p21_sxf_text.1[1].value, FieldValue::Real(1.0));
}

#[test]
fn unsupported_sxf_copies_are_refused_whole_without_mutation() {
    let sfc = source_sfc();
    let base_file = read(&sfc);
    let (before, _) = lift(&base_file);
    let was = paper_elements(&before);

    // index 0: 複合曲線, index 8: インライン属性 $$ATRU$$, index 9: SAF $$ATRF$$
    for (bad_idx, expected_val) in [
        (0usize, "composite_curve"),
        (8usize, "attribute"),
        (9usize, "saf"),
    ] {
        let mut file = read(&sfc);
        let changes = Changes {
            document: None,
            edits: vec![
                Edit::Copy {
                    target: Seen {
                        id: was[1].0.clone(),
                        hash: was[1].1,
                    },
                    transform: SHIFT,
                },
                Edit::Copy {
                    target: Seen {
                        id: was[bad_idx].0.clone(),
                        hash: was[bad_idx].1,
                    },
                    transform: SHIFT,
                },
            ],
        };
        let (res, diag) = cx_run(|cx| file.apply(&changes, cx));
        let mut live = cx_run(|cx| SxfLive::open(&sfc, cx).unwrap()).0;
        let (refused, live_diag) = cx_run(|cx| live.apply(&changes, cx));
        assert!(refused.is_err());
        assert_eq!(
            live_diag.iter().collect::<Vec<_>>(),
            diag.iter().collect::<Vec<_>>()
        );
        assert_eq!(cx_run(|cx| live.save(cx).unwrap()).0, sfc);
        assert!(!cx_run(|cx| live.undo(cx).unwrap()).0);
        assert!(res.is_err(), "{expected_val}");
        assert!(
            diag.iter().any(|d| {
                d.code() == codes::CHANGE_COPY_UNSUPPORTED
                    && d.param("value") == Some(&expected_val.into())
            }),
            "{expected_val}: {diag:?}"
        );
        assert_eq!(write(&file), sfc);
    }

    // 非用紙（部分図空間 space/0）への配置は拒む
    let changes = Changes {
        document: None,
        edits: vec![Edit::Copy {
            target: Seen {
                id: was[1].0.clone(),
                hash: was[1].1,
            },
            transform: SHIFT,
        }],
    };
    let (res, diag) = cx_run(|cx| {
        base_file.applied_with_copies(
            &changes,
            &[CopyFrom {
                place: Some("space/0"),
                from: None,
            }],
            cx,
        )
    });
    assert!(res.is_err());
    let mut live = cx_run(|cx| SxfLive::open(&sfc, cx).unwrap()).0;
    let (refused, observed) = cx_run(|cx| live.apply_with_copy_place(&changes, "space/0", cx));
    assert!(refused.is_err());
    assert_eq!(
        observed.iter().collect::<Vec<_>>(),
        diag.iter().collect::<Vec<_>>()
    );
    assert_eq!(cx_run(|cx| live.save(cx).unwrap()).0, sfc);
    let (missing, observed) = cx_run(|cx| live.apply_with_copy_place(&changes, "space/999999", cx));
    assert!(missing.is_err());
    assert!(
        observed
            .iter()
            .any(|d| d.code() == codes::CHANGE_ELEMENT_MISSING)
    );
    assert!(
        cx_run(|cx| live.apply_with_copy_place(&changes, "sheet/0", cx))
            .0
            .is_ok()
    );

    assert!(diag.iter().any(|d| {
        d.code() == codes::CHANGE_COPY_UNSUPPORTED && d.param("field") == Some(&"place".into())
    }));
}

#[test]
fn cross_document_sxf_copy_and_budget_stop_atomicity() {
    let sfc = source_sfc();
    let src_file = read(&sfc);
    let (before, _) = lift(&src_file);
    let was = paper_elements(&before);

    // 同じ表・部分図定義を持つ別文書へは複写できる
    let dst_file = read(&sfc);
    let (res, diag) = cx_run(|cx| {
        dst_file.applied_with_copies(
            &Changes {
                document: None,
                edits: vec![
                    Edit::Copy {
                        target: Seen {
                            id: was[1].0.clone(),
                            hash: was[1].1,
                        },
                        transform: SHIFT,
                    },
                    Edit::Copy {
                        target: Seen {
                            id: was[7].0.clone(),
                            hash: was[7].1,
                        },
                        transform: SHIFT,
                    },
                ],
            },
            &[
                CopyFrom {
                    place: None,
                    from: Some(&src_file),
                },
                CopyFrom {
                    place: None,
                    from: Some(&src_file),
                },
            ],
            cx,
        )
    });
    let (_, applied) = res.unwrap_or_else(|e| panic!("{e:?}: {diag:?}"));
    assert_eq!(applied.len(), 2);

    // 利用者定義色が異なる別文書へは color = mismatch で拒む
    let diff_sfc = String::from_utf8(sfc.clone())
        .unwrap()
        .replace(
            "user_defined_colour_feature('10','20','30')",
            "user_defined_colour_feature('99','20','30')",
        )
        .into_bytes();
    let dst_bad = read(&diff_sfc);
    let (res, diag) = cx_run(|cx| {
        dst_bad.applied_with_copies(
            &Changes {
                document: None,
                edits: vec![Edit::Copy {
                    target: Seen {
                        id: was[1].0.clone(),
                        hash: was[1].1,
                    },
                    transform: SHIFT,
                }],
            },
            &[CopyFrom {
                place: None,
                from: Some(&src_file),
            }],
            cx,
        )
    });
    assert!(res.is_err());
    assert!(
        diag.iter()
            .any(|d| d.code() == codes::CHANGE_COPY_UNSUPPORTED)
    );

    // 予算停止時の原子性
    let run = |limit: Option<u64>, resident: bool| -> (bool, u64, Vec<u8>) {
        let edits = vec![
            Edit::Copy {
                target: Seen {
                    id: was[1].0.clone(),
                    hash: was[1].1,
                },
                transform: SHIFT,
            },
            Edit::Copy {
                target: Seen {
                    id: was[6].0.clone(),
                    hash: was[6].1,
                },
                transform: SCALE_SHIFT,
            },
        ];
        let limits = match limit {
            Some(w) => Limits::default().with(Kind::Work, Limit::Finite(w)),
            None => Limits::default(),
        };
        let mut budget = Budget::new(limits);
        let mut diag = Diagnostics::new();
        let cancel = CancelToken::new();
        let changes = Changes {
            document: None,
            edits,
        };
        let cx = &mut Cx::new(&mut budget, &mut diag, &cancel);
        let (ok, bytes) = if resident {
            let mut live = cx_run(|cx| SxfLive::open(&sfc, cx).unwrap()).0;
            let ok = live.apply(&changes, cx).is_ok();
            (ok, cx_run(|cx| live.save(cx).unwrap()).0)
        } else {
            let mut file = read(&sfc);
            let ok = file.apply(&changes, cx).is_ok();
            (ok, write(&file))
        };
        (ok, budget.used(Kind::Work), bytes)
    };
    for resident in [false, true] {
        let (done, used, _) = run(None, resident);
        assert!(done);
        for limit in [used / 4, used / 2, used.saturating_sub(1)] {
            let (done, _, written) = run(Some(limit), resident);
            assert!(!done, "{limit}");
            assert_eq!(written, sfc, "{limit}");
        }
    }
}
