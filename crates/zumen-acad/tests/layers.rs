//! レイヤの置き換えと追加を、DXF の画層の表に書き戻す（決定 0162）。
#![cfg(feature = "drawing")]

use zumen_acad::{Dxf, DxfFile};
use zumen_base::{Budget, CancelToken, Code, Cx, Diagnostics, codes};
use zumen_drawing::{
    Applied, Changes, Color, ColorCode, ColorSystem, Drawing2D, Edit, Hash, Layer, LayerState,
    LineWeight, PaperLength, Restyle, Seen, id,
};
use zumen_sdk::{Apply, Error, Lift, testkit};

/// 画層 0・A・Unused を持ち、線が A に、円が小文字の a にある R2000 の DXF。今の画層は A
#[cfg(test)]
fn src() -> Vec<u8> {
    let tags: &[(i16, &str)] = &[
        (0, "SECTION"),
        (2, "HEADER"),
        (9, "$ACADVER"),
        (1, "AC1015"),
        (9, "$HANDSEED"),
        (5, "40"),
        (9, "$CLAYER"),
        (8, "A"),
        (0, "ENDSEC"),
        (0, "SECTION"),
        (2, "TABLES"),
        (0, "TABLE"),
        (2, "LTYPE"),
        (5, "5"),
        (70, "1"),
        (0, "LTYPE"),
        (5, "14"),
        (330, "5"),
        (100, "AcDbSymbolTableRecord"),
        (100, "AcDbLinetypeTableRecord"),
        (2, "Continuous"),
        (70, "0"),
        (3, "Solid line"),
        (72, "65"),
        (73, "0"),
        (40, "0.0"),
        (0, "ENDTAB"),
        (0, "TABLE"),
        (2, "LAYER"),
        (5, "2"),
        (70, "2"),
        (0, "LAYER"),
        (5, "10"),
        (330, "2"),
        (100, "AcDbSymbolTableRecord"),
        (100, "AcDbLayerTableRecord"),
        (2, "0"),
        (70, "0"),
        (62, "7"),
        (6, "Continuous"),
        (370, "-3"),
        (0, "LAYER"),
        (5, "11"),
        (330, "2"),
        (100, "AcDbSymbolTableRecord"),
        (100, "AcDbLayerTableRecord"),
        (2, "A"),
        (70, "0"),
        (62, "1"),
        (6, "Continuous"),
        (370, "-3"),
        (0, "LAYER"),
        (5, "12"),
        (330, "2"),
        (100, "AcDbSymbolTableRecord"),
        (100, "AcDbLayerTableRecord"),
        (2, "Unused"),
        (70, "0"),
        (62, "2"),
        (6, "Continuous"),
        (370, "-3"),
        (0, "ENDTAB"),
        (0, "ENDSEC"),
        (0, "SECTION"),
        (2, "ENTITIES"),
        (0, "LINE"),
        (5, "1A"),
        (100, "AcDbEntity"),
        (8, "A"),
        (100, "AcDbLine"),
        (10, "0.0"),
        (20, "0.0"),
        (30, "0.0"),
        (11, "1.0"),
        (21, "1.0"),
        (31, "0.0"),
        (0, "CIRCLE"),
        (5, "1B"),
        (100, "AcDbEntity"),
        (8, "a"),
        (100, "AcDbCircle"),
        (10, "5.0"),
        (20, "5.0"),
        (30, "0.0"),
        (40, "2.0"),
        (0, "ENDSEC"),
        (0, "EOF"),
    ];
    tags.iter()
        .map(|(code, value)| format!("{code:>3}\n{value}\n"))
        .collect::<String>()
        .into_bytes()
}

#[cfg(test)]
fn lift(file: &DxfFile) -> Drawing2D {
    let mut budget = Budget::default();
    let mut diag = Diagnostics::new();
    file.lift(&mut Cx::new(&mut budget, &mut diag, &CancelToken::new()))
        .unwrap()
}

#[cfg(test)]
fn apply(file: &mut DxfFile, edits: Vec<Edit>) -> (Result<Vec<Applied>, Error>, Vec<Code>) {
    let mut budget = Budget::default();
    let mut diag = Diagnostics::new();
    let result = file.apply(
        &Changes {
            edits,
            document: None,
        },
        &mut Cx::new(&mut budget, &mut diag, &CancelToken::new()),
    );
    (result, diag.iter().map(|d| d.code()).collect())
}

#[cfg(test)]
fn layer<'a>(drawing: &'a Drawing2D, name: &str) -> &'a Layer {
    drawing
        .layers
        .iter()
        .find(|layer| layer.name == name)
        .unwrap()
}

#[cfg(test)]
fn seen(layer: &Layer) -> Seen {
    Seen {
        id: id::layer(layer),
        hash: Hash::layer(layer),
    }
}

#[cfg(test)]
fn aci(value: u32) -> Option<Color> {
    Some(Color {
        code: Some(ColorCode {
            system: ColorSystem::Aci,
            value,
        }),
        rgb: None,
    })
}

#[test]
fn layers_are_renamed_restyled_and_their_entities_follow() {
    let mut file = testkit::read::<Dxf>(&src()).0.unwrap();
    let drawing = lift(&file);
    let was = layer(&drawing, "A");
    let mut new = was.clone();
    new.name = "壁".to_owned();
    new.state = Some(LayerState {
        visible: false,
        printable: false,
        editable: false,
        frozen: true,
    });
    let mut style = new.style.unwrap();
    style.color = aci(3);
    style.line_weight = Some(LineWeight::Width(PaperLength(0.35)));
    new.style = Some(style);
    let (result, codes) = apply(
        &mut file,
        vec![Edit::ReplaceLayer {
            target: seen(was),
            layer: Box::new(new.clone()),
        }],
    );
    // R2000 の DXF に ASCII でない名前は書けない
    assert_eq!(result.unwrap_err(), Error::Rejected);
    assert_eq!(codes, [codes::CHANGE_EDIT_UNSUPPORTED]);

    new.name = "Walls".to_owned();
    let (result, codes) = apply(
        &mut file,
        vec![Edit::ReplaceLayer {
            target: seen(was),
            layer: Box::new(new.clone()),
        }],
    );
    let applied = result.unwrap();
    assert!(codes.is_empty(), "{codes:?}");
    let written = testkit::write::<Dxf>(&file).unwrap();
    let text = String::from_utf8(written.clone()).unwrap();
    let after = lift(&testkit::read::<Dxf>(&written).0.unwrap());
    let renamed = layer(&after, "Walls");
    assert_eq!(renamed.state, new.state);
    assert_eq!(renamed.style, new.style);
    assert_eq!(id::layer(renamed), "layer/h11");
    assert_eq!(applied[0].id.as_deref(), Some("layer/h11"));
    assert_eq!(applied[0].hash, Some(Hash::layer(renamed)));
    // 線と円は名前を変えた画層にあり、今の画層も変わる。凍結した画層の色は負にしない
    let elements = &after.spaces[0].elements;
    assert!(elements.iter().all(|element| element.layer == renamed.id));
    assert!(text.contains("$CLAYER\n  8\nWalls\n"));
    assert!(!text.contains("\n  8\nA\n") && !text.contains("\n  8\na\n"));
    assert!(text.contains("  2\nWalls\n 70\n5\n 62\n3\n"));
    assert!(text.contains("290\n0\n370\n35\n"));
}

#[test]
fn layers_are_added_from_the_zero_layer() {
    let mut file = testkit::read::<Dxf>(&src()).0.unwrap();
    let drawing = lift(&file);
    let mut new = layer(&drawing, "0").clone();
    new.name = "Grid".to_owned();
    let mut style = new.style.unwrap();
    style.color = aci(5);
    new.style = Some(style);
    let (result, codes) = apply(
        &mut file,
        vec![Edit::AddLayer {
            layer: Box::new(new.clone()),
        }],
    );
    let applied = result.unwrap();
    assert!(codes.is_empty(), "{codes:?}");
    let written = testkit::write::<Dxf>(&file).unwrap();
    let after = lift(&testkit::read::<Dxf>(&written).0.unwrap());
    let added = layer(&after, "Grid");
    assert_eq!(added.style, new.style);
    assert_eq!(added.state, new.state);
    assert_eq!(applied[0].id.as_deref(), Some("layer/h40"));
    assert_eq!(applied[0].id.as_deref(), Some(id::layer(added).as_str()));
    assert_eq!(applied[0].hash, Some(Hash::layer(added)));
    // 足したレコードは画層の表の中にあり、ハンドルの種は進む
    let text = String::from_utf8(written).unwrap();
    let (_, rest) = text.split_once("  2\nGrid\n").unwrap();
    let (record, _) = rest.split_once("ENDTAB").unwrap();
    assert!(!record.contains("ENTITIES"));
    assert!(text.contains("$HANDSEED\n  5\n41\n"));
}

#[test]
fn layer_changes_that_cannot_be_written_change_nothing() {
    let src = src();
    let mut file = testkit::read::<Dxf>(&src).0.unwrap();
    let drawing = lift(&file);
    let zero = layer(&drawing, "0");
    let a = layer(&drawing, "A");
    let renamed = |from: &Layer, name: &str| {
        let mut layer = from.clone();
        layer.name = name.to_owned();
        Box::new(layer)
    };
    let mut visible_frozen = a.clone();
    visible_frozen.state = Some(LayerState {
        frozen: true,
        ..a.state.unwrap()
    });
    let stale = Seen {
        hash: Hash::layer(zero),
        ..seen(a)
    };
    let refused: Vec<(Vec<Edit>, Code)> = vec![
        // 名前が重なる。大文字と小文字は区別しない
        (
            vec![Edit::ReplaceLayer {
                target: seen(a),
                layer: renamed(a, "0"),
            }],
            codes::CHANGE_EDIT_UNSUPPORTED,
        ),
        (
            vec![Edit::AddLayer {
                layer: renamed(zero, "a"),
            }],
            codes::CHANGE_EDIT_UNSUPPORTED,
        ),
        // 画層 0 の名前は変えない。名前に使えない文字
        (
            vec![Edit::ReplaceLayer {
                target: seen(zero),
                layer: renamed(zero, "Base"),
            }],
            codes::CHANGE_EDIT_UNSUPPORTED,
        ),
        (
            vec![Edit::AddLayer {
                layer: renamed(zero, "a/b"),
            }],
            codes::CHANGE_EDIT_UNSUPPORTED,
        ),
        // 凍結して表示する画層はない
        (
            vec![Edit::ReplaceLayer {
                target: seen(a),
                layer: Box::new(visible_frozen),
            }],
            codes::CHANGE_EDIT_UNSUPPORTED,
        ),
        // 同じ画層を 2 度変える
        (
            vec![
                Edit::ReplaceLayer {
                    target: seen(a),
                    layer: renamed(a, "B"),
                },
                Edit::ReplaceLayer {
                    target: seen(a),
                    layer: renamed(a, "C"),
                },
            ],
            codes::CHANGE_EDIT_UNSUPPORTED,
        ),
        (
            vec![Edit::ReplaceLayer {
                target: stale,
                layer: renamed(a, "B"),
            }],
            codes::CHANGE_ELEMENT_STALE,
        ),
        (
            vec![Edit::ReplaceLayer {
                target: Seen {
                    id: "layer/h99".to_owned(),
                    ..seen(a)
                },
                layer: renamed(a, "B"),
            }],
            codes::CHANGE_ELEMENT_MISSING,
        ),
    ];
    for (edits, code) in refused {
        let (result, found) = apply(&mut file, edits.clone());
        assert_eq!(result.unwrap_err(), Error::Rejected, "{edits:?}");
        assert_eq!(found, [code], "{edits:?}");
        assert_eq!(testkit::write::<Dxf>(&file).unwrap(), src);
    }
}

#[test]
fn unused_layers_are_removed() {
    let mut file = testkit::read::<Dxf>(&src()).0.unwrap();
    let drawing = lift(&file);
    let unused = layer(&drawing, "Unused");
    let (result, codes) = apply(
        &mut file,
        vec![Edit::RemoveLayer {
            target: seen(unused),
        }],
    );
    assert_eq!(
        result.unwrap(),
        [Applied {
            id: None,
            hash: None
        }]
    );
    assert!(codes.is_empty(), "{codes:?}");
    let written = testkit::write::<Dxf>(&file).unwrap();
    let after = lift(&testkit::read::<Dxf>(&written).0.unwrap());
    assert!(after.layers.iter().all(|layer| layer.name != "Unused"));
    // ほかの画層の ID は、ハンドルで作るので変わらない
    assert_eq!(id::layer(layer(&after, "A")), "layer/h11");
    assert!(!String::from_utf8(written).unwrap().contains("Unused"));
}

#[test]
fn layers_in_use_are_not_removed() {
    let src = src();
    let mut file = testkit::read::<Dxf>(&src).0.unwrap();
    let drawing = lift(&file);
    let (zero, a, unused) = (
        layer(&drawing, "0"),
        layer(&drawing, "A"),
        layer(&drawing, "Unused"),
    );
    let element = &drawing.spaces[0].elements[0];
    let onto_unused = Restyle {
        layer: Some(unused.id),
        ..Restyle::default()
    };
    for edits in [
        // 画層 0、図形と今の画層が指す画層
        vec![Edit::RemoveLayer { target: seen(zero) }],
        vec![Edit::RemoveLayer { target: seen(a) }],
        // 同じ一覧で要素を移す画層と、2 度除く画層
        vec![
            Edit::Restyle {
                target: Seen {
                    id: "space/0/h1A".to_owned(),
                    hash: Hash::element(element),
                },
                style: onto_unused,
            },
            Edit::RemoveLayer {
                target: seen(unused),
            },
        ],
        vec![
            Edit::RemoveLayer {
                target: seen(unused),
            },
            Edit::RemoveLayer {
                target: seen(unused),
            },
        ],
    ] {
        let (result, codes) = apply(&mut file, edits.clone());
        assert_eq!(result.unwrap_err(), Error::Rejected, "{edits:?}");
        assert_eq!(codes, [codes::CHANGE_EDIT_UNSUPPORTED], "{edits:?}");
        assert_eq!(testkit::write::<Dxf>(&file).unwrap(), src);
    }
    // 今の画層（$CLAYER）だけが指す画層も除かない
    let current = String::from_utf8(src.clone())
        .unwrap()
        .replace("$CLAYER\n  8\nA\n", "$CLAYER\n  8\nUnused\n");
    let mut file = testkit::read::<Dxf>(current.as_bytes()).0.unwrap();
    let drawing = lift(&file);
    let (result, codes) = apply(
        &mut file,
        vec![Edit::RemoveLayer {
            target: seen(layer(&drawing, "Unused")),
        }],
    );
    assert_eq!(result.unwrap_err(), Error::Rejected);
    assert_eq!(codes, [codes::CHANGE_EDIT_UNSUPPORTED]);
}

#[test]
fn removing_a_layer_does_not_change_the_ids_of_later_layers_without_handles() {
    let original = String::from_utf8(src()).unwrap();
    let without_handle = original.replacen(
        "  0\nENDTAB\n  0\nENDSEC\n  0\nSECTION\n  2\nENTITIES\n",
        "  0\nLAYER\n  2\nLater\n 70\n0\n 62\n7\n  6\nContinuous\n  0\nENDTAB\n  0\nENDSEC\n  0\nSECTION\n  2\nENTITIES\n",
        1,
    );
    let implicit = original.replace("  8\na\n", "  8\nMissing\n");
    for (input, later) in [(without_handle, "Later"), (implicit, "Missing")] {
        let mut file = testkit::read::<Dxf>(input.as_bytes()).0.unwrap();
        let drawing = lift(&file);
        let before_id = id::layer(layer(&drawing, later));
        assert!(before_id.starts_with("layer/"));
        assert!(!before_id.starts_with("layer/h"));
        let (result, codes) = apply(
            &mut file,
            vec![Edit::RemoveLayer {
                target: seen(layer(&drawing, "Unused")),
            }],
        );
        assert_eq!(result.unwrap_err(), Error::Rejected, "{later}");
        assert_eq!(codes, [codes::CHANGE_EDIT_UNSUPPORTED]);
        assert_eq!(testkit::write::<Dxf>(&file).unwrap(), input.as_bytes());
        assert_eq!(id::layer(layer(&lift(&file), later)), before_id);
    }
}

#[test]
fn table_references_prevent_layer_removal() {
    let original = String::from_utf8(src()).unwrap();
    for reference in ["1001\nAPP\n1003\nUnused\n", "340\n12\n"] {
        let input = original.replacen(
            " 70\n1\n  0\nLTYPE\n",
            &format!(" 70\n1\n{reference}  0\nLTYPE\n"),
            1,
        );
        let mut file = testkit::read::<Dxf>(input.as_bytes()).0.unwrap();
        let drawing = lift(&file);
        let (result, codes) = apply(
            &mut file,
            vec![Edit::RemoveLayer {
                target: seen(layer(&drawing, "Unused")),
            }],
        );
        assert_eq!(result.unwrap_err(), Error::Rejected, "{reference}");
        assert_eq!(codes, [codes::CHANGE_EDIT_UNSUPPORTED]);
        assert_eq!(testkit::write::<Dxf>(&file).unwrap(), input.as_bytes());
    }
}

#[test]
fn layer_renames_count_record_values_and_stop_without_changing_the_file() {
    use zumen_base::{Kind, Limit, Limits};
    let original = String::from_utf8(src()).unwrap();
    let marker = "  2\nA\n 70\n0\n";
    let enlarged = original.replacen(
        marker,
        &format!("  2\nA\n{} 70\n0\n", "999\ncomment\n".repeat(300)),
        1,
    );
    let run = |input: &str, limit: Option<u64>| {
        let mut file = testkit::read::<Dxf>(input.as_bytes()).0.unwrap();
        let drawing = lift(&file);
        let was = layer(&drawing, "A");
        let mut new = was.clone();
        new.name = "Walls".to_owned();
        let changes = Changes {
            edits: vec![Edit::ReplaceLayer {
                target: seen(was),
                layer: Box::new(new),
            }],
            document: None,
        };
        let limits = limit.map_or(Limits::default(), |limit| {
            Limits::default().with(Kind::Work, Limit::Finite(limit))
        });
        let mut budget = Budget::new(limits);
        let mut diagnostics = Diagnostics::new();
        let result = file.apply(
            &changes,
            &mut Cx::new(&mut budget, &mut diagnostics, &CancelToken::new()),
        );
        (
            result,
            budget.used(Kind::Work),
            testkit::write::<Dxf>(&file).unwrap(),
        )
    };
    let (result, baseline, _) = run(&original, None);
    assert!(result.is_ok());
    let (result, larger, _) = run(&enlarged, None);
    assert!(result.is_ok());
    assert!(larger > baseline + 300, "{baseline}, {larger}");
    let (result, _, written) = run(&enlarged, Some(larger - 1));
    assert!(matches!(result, Err(Error::Stopped(_))));
    assert_eq!(written, enlarged.as_bytes());
}

#[test]
fn layer_rename_counts_values_examined_in_objects() {
    use zumen_base::Kind;
    let original = String::from_utf8(src()).unwrap();
    let payload = format!(
        "  0\nSECTION\n  2\nOBJECTS\n  0\nXRECORD\n  5\n35\n{}  0\nENDSEC\n  0\nEOF\n",
        "  1\nvalue\n".repeat(300)
    );
    let input = original.replace("  0\nEOF\n", &payload);
    let run = |rename: bool| {
        let mut file = testkit::read::<Dxf>(input.as_bytes()).0.unwrap();
        let drawing = lift(&file);
        let was = layer(&drawing, "A");
        let mut new = was.clone();
        if rename {
            new.name = "Walls".to_owned();
        }
        let changes = Changes {
            edits: vec![Edit::ReplaceLayer {
                target: seen(was),
                layer: Box::new(new),
            }],
            document: None,
        };
        let mut budget = Budget::default();
        let mut diagnostics = Diagnostics::new();
        file.apply(
            &changes,
            &mut Cx::new(&mut budget, &mut diagnostics, &CancelToken::new()),
        )
        .unwrap();
        budget.used(Kind::Work)
    };
    let unchanged = run(false);
    let renamed = run(true);
    assert!(renamed >= unchanged + 300, "{unchanged}, {renamed}");
}

/// 画層 Unused に、注釈の尺度の XRECORD を持つ拡張辞書を持たせた DXF（決定 0170）
#[cfg(test)]
fn with_dictionary(extra: &str) -> String {
    String::from_utf8(src())
        .unwrap()
        .replacen(
            "  5\n12\n330\n2\n",
            "  5\n12\n102\n{ACAD_XDICTIONARY\n360\nD1\n102\n}\n330\n2\n",
            1,
        )
        .replacen(
            "  0\nEOF\n",
            &format!(
                "  0\nSECTION\n  2\nOBJECTS\n  0\nDICTIONARY\n  5\nD1\n102\n{{ACAD_REACTORS\n330\n12\n102\n}}\n\
                 330\n12\n100\nAcDbDictionary\n281\n1\n  3\nASDK_XREC_ANNO_SCALE_INFO\n360\nD2\n  0\n\
                 XRECORD\n  5\nD2\n102\n{{ACAD_REACTORS\n330\nD1\n102\n}}\n330\nD1\n100\nAcDbXrecord\n\
                 280\n1\n 90\n4\n{extra}  0\nENDSEC\n  0\nEOF\n"
            ),
            1,
        )
}

#[test]
fn layers_are_removed_with_their_extension_dictionaries() {
    let input = with_dictionary("");
    let mut file = testkit::read::<Dxf>(input.as_bytes()).0.unwrap();
    let drawing = lift(&file);
    let (result, codes) = apply(
        &mut file,
        vec![Edit::RemoveLayer {
            target: seen(layer(&drawing, "Unused")),
        }],
    );
    assert!(result.is_ok(), "{codes:?}");
    assert!(codes.is_empty(), "{codes:?}");
    let written = String::from_utf8(testkit::write::<Dxf>(&file).unwrap()).unwrap();
    assert!(!written.contains("Unused"));
    assert!(!written.contains("ASDK_XREC_ANNO_SCALE_INFO"));
    assert!(!written.contains("XRECORD"));
    assert!(!written.contains("\nD1\n") && !written.contains("\nD2\n"));
    let after = lift(&testkit::read::<Dxf>(written.as_bytes()).0.unwrap());
    assert_eq!(id::layer(layer(&after, "A")), "layer/h11");
}

#[test]
fn extension_dictionaries_that_others_point_to_keep_their_layers() {
    let original = with_dictionary("");
    let inputs = [
        // ほかのオブジェクトが、拡張辞書の XRECORD を指す
        with_dictionary("  0\nXRECORD\n  5\nD3\n330\n1F\n100\nAcDbXrecord\n340\nD2\n"),
        // 見出しの変数が、拡張辞書を指す
        original.replacen("$CLAYER\n", "$TSTACKALIGN\n340\nD1\n  9\n$CLAYER\n", 1),
        // 拡張辞書の所有者が、画層でない
        original.replacen(
            "330\n12\n100\nAcDbDictionary\n",
            "330\n11\n100\nAcDbDictionary\n",
            1,
        ),
        // 拡張辞書が持つ XRECORD の所有者が、拡張辞書でない
        original.replacen(
            "330\nD1\n100\nAcDbXrecord\n",
            "330\n12\n100\nAcDbXrecord\n",
            1,
        ),
        // 拡張辞書がない
        original.replacen("360\nD1\n102\n}\n", "360\nD9\n102\n}\n", 1),
        // 拡張辞書の参照がない、組が閉じない、参照先が辞書でない
        original.replacen("360\nD1\n102\n}\n", "102\n}\n", 1),
        original.replacen("360\nD1\n102\n}\n", "360\nD1\n", 1),
        original.replacen("  0\nDICTIONARY\n  5\nD1\n", "  0\nXRECORD\n  5\nD1\n", 1),
        // 画層が反応子を持つ
        original.replacen(
            "  5\n12\n102\n",
            "  5\n12\n102\n{ACAD_REACTORS\n330\n1A\n102\n}\n102\n",
            1,
        ),
    ];
    for input in inputs {
        let mut file = testkit::read::<Dxf>(input.as_bytes()).0.unwrap();
        let drawing = lift(&file);
        let (result, codes) = apply(
            &mut file,
            vec![Edit::RemoveLayer {
                target: seen(layer(&drawing, "Unused")),
            }],
        );
        assert_eq!(result.unwrap_err(), Error::Rejected, "{input}");
        assert_eq!(codes, [codes::CHANGE_EDIT_UNSUPPORTED], "{input}");
        assert_eq!(testkit::write::<Dxf>(&file).unwrap(), input.as_bytes());
    }
}

#[test]
fn removing_dictionary_children_counts_each_removal_and_stops_safely() {
    use zumen_base::{Kind, Limit, Limits};

    let mut entries = String::new();
    let mut objects = String::new();
    for n in 0..40 {
        let handle = 0x100 + n;
        entries.push_str(&format!("  3\nCHILD{n}\n360\n{handle:X}\n"));
        objects.push_str(&format!(
            "  0\nXRECORD\n  5\n{handle:X}\n330\nD1\n100\nAcDbXrecord\n 90\n{n}\n"
        ));
    }
    let input = with_dictionary("")
        .replacen(
            "  3\nASDK_XREC_ANNO_SCALE_INFO",
            &format!("{entries}  3\nASDK_XREC_ANNO_SCALE_INFO"),
            1,
        )
        .replacen(
            "  0\nENDSEC\n  0\nEOF\n",
            &format!("{objects}  0\nENDSEC\n  0\nEOF\n"),
            1,
        );
    let run = |limit: Option<u64>| {
        let mut file = testkit::read::<Dxf>(input.as_bytes()).0.unwrap();
        let drawing = lift(&file);
        let changes = Changes {
            edits: vec![Edit::RemoveLayer {
                target: seen(layer(&drawing, "Unused")),
            }],
            document: None,
        };
        let limits = limit.map_or(Limits::default(), |limit| {
            Limits::default().with(Kind::Work, Limit::Finite(limit))
        });
        let mut budget = Budget::new(limits);
        let mut diagnostics = Diagnostics::new();
        let result = file.apply(
            &changes,
            &mut Cx::new(&mut budget, &mut diagnostics, &CancelToken::new()),
        );
        (
            result,
            budget.used(Kind::Work),
            testkit::write::<Dxf>(&file).unwrap(),
        )
    };
    let (result, used, _) = run(None);
    assert!(result.is_ok(), "{result:?}");
    // 40 個の子はそれぞれ残りの全レコードを走査する。
    assert!(used > 40 * 200, "{used}");
    let (result, _, written) = run(Some(used / 2));
    assert!(matches!(result, Err(Error::Stopped(_))));
    assert_eq!(written, input.as_bytes());
}
