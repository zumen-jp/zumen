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

/// 図形のタグを並べた DXF
#[cfg(test)]
fn dxf(entities: &[(i16, &str)]) -> Vec<u8> {
    versioned("AC1015", entities)
}

/// 版が `version` の、図形のタグを並べた DXF
#[cfg(test)]
fn versioned(version: &str, entities: &[(i16, &str)]) -> Vec<u8> {
    let head: &[(i16, &str)] = &[
        (0, "SECTION"),
        (2, "HEADER"),
        (9, "$ACADVER"),
        (1, version),
        (9, "$INSUNITS"),
        (70, "4"),
        (9, "$HANDSEED"),
        (5, "20"),
        (0, "ENDSEC"),
        (0, "SECTION"),
        (2, "ENTITIES"),
    ];
    let tail: &[(i16, &str)] = &[(0, "ENDSEC"), (0, "EOF")];
    head.iter()
        .chain(entities)
        .chain(tail)
        .map(|(code, value)| format!("{code:>3}\n{value}\n"))
        .collect::<String>()
        .into_bytes()
}

/// ARC・ELLIPSE・LWPOLYLINE を 1 つずつ持つ DXF
#[cfg(test)]
fn curves() -> Vec<u8> {
    dxf(&[
        (0, "ARC"),
        (5, "1A"),
        (8, "0"),
        (10, "0.0"),
        (20, "0.0"),
        (30, "0.0"),
        (40, "1.0"),
        (50, "0.0"),
        (51, "90.0"),
        (0, "ELLIPSE"),
        (5, "1B"),
        (8, "0"),
        (10, "10.0"),
        (20, "0.0"),
        (30, "0.0"),
        (11, "4.0"),
        (21, "0.0"),
        (31, "0.0"),
        (40, "0.5"),
        (41, "0.0"),
        (42, "6.283185307179586"),
        (0, "LWPOLYLINE"),
        (5, "1C"),
        (8, "0"),
        (90, "2"),
        (70, "0"),
        (10, "0.0"),
        (20, "5.0"),
        (10, "2.0"),
        (20, "5.0"),
        (42, "0.5"),
        (210, "0.0"),
        (220, "0.0"),
        (230, "1.0"),
    ])
}

#[test]
fn arcs_ellipses_and_lightweight_polylines_are_replaced() {
    use zumen_drawing::{Edge, ModelLength, Vertex};
    let mut file = testkit::read::<Dxf>(&curves()).0.unwrap();
    let [arc, ellipse, line] = <[_; 3]>::try_from(elements(&lift(&file))).unwrap();
    // 時計回りに 90 度から 0 度へたどる弧は、0 度から 90 度への反時計回りの弧と同じ
    let mut turned = arc.clone();
    if let Shape::Curve(Edge::Arc {
        center,
        radius,
        start,
        end,
        counterclockwise,
    }) = &mut turned.shape
    {
        center.x = 1.0;
        if let Length::Model(radius) = radius {
            radius.value = 2.5;
        }
        (*start, *end, *counterclockwise) = (180.0, 45.0, false);
    }
    let mut opened = ellipse.clone();
    if let Shape::Curve(Edge::Elliptic { end, .. }) = &mut opened.shape {
        *end = 180.0;
    }
    let mut longer = line.clone();
    let Shape::Polyline(polyline) = &mut longer.shape else {
        panic!("{:?}", longer.shape);
    };
    let at = polyline.vertices[0].point;
    polyline.vertices.push(Vertex {
        point: zumen_drawing::ModelPoint { y: 7.0, ..at },
        bulge: -0.25,
    });
    polyline.closed = true;
    polyline.width = Some(Length::Model(ModelLength {
        space: at.space,
        value: 0.5,
    }));
    let edits = vec![
        Edit::Replace(Box::new(turned.clone())),
        Edit::Replace(Box::new(opened.clone())),
        Edit::Replace(Box::new(longer.clone())),
    ];
    let (result, found) = apply(&mut file, edits);
    assert_eq!(result, Ok(()));
    assert_eq!(found, []);

    let written = testkit::write::<Dxf>(&file).unwrap();
    let again = testkit::read::<Dxf>(&written).0.unwrap();
    let [arc, ellipse, line] = <[_; 3]>::try_from(elements(&lift(&again))).unwrap();
    let Shape::Curve(Edge::Arc { start, end, .. }) = arc.shape else {
        panic!("{:?}", arc.shape);
    };
    assert_eq!((start, end), (45.0, 180.0));
    assert_eq!(ellipse.shape, opened.shape);
    assert_eq!(line.shape, longer.shape);
    let text = String::from_utf8(written).unwrap();
    // 変えていない楕円の欄と、頂点の後の押し出しの向きは、元の書き方のまま残る
    assert!(text.contains(" 11\n4.0\n 21\n0.0\n 31\n0.0\n 40\n0.5\n 41\n0.0\n"));
    assert!(text.contains(" 42\n-0.25\n210\n0.0\n220\n0.0\n230\n1.0\n"));
    assert!(text.contains(" 90\n3\n 70\n1\n 43\n0.5\n 10\n0.0\n 20\n5.0\n 10\n2.0\n"));
}

#[test]
fn replacements_the_records_cannot_hold_change_nothing() {
    use zumen_drawing::Edge;
    let src = dxf(&[
        (0, "ELLIPSE"),
        (5, "1B"),
        (8, "0"),
        (10, "10.0"),
        (20, "0.0"),
        (30, "0.0"),
        (11, "4.0"),
        (21, "0.0"),
        (31, "0.0"),
        (40, "0.5"),
        (0, "LWPOLYLINE"),
        (5, "1C"),
        (8, "0"),
        (90, "2"),
        (70, "0"),
        (10, "0.0"),
        (20, "5.0"),
        (91, "1"),
        (10, "2.0"),
        (20, "5.0"),
        (91, "2"),
    ]);
    let mut file = testkit::read::<Dxf>(&src).0.unwrap();
    let [ellipse, line] = <[_; 2]>::try_from(elements(&lift(&file))).unwrap();
    let mut flat = ellipse.clone();
    if let Shape::Curve(Edge::Elliptic { ratio, .. }) = &mut flat.shape {
        *ratio = 1.5;
    }
    let mut longer = line.clone();
    if let Shape::Polyline(polyline) = &mut longer.shape {
        let last = polyline.vertices[1];
        polyline.vertices.push(last);
    }
    let edits = vec![
        Edit::Replace(Box::new(flat)),
        Edit::Replace(Box::new(longer)),
    ];
    let (result, found) = apply(&mut file, edits);
    assert_eq!(result, Err(Error::Rejected));
    assert_eq!(
        found,
        [(
            codes::DXF_CHANGE_UNSUPPORTED,
            vec![ellipse.source, line.source]
        )]
    );
    assert_eq!(testkit::write::<Dxf>(&file).unwrap(), src);
}

/// 左の基準線に置く TEXT と、中央に置く TEXT を持つ DXF
#[cfg(test)]
fn texts(version: &str) -> Vec<u8> {
    versioned(
        version,
        &[
            (0, "TEXT"),
            (5, "1A"),
            (8, "0"),
            (10, "1.0"),
            (20, "2.0"),
            (30, "0.0"),
            (40, "2.5"),
            (1, "A-1"),
            (0, "TEXT"),
            (5, "1B"),
            (8, "0"),
            (10, "4.0"),
            (20, "0.0"),
            (30, "0.0"),
            (40, "2.5"),
            (1, "B"),
            (72, "1"),
            (11, "5.0"),
            (21, "0.0"),
            (31, "0.0"),
        ],
    )
}

#[test]
fn texts_are_moved_rewritten_and_turned() {
    use zumen_drawing::Placement;
    let mut file = testkit::read::<Dxf>(&texts("AC1021")).0.unwrap();
    let [left, centered] = <[_; 2]>::try_from(elements(&lift(&file))).unwrap();
    let mut changed = left.clone();
    let Shape::Text(text) = &mut changed.shape else {
        panic!("{:?}", changed.shape);
    };
    text.content = "通り芯 X1".to_owned();
    text.rotation = 90.0;
    text.backward = true;
    if let Length::Model(height) = &mut text.height.value {
        height.value = 3.5;
    }
    if let Placement::Anchor { point, .. } = &mut text.placement {
        point.x = 10.0;
    }
    let mut moved = centered.clone();
    if let Shape::Text(text) = &mut moved.shape
        && let Placement::Anchor { point, .. } = &mut text.placement
    {
        point.y = 3.0;
    }
    let edits = vec![
        Edit::Replace(Box::new(changed.clone())),
        Edit::Replace(Box::new(moved.clone())),
    ];
    let (result, found) = apply(&mut file, edits);
    assert_eq!(result, Ok(()));
    assert_eq!(found, []);

    let written = testkit::write::<Dxf>(&file).unwrap();
    let again = testkit::read::<Dxf>(&written).0.unwrap();
    let [left, centered] = <[_; 2]>::try_from(elements(&lift(&again))).unwrap();
    assert_eq!(left.shape, changed.shape);
    assert_eq!(centered.shape, moved.shape);
    let text = String::from_utf8(written).unwrap();
    // 回転と反転は、省いた欄を足して書く。中央に置く文字の 10 は、11 と同じだけ動く
    assert!(text.contains("  1\n通り芯 X1\n 50\n90.0\n 71\n2\n"));
    assert!(text.contains(" 10\n4.0\n 20\n3.0\n"));
}

#[test]
fn texts_that_the_records_cannot_hold_change_nothing() {
    use zumen_drawing::Placement;
    // 2007 より前の版は、ASCII でない文字を書かない
    let src = texts("AC1015");
    let mut file = testkit::read::<Dxf>(&src).0.unwrap();
    let [left, centered] = <[_; 2]>::try_from(elements(&lift(&file))).unwrap();
    let mut japanese = left.clone();
    if let Shape::Text(text) = &mut japanese.shape {
        text.content = "通り芯".to_owned();
    }
    let mut realigned = centered.clone();
    if let Shape::Text(text) = &mut realigned.shape
        && let Placement::Anchor { horizontal, .. } = &mut text.placement
    {
        *horizontal = zumen_drawing::Horizontal::Right;
    }
    let edits = vec![
        Edit::Replace(Box::new(japanese)),
        Edit::Replace(Box::new(realigned)),
    ];
    let (result, found) = apply(&mut file, edits);
    assert_eq!(result, Err(Error::Rejected));
    assert_eq!(
        found,
        [(
            codes::DXF_CHANGE_UNSUPPORTED,
            vec![left.source.clone(), centered.source]
        )]
    );
    let mut broken = left.clone();
    if let Shape::Text(text) = &mut broken.shape {
        text.content = "A\nB".to_owned();
    }
    let (result, _) = apply(&mut file, vec![Edit::Replace(Box::new(broken))]);
    assert_eq!(result, Err(Error::Rejected));
    assert_eq!(testkit::write::<Dxf>(&file).unwrap(), src);
}
