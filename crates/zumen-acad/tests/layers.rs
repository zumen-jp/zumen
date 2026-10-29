//! レイヤの置き換えと追加を、DXF の画層の表に書き戻す（決定 0162）。
#![cfg(feature = "drawing")]

use zumen_acad::{Dxf, DxfFile};
use zumen_base::{Budget, CancelToken, Code, Cx, Diagnostics, codes};
use zumen_drawing::{
    Applied, Changes, Color, ColorCode, ColorSystem, Drawing2D, Edit, Hash, Layer, LayerState,
    LineWeight, PaperLength, Seen, id,
};
use zumen_sdk::{Apply, Error, Lift, testkit};

/// 画層 0 と A を持ち、線が A に、円が小文字の a にある R2000 の DXF。今の画層は A
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
