//! 図面モデルの変更を、DXF に書き戻す（決定 0139）。
#![cfg(feature = "drawing")]

use zumen_acad::{Dxf, DxfFile};
use zumen_base::{Budget, CancelToken, Code, Cx, Diagnostics, Ref, codes};
use zumen_drawing::{Changes, Drawing2D, Edit, Element, LayerId, Length, ModelPoint, Shape};
use zumen_sdk::{Apply, Error, Lift, testkit};

/// LINE・CIRCLE・ARC を 1 つずつ持つ DXF
#[cfg(test)]
fn src() -> Vec<u8> {
    let tags: &[(i16, &str)] = &[
        (0, "SECTION"),
        (2, "HEADER"),
        (9, "$INSUNITS"),
        (70, "4"),
        (9, "$HANDSEED"),
        (5, "20"),
        (0, "ENDSEC"),
        (0, "SECTION"),
        (2, "ENTITIES"),
        (0, "LINE"),
        (5, "1A"),
        (8, "0"),
        (10, "0.0"),
        (20, "0.0"),
        (30, "0.0"),
        (11, "1.0"),
        (21, "1.0"),
        (31, "0.0"),
        (0, "CIRCLE"),
        (5, "1B"),
        (8, "0"),
        (10, "5.0"),
        (20, "5.0"),
        (30, "0.0"),
        (40, "2.0"),
        (0, "ARC"),
        (5, "1C"),
        (8, "0"),
        (10, "0.0"),
        (20, "0.0"),
        (30, "0.0"),
        (40, "1.0"),
        (50, "0.0"),
        (51, "90.0"),
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

/// 当てた結果と、診断の code と場所
type Applied = (Result<(), Error>, Vec<(Code, Vec<Ref>)>);

#[cfg(test)]
fn apply(file: &mut DxfFile, edits: Vec<Edit>) -> Applied {
    let mut budget = Budget::default();
    let mut diag = Diagnostics::new();
    let result = file.apply(
        &Changes { edits },
        &mut Cx::new(&mut budget, &mut diag, &CancelToken::new()),
    );
    let found = diag
        .iter()
        .map(|d| (d.code(), d.places().to_vec()))
        .collect();
    (result, found)
}

#[cfg(test)]
fn elements(drawing: &Drawing2D) -> Vec<Element<ModelPoint>> {
    drawing.spaces.first().unwrap().elements.clone()
}

#[test]
fn lines_and_circles_are_moved_and_entities_removed() {
    let mut file = testkit::read::<Dxf>(&src()).0.unwrap();
    let before = elements(&lift(&file));
    let [line, circle, arc] = <[_; 3]>::try_from(before).unwrap();
    let mut moved = line.clone();
    if let Shape::Line { end, .. } = &mut moved.shape {
        end.x = 3.5;
        end.y = -2.0;
    }
    let mut grown = circle.clone();
    if let Shape::Circle { center, radius } = &mut grown.shape {
        center.x = 6.0;
        if let Length::Model(radius) = radius {
            radius.value = 4.25;
        }
    }
    let edits = vec![
        Edit::Replace(Box::new(moved.clone())),
        Edit::Replace(Box::new(grown.clone())),
        Edit::Remove(arc.source.clone()),
    ];
    let (result, found) = apply(&mut file, edits);
    assert_eq!(result, Ok(()));
    assert_eq!(found, []);

    let written = testkit::write::<Dxf>(&file).unwrap();
    let again = testkit::read::<Dxf>(&written).0.unwrap();
    let after = elements(&lift(&again));
    assert_eq!(after.len(), 2);
    assert_eq!(after[0].shape, moved.shape);
    assert_eq!(after[1].shape, grown.shape);
    // 触れていない値は、元の書き方のまま残る
    let text = String::from_utf8(written).unwrap();
    assert!(text.contains(
        "  5\n1A\n  8\n0\n 10\n0.0\n 20\n0.0\n 30\n0.0\n 11\n3.5\n 21\n-2.0\n 31\n0.0\n"
    ));
    assert!(!text.contains("ARC"));
}

#[test]
fn changes_that_cannot_be_written_back_change_nothing() {
    let src = src();
    let mut file = testkit::read::<Dxf>(&src).0.unwrap();
    let [line, circle, arc] = <[_; 3]>::try_from(elements(&lift(&file))).unwrap();
    let mut relayered = line.clone();
    relayered.layer = LayerId(7);
    let mut bent = arc.clone();
    bent.shape = circle.shape.clone();
    let nowhere = Ref::at_record(999);
    let edits = vec![
        Edit::Replace(Box::new(relayered)),
        Edit::Replace(Box::new(bent)),
        Edit::Remove(nowhere.clone()),
        Edit::Remove(circle.source.clone()),
    ];
    let (result, found) = apply(&mut file, edits);
    assert_eq!(result, Err(Error::Rejected));
    assert_eq!(
        found,
        [
            (codes::DXF_CHANGE_UNSUPPORTED, vec![line.source, arc.source]),
            (codes::DXF_ELEMENT_MISSING, vec![nowhere]),
        ]
    );
    assert_eq!(testkit::write::<Dxf>(&file).unwrap(), src);
}
