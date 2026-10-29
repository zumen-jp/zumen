//! 線種の置き換え・追加・除去を、DXF の線種の表に書き戻す（決定 0164）。
#![cfg(feature = "drawing")]

use zumen_acad::{Dxf, DxfFile};
use zumen_base::{Budget, CancelToken, Code, Cx, Diagnostics, codes};
use zumen_drawing::{
    Applied, Changes, Drawing2D, Edit, Hash, LayerStyle, LineType, PatternBasis, Seen, id,
};
use zumen_sdk::{Apply, Error, Lift, testkit};

/// 線種 ByBlock・ByLayer・Continuous・Dashed・Unused・Marked（文字を添える）を持ち、画層 Wall と線分が
/// Dashed を指す R2000 の DXF。今の線種は Dashed
#[cfg(test)]
fn src() -> Vec<u8> {
    let line_type = |handle: &'static str, name: &'static str, dashes: &[(i16, &'static str)]| {
        let count = dashes.iter().filter(|(code, _)| *code == 49).count();
        let total: f64 = dashes
            .iter()
            .filter(|(code, _)| *code == 49)
            .map(|(_, length)| length.parse::<f64>().unwrap().abs())
            .sum();
        let mut tags = vec![
            (0, "LTYPE".to_owned()),
            (5, handle.to_owned()),
            (330, "5".to_owned()),
            (100, "AcDbSymbolTableRecord".to_owned()),
            (100, "AcDbLinetypeTableRecord".to_owned()),
            (2, name.to_owned()),
            (70, "0".to_owned()),
            (3, format!("{name} line")),
            (72, "65".to_owned()),
            (73, count.to_string()),
            (40, total.to_string()),
        ];
        tags.extend(
            dashes
                .iter()
                .map(|(code, value)| (*code, (*value).to_owned())),
        );
        tags
    };
    let mut tags: Vec<(i16, String)> = [
        (0, "SECTION"),
        (2, "HEADER"),
        (9, "$ACADVER"),
        (1, "AC1015"),
        (9, "$HANDSEED"),
        (5, "40"),
        (9, "$CELTYPE"),
        (6, "Dashed"),
        (0, "ENDSEC"),
        (0, "SECTION"),
        (2, "TABLES"),
        (0, "TABLE"),
        (2, "LTYPE"),
        (5, "5"),
        (70, "6"),
    ]
    .iter()
    .map(|(code, value)| (*code, (*value).to_owned()))
    .collect();
    tags.extend(line_type("14", "ByBlock", &[]));
    tags.extend(line_type("15", "ByLayer", &[]));
    tags.extend(line_type("16", "Continuous", &[]));
    tags.extend(line_type(
        "17",
        "Dashed",
        &[(49, "0.5"), (74, "0"), (49, "-0.25"), (74, "0")],
    ));
    tags.extend(line_type(
        "18",
        "Unused",
        &[(49, "1.0"), (74, "0"), (49, "-1.0"), (74, "0")],
    ));
    tags.extend(line_type(
        "19",
        "Marked",
        &[
            (49, "0.5"),
            (74, "0"),
            (49, "-0.2"),
            (74, "2"),
            (75, "0"),
            (340, "11"),
            (46, "0.1"),
            (50, "0.0"),
            (44, "-0.1"),
            (45, "-0.05"),
            (9, "GAS"),
            (49, "-0.25"),
            (74, "0"),
        ],
    ));
    tags.extend(
        [
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
            (0, "LAYER"),
            (5, "12"),
            (330, "2"),
            (100, "AcDbSymbolTableRecord"),
            (100, "AcDbLayerTableRecord"),
            (2, "Wall"),
            (70, "0"),
            (62, "1"),
            (6, "Dashed"),
            (0, "ENDTAB"),
            (0, "TABLE"),
            (2, "STYLE"),
            (5, "3"),
            (70, "1"),
            (0, "STYLE"),
            (5, "11"),
            (330, "3"),
            (100, "AcDbSymbolTableRecord"),
            (100, "AcDbTextStyleTableRecord"),
            (2, "Standard"),
            (70, "0"),
            (40, "0.0"),
            (41, "1.0"),
            (50, "0.0"),
            (71, "0"),
            (42, "2.5"),
            (3, "txt"),
            (4, ""),
            (0, "ENDTAB"),
            (0, "ENDSEC"),
            (0, "SECTION"),
            (2, "ENTITIES"),
            (0, "LINE"),
            (5, "1A"),
            (100, "AcDbEntity"),
            (8, "0"),
            (6, "Dashed"),
            (100, "AcDbLine"),
            (10, "0.0"),
            (20, "0.0"),
            (30, "0.0"),
            (11, "10.0"),
            (21, "0.0"),
            (31, "0.0"),
            (0, "ENDSEC"),
            (0, "EOF"),
        ]
        .iter()
        .map(|(code, value)| (*code, (*value).to_owned())),
    );
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
fn line_type<'a>(drawing: &'a Drawing2D, name: &str) -> &'a LineType {
    drawing
        .line_types
        .iter()
        .find(|line_type| line_type.name == name)
        .unwrap()
}

#[cfg(test)]
fn seen(drawing: &Drawing2D, name: &str) -> Seen {
    let line_type = line_type(drawing, name);
    Seen {
        id: id::line_type(line_type),
        hash: Hash::line_type(line_type),
    }
}

#[test]
fn line_types_are_renamed_and_redefined() {
    let mut file = testkit::read::<Dxf>(&src()).0.unwrap();
    let drawing = lift(&file);
    let mut changed = line_type(&drawing, "Dashed").clone();
    changed.name = "Hidden".to_owned();
    let definition = changed.definition.as_mut().unwrap();
    definition.description = "Hidden __ __".to_owned();
    definition.pattern = vec![1.0, -0.5, 0.0, -0.5];
    // 文字を添える線種は、要素の数を変えずに長さだけを変えられる
    let mut marked = line_type(&drawing, "Marked").clone();
    marked.definition.as_mut().unwrap().pattern = vec![1.0, -0.4, -0.5];
    let (result, found) = apply(
        &mut file,
        vec![
            Edit::ReplaceLineType {
                target: seen(&drawing, "Dashed"),
                line_type: Box::new(changed.clone()),
            },
            Edit::ReplaceLineType {
                target: seen(&drawing, "Marked"),
                line_type: Box::new(marked.clone()),
            },
        ],
    );
    let applied = result.unwrap();
    assert!(found.is_empty(), "{found:?}");
    let written = testkit::write::<Dxf>(&file).unwrap();
    let after = lift(&testkit::read::<Dxf>(&written).0.unwrap());
    let hidden = line_type(&after, "Hidden");
    assert_eq!(id::line_type(hidden), "line-type/h17");
    assert_eq!(hidden.definition, changed.definition);
    assert_eq!(line_type(&after, "Marked").definition, marked.definition);
    assert_eq!(applied[0].id.as_deref(), Some("line-type/h17"));
    assert_eq!(applied[0].hash, Some(Hash::line_type(hidden)));
    // 線分・画層・今の線種も、新しい名前で指す
    let text = String::from_utf8(written).unwrap();
    assert!(text.contains("$CELTYPE\n  6\nHidden\n"), "{text}");
    assert!(
        text.contains("Wall\n 70\n0\n 62\n1\n  6\nHidden\n"),
        "{text}"
    );
    assert!(text.contains("  8\n0\n  6\nHidden\n"), "{text}");
    assert!(!text.contains("Dashed"), "{text}");
}

#[test]
fn line_types_are_added_and_removed() {
    let mut file = testkit::read::<Dxf>(&src()).0.unwrap();
    let drawing = lift(&file);
    let mut added = line_type(&drawing, "Dashed").clone();
    added.name = "Center".to_owned();
    added.definition.as_mut().unwrap().pattern = vec![1.25, -0.25, 0.25, -0.25];
    let (result, found) = apply(
        &mut file,
        vec![
            Edit::AddLineType {
                line_type: Box::new(added.clone()),
            },
            Edit::RemoveLineType {
                target: seen(&drawing, "Unused"),
            },
        ],
    );
    let applied = result.unwrap();
    assert!(found.is_empty(), "{found:?}");
    let after = lift(
        &testkit::read::<Dxf>(&testkit::write::<Dxf>(&file).unwrap())
            .0
            .unwrap(),
    );
    let center = line_type(&after, "Center");
    assert_eq!(id::line_type(center), "line-type/h40");
    assert_eq!(center.definition, added.definition);
    assert_eq!(
        applied,
        [
            Applied {
                id: Some("line-type/h40".to_owned()),
                hash: Some(Hash::line_type(center)),
            },
            Applied {
                id: None,
                hash: None,
            },
        ]
    );
    assert!(after.line_types.iter().all(|other| other.name != "Unused"));
}

#[test]
fn line_type_changes_do_not_merge_undefined_types_or_shift_their_ids() {
    let input = String::from_utf8(src())
        .unwrap()
        .replace("  6\nDashed\n100\nAcDbLine", "  6\nMissing\n100\nAcDbLine");
    let drawing = lift(&testkit::read::<Dxf>(input.as_bytes()).0.unwrap());
    let missing = line_type(&drawing, "Missing");
    assert!(missing.definition.is_none());
    let mut renamed = line_type(&drawing, "Unused").clone();
    renamed.name = "Missing".to_owned();
    let mut added = line_type(&drawing, "Dashed").clone();
    added.name = "Added".to_owned();
    for edit in [
        Edit::ReplaceLineType {
            target: seen(&drawing, "Unused"),
            line_type: Box::new(renamed),
        },
        Edit::AddLineType {
            line_type: Box::new(added),
        },
    ] {
        let mut file = testkit::read::<Dxf>(input.as_bytes()).0.unwrap();
        let (result, found) = apply(&mut file, vec![edit]);
        assert_eq!(result, Err(Error::Rejected));
        assert_eq!(found, [codes::CHANGE_EDIT_UNSUPPORTED]);
        assert_eq!(testkit::write::<Dxf>(&file).unwrap(), input.as_bytes());
    }
}

#[test]
fn line_type_changes_that_cannot_be_written_are_refused() {
    let src = src();
    let drawing = lift(&testkit::read::<Dxf>(&src).0.unwrap());
    let replace = |name: &str, change: &dyn Fn(&mut LineType)| {
        let mut changed = line_type(&drawing, name).clone();
        change(&mut changed);
        Edit::ReplaceLineType {
            target: seen(&drawing, name),
            line_type: Box::new(changed),
        }
    };
    let remove = |name: &str| Edit::RemoveLineType {
        target: seen(&drawing, name),
    };
    let unused = line_type(&drawing, "Unused").id;
    let mut wall = drawing
        .layers
        .iter()
        .find(|layer| layer.name == "Wall")
        .unwrap()
        .clone();
    wall.name = "Grid".to_owned();
    wall.style = Some(LayerStyle {
        line_type: Some(unused),
        ..wall.style.unwrap()
    });
    for edits in [
        // Continuous の名前と、ほかと重なる名前
        vec![replace("Continuous", &|line_type| {
            line_type.name = "Solid".to_owned();
        })],
        vec![replace("Unused", &|line_type| {
            line_type.name = "dashed".to_owned();
        })],
        // 用紙の mm の列、有限でない長さ、添える文字の変更、文字を添える線種の要素の数の変更
        vec![replace("Unused", &|line_type| {
            line_type.definition.as_mut().unwrap().basis = PatternBasis::Paper;
        })],
        vec![replace("Unused", &|line_type| {
            line_type.definition.as_mut().unwrap().pattern = vec![f64::NAN];
        })],
        vec![replace("Marked", &|line_type| {
            line_type.definition.as_mut().unwrap().marks.clear();
        })],
        vec![replace("Marked", &|line_type| {
            line_type.definition.as_mut().unwrap().pattern = vec![0.5, -0.25];
        })],
        // 同じ線種の 2 度の変更
        vec![
            replace("Unused", &|line_type| line_type.name = "Spare".to_owned()),
            remove("Unused"),
        ],
        // Continuous と、線分・画層・今の線種が指すものと、同じ一覧で足すレイヤが指すもの
        vec![remove("Continuous")],
        vec![remove("Dashed")],
        vec![
            Edit::AddLayer {
                layer: Box::new(wall),
            },
            remove("Unused"),
        ],
    ] {
        let mut file = testkit::read::<Dxf>(&src).0.unwrap();
        let (result, found) = apply(&mut file, edits.clone());
        assert_eq!(result, Err(Error::Rejected), "{edits:?}");
        assert_eq!(found, [codes::CHANGE_EDIT_UNSUPPORTED], "{edits:?}");
        assert_eq!(testkit::write::<Dxf>(&file).unwrap(), src, "{edits:?}");
    }
    // 古いハッシュと、ない ID
    let mut stale = seen(&drawing, "Unused");
    stale.hash = Hash::line_type(line_type(&drawing, "Dashed"));
    let mut missing = seen(&drawing, "Unused");
    missing.id = "line-type/h99".to_owned();
    for (target, code) in [
        (stale, codes::CHANGE_ELEMENT_STALE),
        (missing, codes::CHANGE_ELEMENT_MISSING),
    ] {
        let mut file = testkit::read::<Dxf>(&src).0.unwrap();
        let (result, found) = apply(&mut file, vec![Edit::RemoveLineType { target }]);
        assert_eq!(result, Err(Error::Rejected));
        assert_eq!(found, [code]);
    }
    // 線分が表にない線種を指すと、それは後ろに並びの番号の ID で並ぶ。除くと、その ID が変わる
    let src = String::from_utf8(src)
        .unwrap()
        .replace("  8\n0\n  6\nDashed\n", "  8\n0\n  6\nMissing\n")
        .into_bytes();
    let drawing = lift(&testkit::read::<Dxf>(&src).0.unwrap());
    assert!(!id::line_type(line_type(&drawing, "Missing")).starts_with("line-type/h"));
    let mut file = testkit::read::<Dxf>(&src).0.unwrap();
    let (result, found) = apply(&mut file, vec![remove("Unused")]);
    assert_eq!(result, Err(Error::Rejected));
    assert_eq!(found, [codes::CHANGE_EDIT_UNSUPPORTED]);
}

#[test]
fn line_type_renames_count_record_values_and_stop_without_changing_the_file() {
    use zumen_base::{Kind, Limit, Limits};
    let original = String::from_utf8(src()).unwrap();
    let marker = "  2\nUnused\n 70\n0\n";
    let enlarged = original.replacen(
        marker,
        &format!("  2\nUnused\n{} 70\n0\n", "999\ncomment\n".repeat(300)),
        1,
    );
    let run = |input: &str, limit: Option<u64>| {
        let mut file = testkit::read::<Dxf>(input.as_bytes()).0.unwrap();
        let drawing = lift(&file);
        let mut new = line_type(&drawing, "Dashed").clone();
        new.name = "Hidden".to_owned();
        let changes = Changes {
            edits: vec![Edit::ReplaceLineType {
                target: seen(&drawing, "Dashed"),
                line_type: Box::new(new),
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
