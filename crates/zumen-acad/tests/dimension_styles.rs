//! 寸法スタイルの置き換え・追加・除去を、DXF の寸法スタイルの表に書き戻す（決定 0165）。
#![cfg(feature = "drawing")]

use zumen_acad::{Dxf, DxfFile};
use zumen_base::{Budget, CancelToken, Code, Cx, Diagnostics, codes};
use zumen_drawing::{
    Applied, Arrowhead, Changes, DimensionStyle, Drawing2D, Edit, Hash, Seen, SymbolKey, id,
};
use zumen_sdk::{Apply, Error, Lift, testkit};

/// 寸法スタイル Standard・Arch（斜線の端部記号）・Unused・Family と子の Family$0 を持ち、公差が Arch を指す
/// R2000 の DXF。今の寸法スタイルは Standard
#[cfg(test)]
fn src() -> Vec<u8> {
    let style = |handle: &'static str, name: &'static str, extra: &[(i16, &'static str)]| {
        let mut tags = vec![
            (0, "DIMSTYLE".to_owned()),
            (105, handle.to_owned()),
            (330, "A".to_owned()),
            (100, "AcDbSymbolTableRecord".to_owned()),
            (100, "AcDbDimStyleTableRecord".to_owned()),
            (2, name.to_owned()),
            (70, "0".to_owned()),
            (40, "1.0".to_owned()),
            (41, "2.5".to_owned()),
        ];
        tags.extend(
            extra
                .iter()
                .map(|(code, value)| (*code, (*value).to_owned())),
        );
        tags
    };
    let block = |handle: &'static str, name: &'static str| {
        [
            (0, "BLOCK_RECORD"),
            (5, handle),
            (330, "1"),
            (100, "AcDbSymbolTableRecord"),
            (100, "AcDbBlockTableRecord"),
            (2, name),
        ]
        .iter()
        .map(|(code, value)| (*code, (*value).to_owned()))
        .collect::<Vec<_>>()
    };
    let mut tags: Vec<(i16, String)> = [
        (0, "SECTION"),
        (2, "HEADER"),
        (9, "$ACADVER"),
        (1, "AC1015"),
        (9, "$HANDSEED"),
        (5, "40"),
        (9, "$DIMSTYLE"),
        (2, "Standard"),
        (0, "ENDSEC"),
        (0, "SECTION"),
        (2, "TABLES"),
        (0, "TABLE"),
        (2, "BLOCK_RECORD"),
        (5, "1"),
        (70, "3"),
    ]
    .iter()
    .map(|(code, value)| (*code, (*value).to_owned()))
    .collect();
    tags.extend(block("1F", "*Model_Space"));
    tags.extend(block("20", "_DOT"));
    tags.extend(block("21", "_OBLIQUE"));
    tags.extend(
        [
            (0, "ENDTAB"),
            (0, "TABLE"),
            (2, "DIMSTYLE"),
            (5, "A"),
            (70, "5"),
            (100, "AcDbSymbolTable"),
            (100, "AcDbDimStyleTable"),
        ]
        .iter()
        .map(|(code, value)| (*code, (*value).to_owned())),
    );
    tags.extend(style("2B", "Standard", &[]));
    tags.extend(style("2C", "Arch", &[(142, "0.0"), (342, "21")]));
    tags.extend(style("2D", "Unused", &[]));
    tags.extend(style("2E", "Family", &[]));
    tags.extend(style("2F", "Family$0", &[]));
    tags.extend(
        [
            (0, "ENDTAB"),
            (0, "ENDSEC"),
            (0, "SECTION"),
            (2, "ENTITIES"),
            (0, "TOLERANCE"),
            (5, "1A"),
            (100, "AcDbEntity"),
            (8, "0"),
            (100, "AcDbFcf"),
            (3, "Arch"),
            (10, "0.0"),
            (20, "0.0"),
            (30, "0.0"),
            (1, "{\\Fgdt;j}%%v0.1"),
            (11, "1.0"),
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
fn style<'a>(drawing: &'a Drawing2D, name: &str) -> (String, &'a DimensionStyle) {
    let (index, style) = drawing
        .dimension_styles
        .iter()
        .enumerate()
        .find(|(_, style)| style.name == name)
        .unwrap();
    (id::dimension_style(style, index), style)
}

#[cfg(test)]
fn seen(drawing: &Drawing2D, name: &str) -> Seen {
    let (id, style) = style(drawing, name);
    Seen {
        id,
        hash: Hash::dimension_style(style),
    }
}

#[cfg(test)]
fn block(name: &str) -> Arrowhead {
    Arrowhead::Symbol(SymbolKey {
        namespace: "dxf.block",
        name: name.to_owned(),
    })
}

#[test]
fn dimension_styles_are_renamed_and_redefined() {
    let mut file = testkit::read::<Dxf>(&src()).0.unwrap();
    let drawing = lift(&file);
    assert_eq!(
        style(&drawing, "Arch").1.ends,
        [block("_OBLIQUE"), block("_OBLIQUE")]
    );
    let mut changed = style(&drawing, "Arch").1.clone();
    changed.name = "Plan".to_owned();
    changed.size = 1.5;
    changed.scale = 100.0;
    changed.tick = Some(1.0);
    changed.ends = [block("_DOT"), Arrowhead::Default];
    changed.leader = block("_DOT");
    let (result, found) = apply(
        &mut file,
        vec![Edit::ReplaceDimensionStyle {
            target: seen(&drawing, "Arch"),
            style: Box::new(changed.clone()),
        }],
    );
    let applied = result.unwrap();
    assert!(found.is_empty(), "{found:?}");
    let written = testkit::write::<Dxf>(&file).unwrap();
    let after = lift(&testkit::read::<Dxf>(&written).0.unwrap());
    let (plan_id, plan) = style(&after, "Plan");
    assert_eq!(plan_id, "dimension-style/h2C");
    assert_eq!(plan.ends, changed.ends);
    assert_eq!(plan.leader, changed.leader);
    assert_eq!((plan.size, plan.scale, plan.tick), (1.5, 100.0, Some(1.0)));
    assert_eq!(applied[0].id.as_deref(), Some("dimension-style/h2C"));
    assert_eq!(applied[0].hash, Some(Hash::dimension_style(plan)));
    // 公差も、新しい名前で指す
    let text = String::from_utf8(written).unwrap();
    assert!(text.contains("AcDbFcf\n  3\nPlan\n"), "{text}");
    assert!(!text.contains("Arch"), "{text}");
}

#[test]
fn dimension_style_changes_do_not_bind_existing_undefined_references() {
    let input = String::from_utf8(src())
        .unwrap()
        .replace("  3\nArch\n 10\n", "  3\nMissing\n 10\n");
    let drawing = lift(&testkit::read::<Dxf>(input.as_bytes()).0.unwrap());
    let mut renamed = style(&drawing, "Unused").1.clone();
    renamed.name = "Missing".to_owned();
    let mut added = style(&drawing, "Standard").1.clone();
    added.name = "Missing".to_owned();
    for edit in [
        Edit::ReplaceDimensionStyle {
            target: seen(&drawing, "Unused"),
            style: Box::new(renamed),
        },
        Edit::AddDimensionStyle {
            style: Box::new(added),
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
fn dimension_styles_are_added_and_removed() {
    let mut file = testkit::read::<Dxf>(&src()).0.unwrap();
    let drawing = lift(&file);
    let mut added = style(&drawing, "Standard").1.clone();
    added.name = "Civil".to_owned();
    added.size = 3.0;
    added.ends = [block("_OBLIQUE"), block("_OBLIQUE")];
    let (result, found) = apply(
        &mut file,
        vec![
            Edit::AddDimensionStyle {
                style: Box::new(added.clone()),
            },
            Edit::RemoveDimensionStyle {
                target: seen(&drawing, "Unused"),
            },
        ],
    );
    let applied = result.unwrap();
    assert!(found.is_empty(), "{found:?}");
    let written = testkit::write::<Dxf>(&file).unwrap();
    let after = lift(&testkit::read::<Dxf>(&written).0.unwrap());
    let (civil_id, civil) = style(&after, "Civil");
    assert_eq!(civil_id, "dimension-style/h40");
    assert_eq!((civil.size, &civil.ends), (3.0, &added.ends));
    assert_eq!(
        applied,
        [
            Applied {
                id: Some("dimension-style/h40".to_owned()),
                hash: Some(Hash::dimension_style(civil)),
            },
            Applied {
                id: None,
                hash: None,
            },
        ]
    );
    assert!(
        after
            .dimension_styles
            .iter()
            .all(|other| other.name != "Unused")
    );
    // 足したレコードのハンドルは 105 に置く
    let text = String::from_utf8(written).unwrap();
    assert!(text.contains("DIMSTYLE\n105\n40\n"), "{text}");
}

#[test]
fn dimension_style_changes_that_cannot_be_written_are_refused() {
    let src = src();
    let drawing = lift(&testkit::read::<Dxf>(&src).0.unwrap());
    let replace = |name: &str, change: &dyn Fn(&mut DimensionStyle)| {
        let mut changed = style(&drawing, name).1.clone();
        change(&mut changed);
        Edit::ReplaceDimensionStyle {
            target: seen(&drawing, name),
            style: Box::new(changed),
        }
    };
    let remove = |name: &str| Edit::RemoveDimensionStyle {
        target: seen(&drawing, name),
    };
    for edits in [
        // Standard・子を持つもの・子の名前と、ほかと重なる名前と、$ を持つ名前
        vec![replace("Standard", &|style| style.name = "Main".to_owned())],
        vec![replace("Family", &|style| style.name = "Other".to_owned())],
        vec![replace("Family$0", &|style| {
            style.name = "Other".to_owned()
        })],
        vec![replace("Unused", &|style| style.name = "arch".to_owned())],
        vec![replace("Unused", &|style| {
            style.name = "Spare$1".to_owned()
        })],
        // 負の大きさ、有限でない倍率、0 の斜線、ないブロック
        vec![replace("Unused", &|style| style.size = -1.0)],
        vec![replace("Unused", &|style| style.scale = f64::INFINITY)],
        vec![replace("Unused", &|style| style.tick = Some(0.0))],
        vec![replace("Unused", &|style| style.leader = block("_MISSING"))],
        // 同じ寸法スタイルの 2 度の変更
        vec![
            replace("Unused", &|style| style.size = 1.0),
            remove("Unused"),
        ],
        // Standard・子を持つもの・子と、公差が指すもの
        vec![remove("Standard")],
        vec![remove("Family")],
        vec![remove("Family$0")],
        vec![remove("Arch")],
    ] {
        let mut file = testkit::read::<Dxf>(&src).0.unwrap();
        let (result, found) = apply(&mut file, edits.clone());
        assert_eq!(result, Err(Error::Rejected), "{edits:?}");
        assert_eq!(found, [codes::CHANGE_EDIT_UNSUPPORTED], "{edits:?}");
        assert_eq!(testkit::write::<Dxf>(&file).unwrap(), src, "{edits:?}");
    }
    // 古いハッシュと、ない ID
    let mut stale = seen(&drawing, "Unused");
    stale.hash = Hash::dimension_style(style(&drawing, "Arch").1);
    let mut missing = seen(&drawing, "Unused");
    missing.id = "dimension-style/h99".to_owned();
    for (target, code) in [
        (stale, codes::CHANGE_ELEMENT_STALE),
        (missing, codes::CHANGE_ELEMENT_MISSING),
    ] {
        let mut file = testkit::read::<Dxf>(&src).0.unwrap();
        let (result, found) = apply(&mut file, vec![Edit::RemoveDimensionStyle { target }]);
        assert_eq!(result, Err(Error::Rejected));
        assert_eq!(found, [code]);
    }
}

#[test]
fn dimension_style_renames_count_record_values_and_stop_without_changing_the_file() {
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
        let mut new = style(&drawing, "Arch").1.clone();
        new.name = "Plan".to_owned();
        let changes = Changes {
            edits: vec![Edit::ReplaceDimensionStyle {
                target: seen(&drawing, "Arch"),
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
