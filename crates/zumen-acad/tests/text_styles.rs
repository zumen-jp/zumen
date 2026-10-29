//! 文字スタイルの置き換え・追加・除去を、DXF の文字スタイルの表に書き戻す（決定 0163）。
#![cfg(feature = "drawing")]

use zumen_acad::{Dxf, DxfFile};
use zumen_base::{Budget, CancelToken, Code, Cx, Diagnostics, codes};
use zumen_drawing::{Applied, Changes, Drawing2D, Edit, Font, FontKind, Hash, Seen, TextStyle, id};
use zumen_sdk::{Apply, Error, Lift, testkit};

/// 文字スタイル Standard・Notes・Unused・Gothic（XDATA の書体の名前で指すフォント）を持ち、文字が Notes を
/// 指す R2000 の DXF。今の文字スタイルは Notes
#[cfg(test)]
fn src() -> Vec<u8> {
    let style = |handle: &'static str, name: &'static str, font: &'static str| {
        vec![
            (0, "STYLE"),
            (5, handle),
            (330, "3"),
            (100, "AcDbSymbolTableRecord"),
            (100, "AcDbTextStyleTableRecord"),
            (2, name),
            (70, "0"),
            (40, "0.0"),
            (41, "1.0"),
            (50, "0.0"),
            (71, "0"),
            (42, "2.5"),
            (3, font),
            (4, ""),
        ]
    };
    let mut tags: Vec<(i16, &str)> = vec![
        (0, "SECTION"),
        (2, "HEADER"),
        (9, "$ACADVER"),
        (1, "AC1015"),
        (9, "$HANDSEED"),
        (5, "40"),
        (9, "$TEXTSTYLE"),
        (7, "Notes"),
        (0, "ENDSEC"),
        (0, "SECTION"),
        (2, "TABLES"),
        (0, "TABLE"),
        (2, "LAYER"),
        (5, "2"),
        (70, "1"),
        (0, "LAYER"),
        (5, "10"),
        (330, "2"),
        (100, "AcDbSymbolTableRecord"),
        (100, "AcDbLayerTableRecord"),
        (2, "0"),
        (70, "0"),
        (62, "7"),
        (6, "Continuous"),
        (0, "ENDTAB"),
        (0, "TABLE"),
        (2, "STYLE"),
        (5, "3"),
        (70, "4"),
    ];
    tags.extend(style("11", "Standard", "txt"));
    tags.extend(style("12", "Notes", "romans.shx"));
    tags.extend(style("13", "Unused", "txt"));
    tags.extend(style("14", "Gothic", ""));
    tags.extend([
        (1001, "ACAD"),
        (1000, "MS Gothic"),
        (1071, "32802"),
        (0, "ENDTAB"),
        (0, "ENDSEC"),
        (0, "SECTION"),
        (2, "ENTITIES"),
        (0, "TEXT"),
        (5, "1A"),
        (100, "AcDbEntity"),
        (8, "0"),
        (100, "AcDbText"),
        (10, "0.0"),
        (20, "0.0"),
        (30, "0.0"),
        (40, "2.5"),
        (1, "NOTE"),
        (7, "Notes"),
        (100, "AcDbText"),
        (0, "ENDSEC"),
        (0, "EOF"),
    ]);
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
fn style<'a>(drawing: &'a Drawing2D, name: &str) -> (String, &'a TextStyle) {
    let (index, style) = drawing
        .text_styles
        .iter()
        .enumerate()
        .find(|(_, style)| style.name == name)
        .unwrap();
    (id::text_style(style, index), style)
}

#[cfg(test)]
fn seen(drawing: &Drawing2D, name: &str) -> Seen {
    let (id, style) = style(drawing, name);
    Seen {
        id,
        hash: Hash::text_style(style),
    }
}

#[cfg(test)]
fn shape(name: &str) -> Option<Font> {
    Some(Font {
        name: name.to_owned(),
        kind: FontKind::Shape,
    })
}

#[test]
fn text_styles_are_renamed_and_redefined() {
    let mut file = testkit::read::<Dxf>(&src()).0.unwrap();
    let drawing = lift(&file);
    let (_, notes) = style(&drawing, "Notes");
    let mut changed = notes.clone();
    changed.name = "Labels".to_owned();
    let definition = changed.definition.as_mut().unwrap();
    definition.font = Some(Font {
        name: "msgothic.ttc".to_owned(),
        kind: FontKind::Named {
            bold: false,
            italic: false,
            charset: None,
            pitch: None,
        },
    });
    definition.big_font = shape("extfont.shx");
    definition.height = Some(3.5);
    definition.width = 0.8;
    definition.oblique = 15.0;
    definition.backward = true;
    definition.vertical = true;
    let (result, found) = apply(
        &mut file,
        vec![Edit::ReplaceTextStyle {
            target: seen(&drawing, "Notes"),
            style: Box::new(changed.clone()),
        }],
    );
    let applied = result.unwrap();
    assert!(found.is_empty(), "{found:?}");
    let written = testkit::write::<Dxf>(&file).unwrap();
    let after = lift(&testkit::read::<Dxf>(&written).0.unwrap());
    let (id, labels) = style(&after, "Labels");
    assert_eq!(id, "text-style/h12");
    assert_eq!(labels.definition, changed.definition);
    assert_eq!(applied[0].id.as_deref(), Some("text-style/h12"));
    assert_eq!(applied[0].hash, Some(Hash::text_style(labels)));
    // 文字と今の文字スタイルも、新しい名前で指す
    let text = String::from_utf8(written).unwrap();
    assert!(text.contains("$TEXTSTYLE\n  7\nLabels\n"), "{text}");
    assert!(text.contains("NOTE\n  7\nLabels\n"), "{text}");
    assert!(!text.contains("Notes"), "{text}");
}

#[test]
fn text_styles_are_added_and_removed() {
    let mut file = testkit::read::<Dxf>(&src()).0.unwrap();
    let drawing = lift(&file);
    let (_, standard) = style(&drawing, "Standard");
    let mut added = standard.clone();
    added.name = "注記".to_owned();
    added.definition.as_mut().unwrap().font = shape("simplex.shx");
    // R2000 の DXF には、ASCII でない名前を書けない
    let (result, found) = apply(
        &mut file,
        vec![Edit::AddTextStyle {
            style: Box::new(added.clone()),
        }],
    );
    assert_eq!(result, Err(Error::Rejected));
    assert_eq!(found, [codes::CHANGE_EDIT_UNSUPPORTED]);
    added.name = "Added".to_owned();
    let (result, found) = apply(
        &mut file,
        vec![
            Edit::AddTextStyle {
                style: Box::new(added.clone()),
            },
            Edit::RemoveTextStyle {
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
    let (id, new) = style(&after, "Added");
    assert_eq!(id, "text-style/h40");
    assert_eq!(new.definition, added.definition);
    assert_eq!(
        applied,
        [
            Applied {
                id: Some(id),
                hash: Some(Hash::text_style(new)),
            },
            Applied {
                id: None,
                hash: None,
            },
        ]
    );
    assert!(after.text_styles.iter().all(|style| style.name != "Unused"));
}

#[test]
fn text_style_changes_that_cannot_be_written_are_refused() {
    let src = src();
    let drawing = lift(&testkit::read::<Dxf>(&src).0.unwrap());
    let gothic = style(&drawing, "Gothic").1.definition.as_ref().unwrap();
    assert_eq!(gothic.font.as_ref().unwrap().name, "MS Gothic");
    let replace = |name: &str, change: &dyn Fn(&mut TextStyle)| {
        let (_, was) = style(&drawing, name);
        let mut changed = was.clone();
        change(&mut changed);
        Edit::ReplaceTextStyle {
            target: seen(&drawing, name),
            style: Box::new(changed),
        }
    };
    let remove = |name: &str| Edit::RemoveTextStyle {
        target: seen(&drawing, name),
    };
    for edits in [
        // Standard の名前と、ほかと重なる名前
        vec![replace("Standard", &|style| style.name = "Main".to_owned())],
        vec![replace("Unused", &|style| style.name = "notes".to_owned())],
        // XDATA の書体の名前で指すフォントと、属性を持つ名前で指すフォント
        vec![replace("Gothic", &|style| {
            style.definition.as_mut().unwrap().font = shape("txt");
        })],
        vec![replace("Unused", &|style| {
            style.definition.as_mut().unwrap().font = Some(Font {
                name: "arial.ttf".to_owned(),
                kind: FontKind::Named {
                    bold: true,
                    italic: false,
                    charset: None,
                    pitch: None,
                },
            });
        })],
        // 大きいフォントは形のファイルだけ。幅と傾きの範囲
        vec![replace("Unused", &|style| {
            style.definition.as_mut().unwrap().big_font = Some(Font {
                name: "MS Gothic".to_owned(),
                kind: FontKind::Named {
                    bold: false,
                    italic: false,
                    charset: None,
                    pitch: None,
                },
            });
        })],
        vec![replace("Unused", &|style| {
            style.definition.as_mut().unwrap().width = 0.0;
        })],
        vec![replace("Unused", &|style| {
            style.definition.as_mut().unwrap().oblique = 90.0;
        })],
        // 同じ文字スタイルの 2 度の変更
        vec![
            replace("Unused", &|style| style.name = "Spare".to_owned()),
            remove("Unused"),
        ],
        // Standard と、文字と今の文字スタイルが指すもの
        vec![remove("Standard")],
        vec![remove("Notes")],
    ] {
        let mut file = testkit::read::<Dxf>(&src).0.unwrap();
        let (result, found) = apply(&mut file, edits.clone());
        assert_eq!(result, Err(Error::Rejected), "{edits:?}");
        assert_eq!(found, [codes::CHANGE_EDIT_UNSUPPORTED], "{edits:?}");
        assert_eq!(testkit::write::<Dxf>(&file).unwrap(), src, "{edits:?}");
    }
    // 古いハッシュと、ない ID
    let mut stale = seen(&drawing, "Unused");
    stale.hash = Hash::text_style(style(&drawing, "Notes").1);
    let mut missing = seen(&drawing, "Unused");
    missing.id = "text-style/h99".to_owned();
    for (target, code) in [
        (stale, codes::CHANGE_ELEMENT_STALE),
        (missing, codes::CHANGE_ELEMENT_MISSING),
    ] {
        let mut file = testkit::read::<Dxf>(&src).0.unwrap();
        let (result, found) = apply(&mut file, vec![Edit::RemoveTextStyle { target }]);
        assert_eq!(result, Err(Error::Rejected));
        assert_eq!(found, [code]);
    }
    // 文字が表にない文字スタイルを指すと、それは後ろに並びの番号の ID で並ぶ。除くと、その ID が変わる
    let src = String::from_utf8(src)
        .unwrap()
        .replace("NOTE\n  7\nNotes\n", "NOTE\n  7\nMissing\n")
        .into_bytes();
    let drawing = lift(&testkit::read::<Dxf>(&src).0.unwrap());
    assert_eq!(style(&drawing, "Missing").0, "text-style/4");
    let mut file = testkit::read::<Dxf>(&src).0.unwrap();
    let (result, found) = apply(&mut file, vec![remove("Unused")]);
    assert_eq!(result, Err(Error::Rejected));
    assert_eq!(found, [codes::CHANGE_EDIT_UNSUPPORTED]);
}

#[test]
fn text_style_renames_count_record_values_and_stop_without_changing_the_file() {
    use zumen_base::{Kind, Limit, Limits};
    let original = String::from_utf8(src()).unwrap();
    let marker = "  2\nNotes\n 70\n0\n";
    let enlarged = original.replacen(
        marker,
        &format!("  2\nNotes\n{} 70\n0\n", "999\ncomment\n".repeat(300)),
        1,
    );
    let run = |input: &str, limit: Option<u64>| {
        let mut file = testkit::read::<Dxf>(input.as_bytes()).0.unwrap();
        let drawing = lift(&file);
        let mut new = style(&drawing, "Notes").1.clone();
        new.name = "Labels".to_owned();
        let changes = Changes {
            edits: vec![Edit::ReplaceTextStyle {
                target: seen(&drawing, "Notes"),
                style: Box::new(new),
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
