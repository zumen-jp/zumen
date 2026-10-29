//! 図面モデルの変更を、DXF に書き戻す（決定 0139）。
#![cfg(feature = "drawing")]

mod by_source;

use by_source::{Apply, Changes, Edit};
use zumen_acad::{Dxf, DxfFile};
use zumen_base::{Budget, CancelToken, Code, Cx, Diagnostics, Ref, codes};
use zumen_drawing::{Drawing2D, Element, LayerId, Length, ModelPoint, Shape};
use zumen_sdk::{Error, Lift, testkit};

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
            (
                codes::CHANGE_EDIT_UNSUPPORTED,
                vec![line.source, arc.source]
            ),
            // ない要素の診断は、出所ではなく ID を値に持つ
            (codes::CHANGE_ELEMENT_MISSING, vec![]),
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
            codes::CHANGE_EDIT_UNSUPPORTED,
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
            codes::CHANGE_EDIT_UNSUPPORTED,
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

/// MTEXT を 2 つ持つ DXF。2 つ目は、向きを 11・21 で持ち、内容を 250 バイトの区切り（3）でも持つ
#[cfg(test)]
fn mtexts(version: &str, long: &str) -> Vec<u8> {
    versioned(
        version,
        &[
            (0, "MTEXT"),
            (5, "1A"),
            (8, "0"),
            (10, "1.0"),
            (20, "2.0"),
            (30, "0.0"),
            (40, "2.5"),
            (41, "20.0"),
            (71, "1"),
            (72, "1"),
            (1, "{\\fArial|b1;A}\\PB"),
            (0, "MTEXT"),
            (5, "1B"),
            (8, "0"),
            (10, "4.0"),
            (20, "0.0"),
            (30, "0.0"),
            (40, "2.5"),
            (71, "5"),
            (72, "1"),
            (3, long),
            (1, "end"),
            (11, "1.0"),
            (21, "0.0"),
            (31, "0.0"),
        ],
    )
}

/// 内容 `content` を読んで得る区間。区間は内容から決まるので、内容を変えるときは区間も合わせる
#[cfg(test)]
fn spans(content: &str) -> Vec<zumen_drawing::Span> {
    let src = versioned(
        "AC1021",
        &[
            (0, "MTEXT"),
            (5, "1A"),
            (8, "0"),
            (10, "0.0"),
            (20, "0.0"),
            (40, "2.5"),
            (1, content),
        ],
    );
    let file = testkit::read::<Dxf>(&src).0.unwrap();
    match &elements(&lift(&file))[0].shape {
        Shape::Formatted(text) => text.spans.clone(),
        shape => panic!("{shape:?}"),
    }
}

#[test]
fn mtexts_are_moved_rewritten_and_reframed() {
    use zumen_drawing::{Horizontal, LineSpacing, Rotation, Vertical};
    let long = "x".repeat(250);
    let mut file = testkit::read::<Dxf>(&mtexts("AC1021", &long)).0.unwrap();
    let [first, second] = <[_; 2]>::try_from(elements(&lift(&file))).unwrap();
    let mut changed = first.clone();
    let mut moved = second.clone();
    let (Shape::Formatted(text), Shape::Formatted(other)) = (&mut changed.shape, &mut moved.shape)
    else {
        panic!("{:?}", first.shape);
    };
    // 300 バイトを超える内容は、文字の途中で切らずに 3 と 1 に分ける
    text.content = format!("{{\\H2x;通り芯}}\\P{}", "あ".repeat(100));
    text.spans = spans(&text.content);
    text.point.x = 10.0;
    text.rotation = Rotation::Angle(90.0);
    text.width = None;
    text.horizontal = Horizontal::Center;
    text.vertical = Vertical::Middle;
    text.line_spacing = LineSpacing {
        factor: 1.5,
        exact: true,
    };
    other.rotation = Rotation::Direction([0.0, 1.0]);
    other.point.y = 3.0;
    let edits = vec![
        Edit::Replace(Box::new(changed.clone())),
        Edit::Replace(Box::new(moved.clone())),
    ];
    let (result, found) = apply(&mut file, edits);
    assert_eq!(result, Ok(()));
    assert_eq!(found, []);

    let written = testkit::write::<Dxf>(&file).unwrap();
    let again = testkit::read::<Dxf>(&written).0.unwrap();
    let [first, second] = <[_; 2]>::try_from(elements(&lift(&again))).unwrap();
    assert_eq!(first.shape, changed.shape);
    assert_eq!(second.shape, moved.shape);
    let text = String::from_utf8(written).unwrap();
    // 枠の幅は 0 にし、位置合わせを直す。角度（ラジアン）と行送りは、省いた欄を内容の後に足す
    assert!(text.contains(" 41\n0.0\n 71\n5\n"), "{text}");
    assert!(
        text.contains("あ\n 50\n1.5707963267948966\n 73\n2\n 44\n1.5\n"),
        "{text}"
    );
    assert!(text.contains(" 11\n0.0\n 21\n1.0\n"), "{text}");
    // 元の区切りの内容は、変えていないのでそのまま
    assert!(text.contains(&format!("  3\n{long}\n  1\nend\n")));
}

#[test]
fn mtexts_that_the_records_cannot_hold_change_nothing() {
    use zumen_drawing::Rotation;
    let src = mtexts("AC1015", "abc");
    let mut file = testkit::read::<Dxf>(&src).0.unwrap();
    let [first, second] = <[_; 2]>::try_from(elements(&lift(&file))).unwrap();
    let edit = |element: &Element<ModelPoint>,
                change: &dyn Fn(&mut zumen_drawing::FormattedText<ModelPoint>)| {
        let mut element = element.clone();
        let Shape::Formatted(text) = &mut element.shape else {
            panic!("{:?}", element.shape);
        };
        change(text);
        Edit::Replace(Box::new(element))
    };
    let cases = [
        // 2007 より前の版は、ASCII でない文字を書かない
        edit(&first, &|text| {
            text.content = "通り芯".to_owned();
            text.spans = spans(&text.content);
        }),
        // 内容と合わない区間
        edit(&first, &|text| text.spans.clear()),
        // 改行は、値に書けない
        edit(&first, &|text| text.content = "A\nB".to_owned()),
        // 文字スタイルは変えられない
        edit(&first, &|text| text.style = "OTHER".to_owned()),
        // 向きの形（角度か向き）は変えられない
        edit(&second, &|text| text.rotation = Rotation::Angle(0.0)),
    ];
    for case in cases {
        let (result, found) = apply(&mut file, vec![case]);
        assert_eq!(result, Err(Error::Rejected));
        assert_eq!(found.len(), 1, "{found:?}");
        assert_eq!(found[0].0, codes::CHANGE_EDIT_UNSUPPORTED);
    }
    assert_eq!(testkit::write::<Dxf>(&file).unwrap(), src);
}

/// ブロック B を定義し、INSERT を 2 つ持つ DXF。1 つ目は尺度と回転を省き、2 つ目は持つ
#[cfg(test)]
fn inserts() -> Vec<u8> {
    let tags: &[(i16, &str)] = &[
        (0, "SECTION"),
        (2, "HEADER"),
        (9, "$INSUNITS"),
        (70, "4"),
        (9, "$HANDSEED"),
        (5, "20"),
        (0, "ENDSEC"),
        (0, "SECTION"),
        (2, "BLOCKS"),
        (0, "BLOCK"),
        (8, "0"),
        (2, "B"),
        (70, "0"),
        (10, "0.0"),
        (20, "0.0"),
        (30, "0.0"),
        (0, "LINE"),
        (8, "0"),
        (10, "0.0"),
        (20, "0.0"),
        (30, "0.0"),
        (11, "1.0"),
        (21, "0.0"),
        (31, "0.0"),
        (0, "ENDBLK"),
        (8, "0"),
        (0, "ENDSEC"),
        (0, "SECTION"),
        (2, "ENTITIES"),
        (0, "INSERT"),
        (5, "1A"),
        (8, "0"),
        (2, "B"),
        (10, "0.0"),
        (20, "0.0"),
        (30, "0.0"),
        (0, "INSERT"),
        (5, "1B"),
        (8, "0"),
        (2, "B"),
        (10, "5.0"),
        (20, "5.0"),
        (30, "0.0"),
        (41, "2.0"),
        (42, "2.0"),
        (43, "2.0"),
        (50, "30.0"),
        (0, "ENDSEC"),
        (0, "EOF"),
    ];
    tags.iter()
        .map(|(code, value)| format!("{code:>3}\n{value}\n"))
        .collect::<String>()
        .into_bytes()
}

#[test]
fn inserts_are_moved_scaled_and_turned() {
    let mut file = testkit::read::<Dxf>(&inserts()).0.unwrap();
    let [first, second] = <[_; 2]>::try_from(elements(&lift(&file))).unwrap();
    let mut placed = first.clone();
    let mut reset = second.clone();
    let (Shape::Symbol(symbol), Shape::Symbol(other)) = (&mut placed.shape, &mut reset.shape)
    else {
        panic!("{:?}", first.shape);
    };
    symbol.position.x = 3.0;
    symbol.position.y = -1.0;
    symbol.scale = [-1.0, 0.5];
    symbol.rotation = 45.0;
    // 既定の値に戻した欄も、元が持っていれば書く
    other.scale = [1.0, 1.0];
    other.rotation = 0.0;
    let edits = vec![
        Edit::Replace(Box::new(placed.clone())),
        Edit::Replace(Box::new(reset.clone())),
    ];
    let (result, found) = apply(&mut file, edits);
    assert_eq!(result, Ok(()));
    assert!(found.is_empty(), "{found:?}");
    let written = testkit::write::<Dxf>(&file).unwrap();
    let text = String::from_utf8(written.clone()).unwrap();
    assert!(
        text.contains(" 10\n3.0\n 20\n-1.0\n 30\n0.0\n 41\n-1.0\n 42\n0.5\n 50\n45.0\n"),
        "{text}"
    );
    assert!(
        text.contains(" 41\n1.0\n 42\n1.0\n 43\n2.0\n 50\n0.0\n"),
        "{text}"
    );
    let after = elements(&lift(&testkit::read::<Dxf>(&written).0.unwrap()));
    assert_eq!(after, [placed, reset]);
}

#[test]
fn inserts_that_the_records_cannot_hold_change_nothing() {
    use zumen_drawing::Grid;
    let src = inserts();
    let mut file = testkit::read::<Dxf>(&src).0.unwrap();
    let [first, second] = <[_; 2]>::try_from(elements(&lift(&file))).unwrap();
    let mut renamed = first.clone();
    let mut gridded = second.clone();
    let mut flattened = second.clone();
    if let Shape::Symbol(symbol) = &mut renamed.shape {
        symbol.key.name = "C".to_owned();
    }
    if let Shape::Symbol(symbol) = &mut gridded.shape {
        symbol.grid = Some(Grid {
            columns: 2,
            rows: 1,
            column_spacing: Length::Paper(zumen_drawing::PaperLength(1.0)),
            row_spacing: Length::Paper(zumen_drawing::PaperLength(1.0)),
        });
    }
    if let Shape::Symbol(symbol) = &mut flattened.shape {
        symbol.scale = [0.0, 1.0];
    }
    for edit in [renamed, gridded, flattened] {
        let at = edit.source.clone();
        let (result, found) = apply(&mut file, vec![Edit::Replace(Box::new(edit))]);
        assert_eq!(result, Err(Error::Rejected));
        assert_eq!(found, [(codes::CHANGE_EDIT_UNSUPPORTED, vec![at])]);
    }
    assert_eq!(testkit::write::<Dxf>(&file).unwrap(), src);
}

/// XDATA を持つ LINE を 2 つ持つ DXF。1 つ目は文字列だけを、2 つ目は図形とともに動く位置（1011）を持つ
#[cfg(test)]
fn xdata_lines() -> Vec<u8> {
    dxf(&[
        (0, "LINE"),
        (5, "1A"),
        (8, "0"),
        (10, "0.0"),
        (20, "0.0"),
        (30, "0.0"),
        (11, "1.0"),
        (21, "1.0"),
        (31, "0.0"),
        (1001, "ZUMEN"),
        (1000, "kept"),
        (0, "LINE"),
        (5, "1B"),
        (8, "0"),
        (10, "0.0"),
        (20, "0.0"),
        (30, "0.0"),
        (11, "1.0"),
        (21, "1.0"),
        (31, "0.0"),
        (1001, "ZUMEN"),
        (1000, "anchor"),
        (1011, "1.0"),
        (1021, "1.0"),
        (1031, "0.0"),
    ])
}

#[test]
fn xdata_is_kept_unless_it_moves_with_the_entity() {
    let src = xdata_lines();
    let mut file = testkit::read::<Dxf>(&src).0.unwrap();
    let [plain, anchored] = <[_; 2]>::try_from(elements(&lift(&file))).unwrap();
    let moved = |element: &Element<ModelPoint>| {
        let mut element = element.clone();
        if let Shape::Line { end, .. } = &mut element.shape {
            end.x = 4.0;
        }
        element
    };
    // 位置を持つ XDATA があれば、もう 1 つの変更も含めて何も反映しない
    let edits = vec![
        Edit::Replace(Box::new(moved(&plain))),
        Edit::Replace(Box::new(moved(&anchored))),
    ];
    let (result, found) = apply(&mut file, edits);
    assert_eq!(result, Err(Error::Rejected));
    assert_eq!(
        found,
        [(
            codes::CHANGE_XDATA_UNSUPPORTED,
            vec![anchored.source.clone()]
        )]
    );
    assert_eq!(testkit::write::<Dxf>(&file).unwrap(), src);
    // 形を変えなければ、位置を持つ XDATA があっても当てられる
    let (result, found) = apply(
        &mut file,
        vec![
            Edit::Replace(Box::new(moved(&plain))),
            Edit::Replace(Box::new(anchored.clone())),
        ],
    );
    assert_eq!(result, Ok(()));
    assert!(found.is_empty(), "{found:?}");
    let text = String::from_utf8(testkit::write::<Dxf>(&file).unwrap()).unwrap();
    assert!(
        text.contains(" 11\n4.0\n 21\n1.0\n 31\n0.0\n1001\nZUMEN\n1000\nkept\n"),
        "{text}"
    );
}

/// 長さの単位がインチで、用紙の原点（$PLIMMIN）が (10, 20) の DXF。ペーパー空間に LINE と TEXT を、
/// モデル空間に CIRCLE を持つ
#[cfg(test)]
fn sheet() -> Vec<u8> {
    let tags: &[(i16, &str)] = &[
        (0, "SECTION"),
        (2, "HEADER"),
        (9, "$INSUNITS"),
        (70, "1"),
        (9, "$HANDSEED"),
        (5, "20"),
        (9, "$PLIMMIN"),
        (10, "10.0"),
        (20, "20.0"),
        (9, "$PLIMMAX"),
        (10, "30.0"),
        (20, "40.0"),
        (0, "ENDSEC"),
        (0, "SECTION"),
        (2, "ENTITIES"),
        (0, "LINE"),
        (5, "1A"),
        (67, "1"),
        (8, "0"),
        (10, "10.0"),
        (20, "20.0"),
        (30, "0.0"),
        (11, "11.0"),
        (21, "21.0"),
        (31, "0.0"),
        (0, "TEXT"),
        (5, "1B"),
        (67, "1"),
        (8, "0"),
        (10, "12.0"),
        (20, "22.0"),
        (30, "0.0"),
        (40, "0.5"),
        (1, "A"),
        (0, "CIRCLE"),
        (5, "1C"),
        (8, "0"),
        (10, "0.0"),
        (20, "0.0"),
        (30, "0.0"),
        (40, "1.0"),
        (0, "ENDSEC"),
        (0, "EOF"),
    ];
    tags.iter()
        .map(|(code, value)| format!("{code:>3}\n{value}\n"))
        .collect::<String>()
        .into_bytes()
}

#[test]
fn sheet_elements_are_moved_in_paper_millimeters_and_removed() {
    use zumen_drawing::{PaperLength, PaperPoint, Placement};
    let mut file = testkit::read::<Dxf>(&sheet()).0.unwrap();
    let drawing = lift(&file);
    let [line, text] =
        <[_; 2]>::try_from(drawing.sheets.first().unwrap().elements.clone()).unwrap();
    assert_eq!(
        line.shape,
        Shape::Line {
            start: PaperPoint {
                sheet: zumen_drawing::SheetId(0),
                x: 0.0,
                y: 0.0
            },
            end: PaperPoint {
                sheet: zumen_drawing::SheetId(0),
                x: 25.4,
                y: 25.4
            },
        }
    );
    // 用紙の 50.8 mm は、原点からの 2 インチ
    let mut moved = line.clone();
    if let Shape::Line { end, .. } = &mut moved.shape {
        end.x = 50.8;
    }
    let mut taller = text.clone();
    if let Shape::Text(text) = &mut taller.shape {
        text.height.value = Length::Paper(PaperLength(25.4));
        if let Placement::Anchor { point, .. } = &mut text.placement {
            point.y = 0.0;
        }
    }
    let edits = vec![
        Edit::ReplaceOnSheet(Box::new(moved.clone())),
        Edit::ReplaceOnSheet(Box::new(taller.clone())),
    ];
    let (result, found) = apply(&mut file, edits);
    assert_eq!(result, Ok(()));
    assert!(found.is_empty(), "{found:?}");
    let written = testkit::write::<Dxf>(&file).unwrap();
    let text_out = String::from_utf8(written.clone()).unwrap();
    // 変えていない始まりの点は、元の値のまま
    assert!(
        text_out.contains(" 10\n10.0\n 20\n20.0\n 30\n0.0\n 11\n12.0\n 21\n21.0\n"),
        "{text_out}"
    );
    assert!(
        text_out.contains(" 10\n12.0\n 20\n20.0\n 30\n0.0\n 40\n1.0\n"),
        "{text_out}"
    );
    let after = lift(&testkit::read::<Dxf>(&written).0.unwrap());
    assert_eq!(after.sheets.first().unwrap().elements, [moved, taller]);
    // 用紙の要素も除ける
    let mut file = testkit::read::<Dxf>(&written).0.unwrap();
    let (result, _) = apply(&mut file, vec![Edit::Remove(line.source.clone())]);
    assert_eq!(result, Ok(()));
    let after = lift(
        &testkit::read::<Dxf>(&testkit::write::<Dxf>(&file).unwrap())
            .0
            .unwrap(),
    );
    assert_eq!(after.sheets.first().unwrap().elements.len(), 1);
}

#[test]
fn sheet_and_model_replacements_do_not_cross() {
    let src = sheet();
    let mut file = testkit::read::<Dxf>(&src).0.unwrap();
    let drawing = lift(&file);
    let line = drawing
        .sheets
        .first()
        .unwrap()
        .elements
        .first()
        .unwrap()
        .clone();
    let circle = elements(&drawing).first().unwrap().clone();
    // 用紙の要素を実寸の空間の置き換えで指しても、実寸の空間の要素を用紙の置き換えで指しても、見つからない
    let as_model = Element {
        shape: Shape::Line {
            start: ModelPoint {
                space: zumen_drawing::SpaceId(0),
                x: 0.0,
                y: 0.0,
            },
            end: ModelPoint {
                space: zumen_drawing::SpaceId(0),
                x: 1.0,
                y: 1.0,
            },
        },
        layer: line.layer,
        style: line.style,
        source: line.source.clone(),
    };
    let as_paper = Element {
        shape: line.shape.clone(),
        layer: circle.layer,
        style: circle.style,
        source: circle.source.clone(),
    };
    let edits = vec![
        Edit::Replace(Box::new(as_model)),
        Edit::ReplaceOnSheet(Box::new(as_paper)),
    ];
    let (result, found) = apply(&mut file, edits);
    assert_eq!(result, Err(Error::Rejected));
    assert_eq!(
        found,
        // ID は空間か用紙を表すので、要素はあるが、別の空間の要素では置き換えられない
        [(
            codes::CHANGE_EDIT_UNSUPPORTED,
            vec![line.source.clone(), circle.source.clone()]
        )]
    );
    assert_eq!(testkit::write::<Dxf>(&file).unwrap(), src);
}

/// 実寸の空間の点
#[cfg(test)]
fn at(x: f64, y: f64) -> ModelPoint {
    ModelPoint {
        space: zumen_drawing::SpaceId(0),
        x,
        y,
    }
}

/// 実寸の空間の長さ
#[cfg(test)]
fn model(value: f64) -> Length {
    Length::Model(zumen_drawing::ModelLength {
        space: zumen_drawing::SpaceId(0),
        value,
    })
}

/// 足す線分・円弧・楕円・軽量の折れ線・文字
#[cfg(test)]
fn newcomers() -> Vec<Shape<ModelPoint>> {
    use zumen_drawing::{
        Edge, Flow, Format, FormattedText, Height, HeightMeaning, Horizontal, LineSpacing, Piece,
        Placement, Polyline, Rotation, Span, Text, Vertex, Vertical,
    };
    let span = |piece| Span {
        piece,
        format: Format::default(),
    };
    vec![
        Shape::Line {
            start: at(0.0, 0.0),
            end: at(2.0, 3.0),
        },
        Shape::Curve(Edge::Arc {
            center: at(1.0, 1.0),
            radius: model(2.0),
            start: 30.0,
            end: 120.0,
            counterclockwise: true,
        }),
        Shape::Curve(Edge::Elliptic {
            center: at(5.0, 5.0),
            major: at(8.0, 5.0),
            ratio: 0.5,
            start: 0.0,
            end: 180.0,
            counterclockwise: true,
        }),
        Shape::Polyline(Polyline {
            vertices: vec![
                Vertex {
                    point: at(0.0, 0.0),
                    bulge: 0.5,
                },
                Vertex {
                    point: at(4.0, 0.0),
                    bulge: 0.0,
                },
                Vertex {
                    point: at(4.0, 3.0),
                    bulge: 0.0,
                },
            ],
            closed: true,
            width: None,
            vertex_widths: Vec::new(),
        }),
        Shape::Text(Text {
            content: "zumen".to_owned(),
            height: Height {
                value: model(2.5),
                meaning: HeightMeaning::Declared("dxf.TEXT.40"),
            },
            placement: Placement::Anchor {
                point: at(3.0, 3.0),
                horizontal: Horizontal::Center,
                vertical: Vertical::Middle,
            },
            rotation: 15.0,
            width_factor: 1.0,
            oblique: 0.0,
            backward: false,
            upside_down: false,
            style: "STANDARD".to_owned(),
        }),
        Shape::Formatted(FormattedText {
            content: "zumen\\Pdrawing".to_owned(),
            spans: vec![
                span(Piece::Text("zumen".to_owned())),
                span(Piece::Paragraph),
                span(Piece::Text("drawing".to_owned())),
            ],
            height: Height {
                value: model(2.5),
                meaning: HeightMeaning::Declared("dxf.MTEXT.40"),
            },
            point: at(6.0, 1.0),
            horizontal: Horizontal::Center,
            vertical: Vertical::Top,
            rotation: Rotation::Angle(0.0),
            width: Some(model(10.0)),
            frame_height: None,
            flow: Flow::LeftToRight,
            line_spacing: LineSpacing {
                factor: 1.5,
                exact: true,
            },
            background: None,
            style: "STANDARD".to_owned(),
        }),
    ]
}

#[test]
fn elements_are_added_with_new_handles() {
    use zumen_drawing::{Color, ColorCode, ColorSystem, Inherit};
    let mut file = testkit::read::<Dxf>(&src()).0.unwrap();
    let before = elements(&lift(&file));
    let shapes = newcomers();
    let edits = shapes
        .iter()
        .enumerate()
        .map(|(index, shape)| {
            let mut element = before[0].clone();
            element.shape = shape.clone();
            if index == 0 {
                element.style.color = Inherit::Own(Color {
                    code: Some(ColorCode {
                        system: ColorSystem::Aci,
                        value: 3,
                    }),
                    rgb: None,
                });
            }
            Edit::Add(Box::new(element))
        })
        .collect();
    let (result, found) = apply(&mut file, edits);
    assert_eq!(result, Ok(()));
    assert_eq!(found, []);

    let written = testkit::write::<Dxf>(&file).unwrap();
    let after = elements(&lift(&testkit::read::<Dxf>(&written).0.unwrap()));
    assert_eq!(after.len(), before.len() + shapes.len());
    let added: Vec<_> = after[before.len()..]
        .iter()
        .map(|e| e.shape.clone())
        .collect();
    assert_eq!(added, shapes);
    assert!(
        after[before.len()..]
            .iter()
            .all(|e| e.layer == before[0].layer)
    );
    // ハンドルは $HANDSEED（20）から順に付け、$HANDSEED を最後のハンドルの次に進める。区切り（100）を
    // 持たない表には、区切りを書かない
    let text = String::from_utf8(written).unwrap();
    assert!(text.contains("  0\nLINE\n  5\n20\n  8\n0\n 62\n3\n 10\n0.0\n"));
    assert!(text.contains("  0\nTEXT\n  5\n24\n"));
    assert!(text.contains("  0\nMTEXT\n  5\n25\n"));
    assert!(text.contains("$HANDSEED\n  5\n26\n"));
    assert!(!text.contains("AcDb"));
}

#[test]
fn elements_that_cannot_be_added_change_nothing() {
    use zumen_drawing::{Placement, SpaceId};
    let src = src();
    let mut file = testkit::read::<Dxf>(&src).0.unwrap();
    let template = elements(&lift(&file)).swap_remove(0);
    let shapes = newcomers();
    let add = |record: u64, shape: Shape<ModelPoint>, layer: LayerId| {
        let mut element = template.clone();
        element.source = Ref::at_record(record);
        element.shape = shape;
        element.layer = layer;
        Edit::Add(Box::new(element))
    };
    // 図面にないレイヤ、半径が 0 の円、ほかの空間の線分、改行を持つ文字、内容と合わない区間を持つ書式つきの
    // 文字は足せない
    let mut elsewhere = shapes[0].clone();
    if let Shape::Line { end, .. } = &mut elsewhere {
        end.space = SpaceId(1);
    }
    let mut broken = shapes[4].clone();
    if let Shape::Text(text) = &mut broken {
        text.content = "a\nb".to_owned();
        if let Placement::Anchor { point, .. } = &mut text.placement {
            point.x = 1.0;
        }
    }
    let mut stale = shapes[5].clone();
    if let Shape::Formatted(text) = &mut stale {
        text.content = "zumen".to_owned();
    }
    let edits = vec![
        add(901, shapes[0].clone(), LayerId(99)),
        add(
            902,
            Shape::Circle {
                center: at(0.0, 0.0),
                radius: model(0.0),
            },
            template.layer,
        ),
        add(903, elsewhere, template.layer),
        add(904, broken, template.layer),
        add(905, stale, template.layer),
        add(906, shapes[1].clone(), template.layer),
    ];
    let (result, found) = apply(&mut file, edits);
    assert_eq!(result, Err(Error::Rejected));
    assert_eq!(
        found,
        // 足す要素の出所は使わないので、診断は場所を持たない
        [(codes::CHANGE_EDIT_UNSUPPORTED, vec![])]
    );
    assert_eq!(testkit::write::<Dxf>(&file).unwrap(), src);
}

#[test]
fn sheet_elements_are_added_in_paper_millimeters() {
    use zumen_drawing::{PaperLength, PaperPoint, SheetId};
    let mut file = testkit::read::<Dxf>(&sheet()).0.unwrap();
    let drawing = lift(&file);
    let template = drawing.sheets[0].elements[0].clone();
    let at = |sheet: u32, x: f64, y: f64| PaperPoint {
        sheet: SheetId(sheet),
        x,
        y,
    };
    let mut line = template.clone();
    line.shape = Shape::Line {
        start: at(0, 25.4, 50.8),
        end: at(0, 50.8, 25.4),
    };
    let mut circle = template.clone();
    circle.shape = Shape::Circle {
        center: at(0, 25.4, 25.4),
        radius: Length::Paper(PaperLength(12.7)),
    };
    let (result, found) = apply(
        &mut file,
        vec![
            Edit::AddOnSheet(Box::new(line.clone())),
            Edit::AddOnSheet(Box::new(circle.clone())),
        ],
    );
    assert_eq!(result, Ok(()));
    assert_eq!(found, []);
    let written = testkit::write::<Dxf>(&file).unwrap();
    let after = lift(&testkit::read::<Dxf>(&written).0.unwrap());
    let shapes: Vec<_> = after.sheets[0].elements[2..]
        .iter()
        .map(|e| e.shape.clone())
        .collect();
    assert_eq!(shapes, [line.shape, circle.shape]);
    // 用紙の mm を図面の単位（インチ）に戻して用紙の原点を足し、ペーパー空間の印を付ける。値は、インチに
    // 戻して最後の桁が違わないものを選ぶ
    let text = String::from_utf8(written).unwrap();
    assert!(text.contains(
        "  0\nLINE\n  5\n20\n 67\n1\n  8\n0\n 10\n11.0\n 20\n22.0\n 30\n0.0\n 11\n12.0\n 21\n21.0\n"
    ));
    assert_eq!(elements(&after).len(), 1);
}

#[test]
fn sheet_elements_need_a_known_sheet() {
    use zumen_drawing::{PaperPoint, SheetId};
    let src = sheet();
    let mut file = testkit::read::<Dxf>(&src).0.unwrap();
    let mut line = lift(&file).sheets[0].elements[0].clone();
    let at = |x: f64| PaperPoint {
        sheet: SheetId(5),
        x,
        y: 0.0,
    };
    line.shape = Shape::Line {
        start: at(0.0),
        end: at(1.0),
    };
    line.source = Ref::at_record(900);
    let (result, found) = apply(&mut file, vec![Edit::AddOnSheet(Box::new(line))]);
    assert_eq!(result, Err(Error::Rejected));
    assert_eq!(
        found,
        // 用紙がないので、足す先がない
        [(codes::CHANGE_ELEMENT_MISSING, vec![])]
    );
    assert_eq!(testkit::write::<Dxf>(&file).unwrap(), src);
}

/// 定義を持つ空の DXF。BLOCK_RECORD の表は持たない。
fn addition_source(version: &str, seed: &str, blocks: &[(i16, &str)]) -> Vec<u8> {
    let mut tags = vec![
        (0, "SECTION"),
        (2, "HEADER"),
        (9, "$ACADVER"),
        (1, version),
        (9, "$INSUNITS"),
        (70, "4"),
        (9, "$HANDSEED"),
        (5, seed),
        (0, "ENDSEC"),
        (0, "SECTION"),
        (2, "TABLES"),
        (0, "TABLE"),
        (2, "LAYER"),
        (0, "LAYER"),
        (5, "10"),
        (2, "0"),
        (70, "0"),
        (62, "7"),
        (0, "ENDTAB"),
        (0, "ENDSEC"),
        (0, "SECTION"),
        (2, "BLOCKS"),
    ];
    tags.extend_from_slice(blocks);
    tags.extend_from_slice(&[
        (0, "ENDSEC"),
        (0, "SECTION"),
        (2, "ENTITIES"),
        (0, "ENDSEC"),
        (0, "EOF"),
    ]);
    tags.into_iter()
        .map(|(code, value)| format!("{code:>3}\n{value}\n"))
        .collect::<String>()
        .into_bytes()
}

fn addition_element(shape: Shape<ModelPoint>) -> Element<ModelPoint> {
    use zumen_drawing::{Inherit, Style};
    Element {
        source: Ref::default(),
        layer: LayerId(0),
        shape,
        style: Style {
            color: Inherit::Layer,
            line_type: Inherit::Layer,
            line_type_scale: 1.0,
            line_weight: Some(Inherit::Layer),
        },
    }
}

#[test]
fn additions_to_an_empty_modern_dxf_keep_subclass_markers() {
    let src = addition_source("AC1021", "20", &[]);
    let mut file = testkit::read::<Dxf>(&src).0.unwrap();
    let element = addition_element(newcomers().remove(0));
    assert_eq!(
        apply(&mut file, vec![Edit::Add(Box::new(element))]).0,
        Ok(())
    );
    let written = String::from_utf8(testkit::write::<Dxf>(&file).unwrap()).unwrap();
    assert!(written.contains("100\nAcDbEntity\n"));
    assert!(written.contains("100\nAcDbLine\n"));
}

#[test]
fn additions_use_only_entity_kinds_available_in_the_dxf_version() {
    for (version, index) in [("AC1009", 2), ("AC1009", 3), ("AC1012", 3)] {
        let src = addition_source(version, "20", &[]);
        let mut file = testkit::read::<Dxf>(&src).0.unwrap();
        let element = addition_element(newcomers().remove(index));
        assert_eq!(
            apply(&mut file, vec![Edit::Add(Box::new(element))]).0,
            Err(Error::Rejected),
            "{version} {index}"
        );
        assert_eq!(testkit::write::<Dxf>(&file).unwrap(), src);
    }
    for (version, index) in [("AC1012", 2), ("AC1014", 3)] {
        let src = addition_source(version, "20", &[]);
        let mut file = testkit::read::<Dxf>(&src).0.unwrap();
        let element = addition_element(newcomers().remove(index));
        assert_eq!(
            apply(&mut file, vec![Edit::Add(Box::new(element))]).0,
            Ok(()),
            "{version} {index}"
        );
    }
}

#[test]
fn additions_require_an_existing_block_even_without_block_records() {
    use zumen_drawing::{Symbol, SymbolKey};
    let blocks = [
        (0, "BLOCK"),
        (5, "30"),
        (2, "PART"),
        (70, "0"),
        (10, "0.0"),
        (20, "0.0"),
        (0, "ENDBLK"),
    ];
    for name in ["part", "MISSING"] {
        let src = addition_source("AC1009", "20", &blocks);
        let mut file = testkit::read::<Dxf>(&src).0.unwrap();
        let element = addition_element(Shape::Symbol(Symbol {
            key: SymbolKey {
                namespace: "dxf.block",
                name: name.to_owned(),
            },
            position: at(0.0, 0.0),
            scale: [1.0, 1.0],
            rotation: 0.0,
            grid: None,
            attributes: vec![],
            clip: None,
        }));
        let (result, _) = apply(&mut file, vec![Edit::Add(Box::new(element))]);
        if name == "part" {
            assert_eq!(result, Ok(()));
            let drawing = lift(&file);
            let Shape::Symbol(symbol) = &drawing.spaces[0].elements[0].shape else {
                panic!();
            };
            assert_eq!(symbol.key.name, "PART");
        } else {
            assert_eq!(result, Err(Error::Rejected));
            assert_eq!(testkit::write::<Dxf>(&file).unwrap(), src);
        }
    }
}

#[test]
fn additions_reject_exhausted_handles_before_changing_the_file() {
    for (seed, count) in [("FFFFFFFFFFFFFFFF", 1), ("FFFFFFFFFFFFFFFE", 2)] {
        let src = addition_source("AC1021", seed, &[]);
        let mut file = testkit::read::<Dxf>(&src).0.unwrap();
        let element = addition_element(newcomers().remove(0));
        let edits = (0..count)
            .map(|_| Edit::Add(Box::new(element.clone())))
            .collect();
        assert_eq!(apply(&mut file, edits).0, Err(Error::Rejected), "{seed}");
        assert_eq!(testkit::write::<Dxf>(&file).unwrap(), src);
    }
}

#[test]
fn additions_can_use_the_last_handle_with_a_following_seed() {
    let src = addition_source("AC1021", "FFFFFFFFFFFFFFFE", &[]);
    let mut file = testkit::read::<Dxf>(&src).0.unwrap();
    let element = addition_element(newcomers().remove(0));
    assert_eq!(
        apply(&mut file, vec![Edit::Add(Box::new(element.clone()))]).0,
        Ok(())
    );
    let written = testkit::write::<Dxf>(&file).unwrap();
    assert!(String::from_utf8_lossy(&written).contains("$HANDSEED\n  5\nFFFFFFFFFFFFFFFF\n"));
    assert_eq!(
        apply(&mut file, vec![Edit::Add(Box::new(element))]).0,
        Err(Error::Rejected)
    );
    assert_eq!(testkit::write::<Dxf>(&file).unwrap(), written);
}

#[test]
fn additions_stop_before_visiting_vertices_over_the_work_budget() {
    use zumen_base::{Kind, Limit, Limits};
    use zumen_drawing::{Polyline, Vertex};
    let src = addition_source("AC1021", "20", &[]);
    let line = addition_element(newcomers().remove(0));
    let mut file = testkit::read::<Dxf>(&src).0.unwrap();
    let mut control = Budget::default();
    let mut diag = Diagnostics::new();
    let cancel = CancelToken::new();
    file.apply(
        &Changes {
            edits: vec![Edit::Add(Box::new(line.clone()))],
        },
        &mut Cx::new(&mut control, &mut diag, &cancel),
    )
    .unwrap();
    let mut polyline = line;
    polyline.shape = Shape::Polyline(Polyline {
        vertices: (0..100)
            .map(|x| Vertex {
                point: at(f64::from(x), 0.0),
                bulge: 0.0,
            })
            .collect(),
        closed: false,
        width: None,
        vertex_widths: vec![],
    });
    let mut file = testkit::read::<Dxf>(&src).0.unwrap();
    let mut budget = Budget::new(
        Limits::default().with(Kind::Work, Limit::Finite(control.used(Kind::Work) + 10)),
    );
    let mut diag = Diagnostics::new();
    let result = file.apply(
        &Changes {
            edits: vec![Edit::Add(Box::new(polyline))],
        },
        &mut Cx::new(&mut budget, &mut diag, &cancel),
    );
    assert!(matches!(result, Err(Error::Stopped(_))));
    assert_eq!(testkit::write::<Dxf>(&file).unwrap(), src);
}

#[test]
fn mtext_line_spacing_must_fit_the_dxf_range() {
    let src = mtexts("AC1021", "abc");
    for (factor, supported) in [(0.249, false), (0.25, true), (4.0, true), (4.001, false)] {
        let mut file = testkit::read::<Dxf>(&src).0.unwrap();
        let mut element = elements(&lift(&file)).remove(0);
        let Shape::Formatted(text) = &mut element.shape else {
            panic!("{:?}", element.shape);
        };
        text.line_spacing.factor = factor;
        let (result, found) = apply(&mut file, vec![Edit::Replace(Box::new(element))]);
        if supported {
            assert_eq!(result, Ok(()), "{factor}");
            assert_eq!(found, []);
        } else {
            assert_eq!(result, Err(Error::Rejected), "{factor}");
            assert_eq!(found[0].0, codes::CHANGE_EDIT_UNSUPPORTED);
            assert_eq!(testkit::write::<Dxf>(&file).unwrap(), src);
        }
    }
}

#[test]
fn mtext_direction_changes_win_over_an_old_angle_and_fill_missing_components() {
    use zumen_drawing::Rotation;
    // MTEXT は、角度（50）と方向（11）のうち、後の指定が優先される（DXF Reference「MTEXT」）。
    for with_y in [true, false] {
        let mut tags = vec![
            (0, "MTEXT"),
            (5, "1A"),
            (8, "0"),
            (10, "0.0"),
            (20, "0.0"),
            (40, "2.5"),
            (1, "A"),
            (11, "1.0"),
        ];
        if with_y {
            tags.push((21, "0.0"));
        }
        tags.extend([(31, "0.0"), (50, "0.0")]);
        let src = versioned("AC1021", &tags);
        let mut file = testkit::read::<Dxf>(&src).0.unwrap();
        let mut element = elements(&lift(&file)).remove(0);
        let Shape::Formatted(text) = &mut element.shape else {
            panic!("{:?}", element.shape);
        };
        text.rotation = Rotation::Direction([0.0, 1.0]);
        let (result, found) = apply(&mut file, vec![Edit::Replace(Box::new(element))]);
        assert_eq!(result, Ok(()), "{with_y}");
        assert_eq!(found, []);
        let written = testkit::write::<Dxf>(&file).unwrap();
        let file = testkit::read::<Dxf>(&written).0.unwrap();
        let record = file
            .records()
            .iter()
            .find(|r| file.kind(r) == b"MTEXT")
            .unwrap();
        let last = record
            .tags()
            .iter()
            .rev()
            .find(|tag| matches!(tag.code(), 11 | 50))
            .unwrap();
        if last.code() == 50 {
            assert_eq!(file.real(record, 50), Ok(core::f64::consts::FRAC_PI_2));
        } else {
            assert_eq!(file.real(record, 11), Ok(0.0));
            assert_eq!(file.real(record, 21), Ok(1.0));
        }
    }
}

#[test]
fn binary_mtext_content_keeps_caret_escapes_across_chunk_boundaries() {
    let nul = |value: &str| [value.as_bytes(), b"\0"].concat();
    let tags = [
        (0i16, nul("SECTION")),
        (2, nul("HEADER")),
        (9, nul("$INSUNITS")),
        (70, 4i16.to_le_bytes().to_vec()),
        (0, nul("ENDSEC")),
        (0, nul("SECTION")),
        (2, nul("ENTITIES")),
        (0, nul("MTEXT")),
        (5, nul("1A")),
        (8, nul("0")),
        (10, 0.0f64.to_le_bytes().to_vec()),
        (20, 0.0f64.to_le_bytes().to_vec()),
        (40, 2.5f64.to_le_bytes().to_vec()),
        (1, nul("A")),
        (0, nul("ENDSEC")),
        (0, nul("EOF")),
    ];
    let mut src = b"AutoCAD Binary DXF\r\n\x1a\0".to_vec();
    for (code, value) in tags {
        src.extend_from_slice(&code.to_le_bytes());
        src.extend(value);
    }
    let mut file = testkit::read::<Dxf>(&src).0.unwrap();
    let mut element = elements(&lift(&file)).remove(0);
    let Shape::Formatted(text) = &mut element.shape else {
        panic!("{:?}", element.shape);
    };
    text.content = format!("{}^ B", "a".repeat(249));
    text.spans = spans(&text.content);
    let expected = text.content.clone();
    let (result, found) = apply(&mut file, vec![Edit::Replace(Box::new(element))]);
    assert_eq!(result, Ok(()));
    assert_eq!(found, []);
    let written = testkit::write::<Dxf>(&file).unwrap();
    let file = testkit::read::<Dxf>(&written).0.unwrap();
    let Shape::Formatted(text) = &elements(&lift(&file))[0].shape else {
        panic!("not MTEXT");
    };
    assert_eq!(text.content, expected);
}

#[test]
fn sheet_additions_need_a_sheet_from_successfully_lifted_elements() {
    use zumen_drawing::{PaperPoint, SheetId};
    let src = versioned(
        "AC1015",
        &[
            (0, "LINE"),
            (5, "1A"),
            (8, "0"),
            (10, "0.0"),
            (20, "0.0"),
            (11, "1.0"),
            (21, "0.0"),
            (0, "CIRCLE"),
            (5, "1B"),
            (8, "0"),
            (67, "1"),
            (10, "0.0"),
            (20, "0.0"),
            (40, "0.0"),
        ],
    );
    let mut file = testkit::read::<Dxf>(&src).0.unwrap();
    let drawing = lift(&file);
    assert!(drawing.sheets.is_empty());
    let template = &drawing.spaces[0].elements[0];
    let element = Element {
        source: template.source.clone(),
        layer: template.layer,
        style: template.style,
        shape: Shape::Line {
            start: PaperPoint {
                sheet: SheetId(0),
                x: 0.0,
                y: 0.0,
            },
            end: PaperPoint {
                sheet: SheetId(0),
                x: 1.0,
                y: 0.0,
            },
        },
    };
    assert_eq!(
        apply(&mut file, vec![Edit::AddOnSheet(Box::new(element))]).0,
        Err(Error::Rejected)
    );
    assert_eq!(testkit::write::<Dxf>(&file).unwrap(), src);
}

#[test]
fn mtext_additions_need_the_supported_version_and_spacing() {
    for (version, spacing, supported) in [
        ("AC1009", 1.0, false),
        ("AC1021", 0.249, false),
        ("AC1021", 4.001, false),
        ("AC1012", 0.25, true),
        ("AC1021", 4.0, true),
    ] {
        let src = addition_source(version, "20", &[]);
        let mut file = testkit::read::<Dxf>(&src).0.unwrap();
        let mut shape = newcomers().remove(5);
        if let Shape::Formatted(text) = &mut shape {
            text.line_spacing.factor = spacing;
        }
        let result = apply(
            &mut file,
            vec![Edit::Add(Box::new(addition_element(shape)))],
        )
        .0;
        if supported {
            assert_eq!(result, Ok(()), "{version} {spacing}");
        } else {
            assert_eq!(result, Err(Error::Rejected), "{version} {spacing}");
            assert_eq!(testkit::write::<Dxf>(&file).unwrap(), src);
        }
    }
}

#[test]
fn mtext_additions_count_content_before_parsing_or_copying() {
    use zumen_base::{Kind, Limit, Limits};
    let src = addition_source("AC1021", "20", &[]);
    let mut file = testkit::read::<Dxf>(&src).0.unwrap();
    let mut control = Budget::default();
    let mut diag = Diagnostics::new();
    let cancel = CancelToken::new();
    file.apply(
        &Changes {
            edits: vec![Edit::Add(Box::new(addition_element(newcomers().remove(0))))],
        },
        &mut Cx::new(&mut control, &mut diag, &cancel),
    )
    .unwrap();
    let mut shape = newcomers().remove(5);
    if let Shape::Formatted(text) = &mut shape {
        text.content = "x".repeat(1000);
        text.spans = spans(&text.content);
    }
    let mut file = testkit::read::<Dxf>(&src).0.unwrap();
    let mut budget = Budget::new(
        Limits::default().with(Kind::Work, Limit::Finite(control.used(Kind::Work) + 20)),
    );
    let result = file.apply(
        &Changes {
            edits: vec![Edit::Add(Box::new(addition_element(shape)))],
        },
        &mut Cx::new(&mut budget, &mut diag, &cancel),
    );
    assert!(matches!(result, Err(Error::Stopped(_))));
    assert_eq!(testkit::write::<Dxf>(&file).unwrap(), src);
}

#[test]
fn block_sheet_additions_count_insertion_work_before_changing_the_file() {
    use zumen_base::{Kind, Limit, Limits};
    let blocks = [
        (0, "BLOCK"),
        (5, "C2"),
        (330, "C1"),
        (2, "*Paper_Space0"),
        (70, "0"),
        (10, "0.0"),
        (20, "0.0"),
        (0, "LINE"),
        (5, "C3"),
        (330, "C1"),
        (67, "1"),
        (8, "0"),
        (10, "0.0"),
        (20, "0.0"),
        (11, "1.0"),
        (21, "1.0"),
        (0, "ENDBLK"),
        (5, "C4"),
        (330, "C1"),
    ];
    let src = String::from_utf8(addition_source("AC1021", "1000", &blocks)).unwrap().replacen(
        "  0\nENDTAB\n  0\nENDSEC\n", "  0\nENDTAB\n  0\nTABLE\n  2\nBLOCK_RECORD\n  0\nBLOCK_RECORD\n  5\nC1\n  2\n*Paper_Space0\n  0\nENDTAB\n  0\nENDSEC\n", 1).into_bytes();
    let mut file = testkit::read::<Dxf>(&src).0.unwrap();
    let mut control = Budget::default();
    let mut diag = Diagnostics::new();
    let cancel = CancelToken::new();
    file.apply(
        &Changes { edits: vec![] },
        &mut Cx::new(&mut control, &mut diag, &cancel),
    )
    .unwrap();
    let element = lift(&file).sheets[0].elements[0].clone();
    let mut budget = Budget::new(
        Limits::default().with(Kind::Work, Limit::Finite(control.used(Kind::Work) + 5)),
    );
    let result = file.apply(
        &Changes {
            edits: vec![Edit::AddOnSheet(Box::new(element))],
        },
        &mut Cx::new(&mut budget, &mut diag, &cancel),
    );
    assert!(matches!(result, Err(Error::Stopped(_))));
    assert_eq!(testkit::write::<Dxf>(&file).unwrap(), src);
}

#[cfg(test)]
fn attributed_newcomer() -> Shape<ModelPoint> {
    use zumen_drawing::{Attribute, HeightMeaning, Symbol, SymbolKey};
    let Shape::Text(mut text) = newcomers().remove(4) else {
        panic!();
    };
    text.height.meaning = HeightMeaning::Declared("dxf.ATTRIB.40");
    Shape::Symbol(Symbol {
        key: SymbolKey {
            namespace: "dxf.block",
            name: "PART".to_owned(),
        },
        position: at(0.0, 0.0),
        scale: [1.0, 1.0],
        rotation: 0.0,
        grid: None,
        clip: None,
        attributes: vec![Attribute {
            tag: "A".to_owned(),
            text,
            formatted: None,
            hidden: false,
            layer: LayerId(0),
            style: addition_element(newcomers().remove(0)).style,
            source: Ref::default(),
        }],
    })
}

#[cfg(test)]
fn attributed_addition_source(seed: &str) -> Vec<u8> {
    addition_source(
        "AC1021",
        seed,
        &[
            (0, "BLOCK"),
            (5, "30"),
            (2, "PART"),
            (70, "0"),
            (10, "0.0"),
            (20, "0.0"),
            (0, "ENDBLK"),
        ],
    )
}

#[test]
fn attributed_additions_reserve_followers_and_the_next_seed() {
    for (seed, supported) in [("FFFFFFFFFFFFFFFD", false), ("FFFFFFFFFFFFFFFC", true)] {
        let src = attributed_addition_source(seed);
        let mut file = testkit::read::<Dxf>(&src).0.unwrap();
        let result = apply(
            &mut file,
            vec![Edit::Add(Box::new(addition_element(attributed_newcomer())))],
        )
        .0;
        if supported {
            assert_eq!(result, Ok(()));
            assert!(
                String::from_utf8_lossy(&testkit::write::<Dxf>(&file).unwrap())
                    .contains("$HANDSEED\n  5\nFFFFFFFFFFFFFFFF\n")
            );
        } else {
            assert_eq!(result, Err(Error::Rejected));
            assert_eq!(testkit::write::<Dxf>(&file).unwrap(), src);
        }
    }
    let src = attributed_addition_source("FFFFFFFFFFFFFFFC");
    let mut file = testkit::read::<Dxf>(&src).0.unwrap();
    let edits = vec![
        Edit::Add(Box::new(addition_element(attributed_newcomer()))),
        Edit::Add(Box::new(addition_element(newcomers().remove(0)))),
    ];
    assert_eq!(apply(&mut file, edits).0, Err(Error::Rejected));
    assert_eq!(testkit::write::<Dxf>(&file).unwrap(), src);
}

#[test]
fn attributed_additions_count_attribute_content_before_copying() {
    use zumen_base::{Kind, Limit, Limits};
    let src = attributed_addition_source("100");
    let mut file = testkit::read::<Dxf>(&src).0.unwrap();
    let mut control = Budget::default();
    let mut diag = Diagnostics::new();
    let cancel = CancelToken::new();
    file.apply(
        &Changes {
            edits: vec![Edit::Add(Box::new(addition_element(newcomers().remove(0))))],
        },
        &mut Cx::new(&mut control, &mut diag, &cancel),
    )
    .unwrap();
    let mut shape = attributed_newcomer();
    if let Shape::Symbol(symbol) = &mut shape {
        symbol.attributes[0].text.content = "x".repeat(100);
    }
    let mut file = testkit::read::<Dxf>(&src).0.unwrap();
    let mut budget = Budget::new(
        Limits::default().with(Kind::Work, Limit::Finite(control.used(Kind::Work) + 20)),
    );
    let result = file.apply(
        &Changes {
            edits: vec![Edit::Add(Box::new(addition_element(shape)))],
        },
        &mut Cx::new(&mut budget, &mut diag, &cancel),
    );
    assert!(matches!(result, Err(Error::Stopped(_))));
    assert_eq!(testkit::write::<Dxf>(&file).unwrap(), src);
}

/// 空の LAYOUT。現在の用紙は見出しの原点、ほかの用紙は LAYOUT の原点を使う。
#[cfg(test)]
fn empty_sheet_source(current: bool, units: &str, viewport: bool) -> Vec<u8> {
    let name = if current {
        "*Paper_Space"
    } else {
        "*Paper_Space0"
    };
    let mut tags = vec![
        (0, "SECTION"),
        (2, "HEADER"),
        (9, "$ACADVER"),
        (1, "AC1021"),
        (9, "$INSUNITS"),
        (70, units),
        (9, "$HANDSEED"),
        (5, "1000"),
        (9, "$PLIMMIN"),
        (10, "2.0"),
        (20, "3.0"),
        (0, "ENDSEC"),
        (0, "SECTION"),
        (2, "TABLES"),
        (0, "TABLE"),
        (2, "LAYER"),
        (0, "LAYER"),
        (5, "10"),
        (2, "0"),
        (70, "0"),
        (62, "7"),
        (0, "ENDTAB"),
        (0, "TABLE"),
        (2, "BLOCK_RECORD"),
        (0, "BLOCK_RECORD"),
        (5, "C1"),
        (2, name),
        (0, "ENDTAB"),
        (0, "ENDSEC"),
        (0, "SECTION"),
        (2, "BLOCKS"),
        (0, "BLOCK"),
        (5, "C2"),
        (330, "C1"),
        (2, name),
        (70, "0"),
        (10, "0.0"),
        (20, "0.0"),
    ];
    let view = [
        (0, "VIEWPORT"),
        (5, "C3"),
        (330, "C1"),
        (8, "0"),
        (67, "1"),
        (69, "2"),
        (10, "5.0"),
        (20, "5.0"),
        (40, "10.0"),
        (41, "10.0"),
        (45, "100.0"),
    ];
    if viewport && !current {
        tags.extend(view);
    }
    tags.extend([
        (0, "ENDBLK"),
        (5, "C4"),
        (330, "C1"),
        (0, "ENDSEC"),
        (0, "SECTION"),
        (2, "ENTITIES"),
    ]);
    if viewport && current {
        tags.extend(view);
    }
    tags.extend([
        (0, "ENDSEC"),
        (0, "SECTION"),
        (2, "OBJECTS"),
        (0, "LAYOUT"),
        (5, "D1"),
        (330, "1A"),
        (100, "AcDbPlotSettings"),
        (1, ""),
        (100, "AcDbLayout"),
        (1, "Paper"),
        (71, "1"),
        (10, "7.0"),
        (20, "11.0"),
        (11, "17.0"),
        (21, "21.0"),
        (330, "C1"),
        (0, "ENDSEC"),
        (0, "EOF"),
    ]);
    tags.into_iter()
        .map(|(code, value)| format!("{code:>3}\n{value}\n"))
        .collect::<String>()
        .into_bytes()
}

#[test]
fn sheet_additions_use_the_layout_origin_and_units_without_elements() {
    use zumen_drawing::PaperPoint;
    for (current, viewport, units) in [
        (true, false, "1"),
        (false, false, "1"),
        (true, true, "1"),
        (false, true, "1"),
        (true, false, "0"),
        (false, false, "0"),
    ] {
        let src = empty_sheet_source(current, units, viewport);
        let mut file = testkit::read::<Dxf>(&src).0.unwrap();
        let before = lift(&file);
        let sheet = &before.sheets[0];
        assert!(sheet.elements.is_empty());
        assert_eq!(sheet.views.len(), usize::from(viewport && units != "0"));
        let template = addition_element(newcomers().remove(0));
        let shape = |sheet| Shape::Line {
            start: PaperPoint {
                sheet,
                x: 5.0,
                y: 10.0,
            },
            end: PaperPoint {
                sheet,
                x: 50.0,
                y: 30.0,
            },
        };
        let element = Element {
            source: Ref::default(),
            layer: template.layer,
            style: template.style,
            shape: shape(sheet.id),
        };
        let result = apply(&mut file, vec![Edit::AddOnSheet(Box::new(element))]).0;
        if units == "0" {
            assert_eq!(result, Err(Error::Rejected));
            assert_eq!(testkit::write::<Dxf>(&file).unwrap(), src);
            continue;
        }
        assert_eq!(result, Ok(()), "current={current} viewport={viewport}");
        let written = testkit::write::<Dxf>(&file).unwrap();
        let after = lift(&testkit::read::<Dxf>(&written).0.unwrap());
        let found = after
            .sheets
            .iter()
            .find(|sheet| sheet.name.as_deref() == Some("Paper"))
            .unwrap();
        assert_eq!(found.elements.len(), 1);
        let Shape::Line { start, end } = &found.elements[0].shape else {
            panic!()
        };
        let Shape::Line {
            start: expected_start,
            end: expected_end,
        } = shape(found.id)
        else {
            panic!()
        };
        for (found, expected) in [
            (start.x, expected_start.x),
            (start.y, expected_start.y),
            (end.x, expected_end.x),
            (end.y, expected_end.y),
        ] {
            assert!((found - expected).abs() < 1e-10, "{found} != {expected}");
        }
    }
}

/// 実寸の空間の要素の ID と、今のハッシュ
#[cfg(test)]
fn seen(drawing: &Drawing2D) -> Vec<(zumen_drawing::Seen, Element<ModelPoint>)> {
    use zumen_drawing::{Hash, Seen, id};
    let space = drawing.spaces.first().unwrap();
    let place = id::space(space.id);
    let mut namer = id::Namer::new(&place);
    space
        .elements
        .iter()
        .enumerate()
        .map(|(index, element)| {
            let seen = Seen {
                id: namer.next(index, element),
                hash: Hash::element(element),
            };
            (seen, element.clone())
        })
        .collect()
}

#[test]
fn changes_check_ids_element_hashes_and_the_document_hash_before_applying() {
    use zumen_drawing::{Hash, Placed, Seen};
    let src = src();
    let mut file = testkit::read::<Dxf>(&src).0.unwrap();
    let [(line, _), (circle, _), (arc, arc_element)] =
        <[_; 3]>::try_from(seen(&lift(&file))).unwrap();
    assert_eq!(
        [line.id.as_str(), circle.id.as_str(), arc.id.as_str()],
        ["space/0/h1A", "space/0/h1B", "space/0/h1C"]
    );
    let changes = zumen_drawing::Changes {
        edits: vec![
            zumen_drawing::Edit::Remove {
                target: Seen {
                    id: line.id.clone(),
                    hash: Hash([0; 16]),
                },
            },
            zumen_drawing::Edit::Remove {
                target: Seen {
                    id: "space/0/h99".into(),
                    hash: circle.hash,
                },
            },
            zumen_drawing::Edit::Add {
                place: "sheet/h5".into(),
                element: Box::new(Placed::Model(arc_element)),
            },
            zumen_drawing::Edit::Remove { target: arc },
        ],
        document: Some(Hash::document(b"another file")),
    };
    let mut budget = Budget::default();
    let mut diag = Diagnostics::new();
    let result = zumen_sdk::Apply::apply(
        &mut file,
        &changes,
        &mut Cx::new(&mut budget, &mut diag, &CancelToken::new()),
    );
    assert_eq!(result, Err(Error::Rejected));
    // 最初の 1 つで止めずに、すべての変更を確かめる
    let found: Vec<_> = diag
        .iter()
        .map(|d| (d.code(), format!("{:?}", d.params())))
        .collect();
    assert_eq!(
        found.iter().map(|(code, _)| *code).collect::<Vec<_>>(),
        [
            codes::CHANGE_DOCUMENT_STALE,
            codes::CHANGE_ELEMENT_STALE,
            codes::CHANGE_ELEMENT_MISSING,
            codes::CHANGE_ELEMENT_MISSING,
        ]
    );
    assert!(found[1].1.contains("space/0/h1A"));
    assert!(found[2].1.contains("space/0/h99"));
    assert!(found[3].1.contains("sheet/h5"));
    assert_eq!(testkit::write::<Dxf>(&file).unwrap(), src);
}

#[test]
fn applied_changes_return_the_ids_and_hashes_that_reading_again_gives() {
    use zumen_drawing::{Hash, Placed};
    let src = src();
    let mut file = testkit::read::<Dxf>(&src).0.unwrap();
    let [(line, mut moved), (circle, _), (arc, _)] =
        <[_; 3]>::try_from(seen(&lift(&file))).unwrap();
    if let Shape::Line { end, .. } = &mut moved.shape {
        end.x = 3.5;
    }
    let mut added = moved.clone();
    added.source = Ref::default();
    let changes = zumen_drawing::Changes {
        edits: vec![
            zumen_drawing::Edit::Replace {
                target: line.clone(),
                element: Box::new(Placed::Model(moved.clone())),
            },
            zumen_drawing::Edit::Add {
                place: "space/0".into(),
                element: Box::new(Placed::Model(added)),
            },
            zumen_drawing::Edit::Remove { target: arc },
        ],
        // 読んだバイト列のハッシュ。変えていないファイルは元のバイトのまま書くので、今の文書と同じ
        document: Some(Hash::document(&src)),
    };
    let mut budget = Budget::default();
    let mut diag = Diagnostics::new();
    let applied = zumen_sdk::Apply::apply(
        &mut file,
        &changes,
        &mut Cx::new(&mut budget, &mut diag, &CancelToken::new()),
    )
    .unwrap();
    assert_eq!(diag.iter().count(), 0);

    let written = testkit::write::<Dxf>(&file).unwrap();
    let again = seen(&lift(&testkit::read::<Dxf>(&written).0.unwrap()));
    let ids: Vec<_> = again.iter().map(|(seen, _)| seen.id.as_str()).collect();
    // 足した図形のハンドルは、$HANDSEED（20）から振る
    assert_eq!(ids, ["space/0/h1A", "space/0/h1B", "space/0/h20"]);
    assert_eq!(applied.len(), 3);
    assert_eq!(applied[0].id.as_deref(), Some("space/0/h1A"));
    assert_eq!(applied[0].hash, Some(again[0].0.hash));
    assert_ne!(applied[0].hash, Some(line.hash));
    assert_eq!(applied[1].id.as_deref(), Some("space/0/h20"));
    assert_eq!(applied[1].hash, Some(again[2].0.hash));
    assert_eq!(
        applied[2],
        zumen_drawing::Applied {
            id: None,
            hash: None
        }
    );
    // 触れていない要素のハッシュは変わらない
    assert_eq!(again[1].0, circle);
}

#[test]
fn multiple_changes_to_one_element_are_rejected_without_writing() {
    use zumen_drawing::{Edit as Change, Placed};
    let src = src();
    let mut file = testkit::read::<Dxf>(&src).0.unwrap();
    let (line, mut moved) = seen(&lift(&file)).remove(0);
    if let Shape::Line { end, .. } = &mut moved.shape {
        end.x = 3.5;
    }
    let changes = zumen_drawing::Changes {
        edits: vec![
            Change::Replace {
                target: line.clone(),
                element: Box::new(Placed::Model(moved)),
            },
            Change::Remove { target: line },
        ],
        document: None,
    };
    let mut budget = Budget::default();
    let mut diag = Diagnostics::new();
    let result = zumen_sdk::Apply::apply(
        &mut file,
        &changes,
        &mut Cx::new(&mut budget, &mut diag, &CancelToken::new()),
    );
    assert_eq!(result, Err(Error::Rejected));
    assert!(
        diag.iter()
            .any(|d| d.code() == codes::CHANGE_EDIT_UNSUPPORTED)
    );
    assert_eq!(testkit::write::<Dxf>(&file).unwrap(), src);
}

#[test]
fn duplicate_entity_handles_can_remove_the_wrong_entity() {
    use zumen_drawing::Edit as Change;
    let src = versioned(
        "AC1021",
        &[
            (0, "LINE"),
            (5, "1A"),
            (8, "0"),
            (10, "0"),
            (20, "0"),
            (11, "1"),
            (21, "1"),
            (1001, "APP"),
            (1000, "first"),
            (0, "LINE"),
            (5, "1A"),
            (8, "0"),
            (10, "0"),
            (20, "0"),
            (11, "1"),
            (21, "1"),
            (1001, "APP"),
            (1000, "second"),
        ],
    );
    let mut file = testkit::read::<Dxf>(&src).0.unwrap();
    let entities = seen(&lift(&file));
    assert_eq!(entities.len(), 2);
    assert_eq!(entities[0].0.hash, entities[1].0.hash);
    let changes = zumen_drawing::Changes {
        edits: vec![Change::Remove {
            target: entities[0].0.clone(),
        }],
        document: None,
    };
    let mut budget = Budget::default();
    let mut diag = Diagnostics::new();
    let result = zumen_sdk::Apply::apply(
        &mut file,
        &changes,
        &mut Cx::new(&mut budget, &mut diag, &CancelToken::new()),
    );
    assert_eq!(result, Err(Error::Rejected));
    assert!(
        diag.iter()
            .any(|d| d.code() == codes::CHANGE_EDIT_UNSUPPORTED)
    );
    assert_eq!(testkit::write::<Dxf>(&file).unwrap(), src);
}

#[test]
fn duplicate_handles_in_definitions_and_tables_reject_apply() {
    let record = |kind: &str, name: &str| format!("0\n{kind}\n5\n40\n2\n{name}\n70\n0\n");
    for (section, first, second) in [
        ("TABLES", record("LAYER", "A"), record("LAYER", "B")),
        ("TABLES", record("LTYPE", "A"), record("LTYPE", "B")),
        ("BLOCKS", record("BLOCK", "A"), record("BLOCK", "B")),
        ("OBJECTS", record("LAYOUT", "A"), record("LAYOUT", "B")),
    ] {
        let src =
            format!("0\nSECTION\n2\n{section}\n{first}{second}0\nENDSEC\n0\nEOF\n").into_bytes();
        let mut file = testkit::read::<Dxf>(&src).0.unwrap();
        let mut budget = Budget::default();
        let mut diag = Diagnostics::new();
        let result = zumen_sdk::Apply::apply(
            &mut file,
            &zumen_drawing::Changes {
                edits: vec![],
                document: None,
            },
            &mut Cx::new(&mut budget, &mut diag, &CancelToken::new()),
        );
        assert_eq!(result, Err(Error::Rejected), "{section}");
        assert!(
            diag.iter()
                .any(|d| d.code() == codes::CHANGE_EDIT_UNSUPPORTED)
        );
        assert_eq!(testkit::write::<Dxf>(&file).unwrap(), src);
    }
}

/// 2 つのレイヤと 2 つの線種を持ち、区切り（100）と XDATA を持つ LINE と、区切りを持たない CIRCLE を持つ
/// R2004 の DXF
#[cfg(test)]
fn styled() -> Vec<u8> {
    let tags: &[(i16, &str)] = &[
        (0, "SECTION"),
        (2, "HEADER"),
        (9, "$ACADVER"),
        (1, "AC1018"),
        (9, "$INSUNITS"),
        (70, "4"),
        (9, "$HANDSEED"),
        (5, "40"),
        (0, "ENDSEC"),
        (0, "SECTION"),
        (2, "TABLES"),
        (0, "TABLE"),
        (2, "LTYPE"),
        (0, "LTYPE"),
        (5, "11"),
        (2, "CONTINUOUS"),
        (70, "0"),
        (0, "LTYPE"),
        (5, "12"),
        (2, "Dashed"),
        (70, "0"),
        (0, "ENDTAB"),
        (0, "TABLE"),
        (2, "LAYER"),
        (0, "LAYER"),
        (5, "10"),
        (2, "0"),
        (70, "0"),
        (62, "7"),
        (6, "CONTINUOUS"),
        (0, "LAYER"),
        (5, "13"),
        (2, "Wall"),
        (70, "0"),
        (62, "1"),
        (6, "CONTINUOUS"),
        (0, "ENDTAB"),
        (0, "ENDSEC"),
        (0, "SECTION"),
        (2, "ENTITIES"),
        (0, "LINE"),
        (5, "1A"),
        (100, "AcDbEntity"),
        (8, "0"),
        (62, "5"),
        (100, "AcDbLine"),
        (10, "0.0"),
        (20, "0.0"),
        (30, "0.0"),
        (11, "1.0"),
        (21, "1.0"),
        (31, "0.0"),
        (1001, "ZUMEN"),
        (1000, "kept"),
        (0, "CIRCLE"),
        (5, "1B"),
        (8, "0"),
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

#[test]
fn restyles_change_only_the_named_style_fields_and_keep_the_shape() {
    use zumen_drawing::{Color, ColorCode, ColorSystem, Inherit, LineWeight, Restyle, Seen};
    let mut file = testkit::read::<Dxf>(&styled()).0.unwrap();
    let drawing = lift(&file);
    let wall = drawing
        .layers
        .iter()
        .position(|layer| layer.name == "Wall")
        .map(|index| LayerId(index as u32))
        .unwrap();
    let dashed = drawing
        .line_types
        .iter()
        .position(|line_type| line_type.name == "Dashed")
        .map(|index| zumen_drawing::LineTypeId(index as u32))
        .unwrap();
    let [(line, line_before), (circle, circle_before)] =
        <[_; 2]>::try_from(seen(&drawing)).unwrap();
    let red = Color {
        code: Some(ColorCode {
            system: ColorSystem::Aci,
            value: 1,
        }),
        rgb: Some([255, 0, 0]),
    };
    let restyle = |target: Seen, style: Restyle| zumen_drawing::Edit::Restyle { target, style };
    let changes = zumen_drawing::Changes {
        edits: vec![
            restyle(
                line.clone(),
                Restyle {
                    layer: Some(wall),
                    color: Some(Inherit::Layer),
                    line_type: Some(Inherit::Own(dashed)),
                    line_weight: Some(Inherit::Own(LineWeight::Width(zumen_drawing::PaperLength(
                        0.35,
                    )))),
                    ..Restyle::default()
                },
            ),
            restyle(
                circle.clone(),
                Restyle {
                    color: Some(Inherit::Own(red)),
                    line_type_scale: Some(2.5),
                    ..Restyle::default()
                },
            ),
        ],
        document: None,
    };
    let mut budget = Budget::default();
    let mut diag = Diagnostics::new();
    let applied = zumen_sdk::Apply::apply(
        &mut file,
        &changes,
        &mut Cx::new(&mut budget, &mut diag, &CancelToken::new()),
    )
    .unwrap();
    assert_eq!(diag.iter().count(), 0);

    let written = testkit::write::<Dxf>(&file).unwrap();
    let again = seen(&lift(&testkit::read::<Dxf>(&written).0.unwrap()));
    let [(line_id, line_after), (circle_id, circle_after)] = <[_; 2]>::try_from(again).unwrap();
    assert_eq!(line_after.shape, line_before.shape);
    assert_eq!(line_after.layer, wall);
    assert_eq!(line_after.style.color, Inherit::Layer);
    assert_eq!(line_after.style.line_type, Inherit::Own(dashed));
    assert_eq!(
        line_after.style.line_weight,
        Some(Inherit::Own(LineWeight::Width(zumen_drawing::PaperLength(
            0.35
        ))))
    );
    assert_eq!(circle_after.shape, circle_before.shape);
    assert_eq!(circle_after.layer, circle_before.layer);
    assert_eq!(circle_after.style.color, Inherit::Own(red));
    assert_eq!(circle_after.style.line_type_scale, 2.5);
    // ID は変わらず、返すハッシュは読み直したときと同じ
    assert_eq!(applied[0].id.as_deref(), Some(line.id.as_str()));
    assert_eq!(applied[0].hash, Some(line_id.hash));
    assert_eq!(applied[1].hash, Some(circle_id.hash));
    assert_eq!(circle_id.id, circle.id);
    // 変えた欄だけを AcDbEntity の区切りの中に書き、XDATA と形の値は元の書き方のまま残す
    let text = String::from_utf8(written).unwrap();
    assert!(text.contains(
        "  0\nLINE\n  5\n1A\n100\nAcDbEntity\n  8\nWall\n  6\nDashed\n370\n35\n100\nAcDbLine\n 10\n0.0\n"
    ));
    assert!(text.contains("1001\nZUMEN\n1000\nkept\n"));
    assert!(
        text.contains("  0\nCIRCLE\n  5\n1B\n  8\n0\n 62\n1\n 48\n2.5\n420\n16711680\n 10\n5.0\n")
    );
}

#[test]
fn restyles_the_records_cannot_hold_change_nothing() {
    use zumen_drawing::{Inherit, LineWeight, Restyle};
    let src = styled();
    let mut file = testkit::read::<Dxf>(&src).0.unwrap();
    let [(line, _), (circle, _)] = <[_; 2]>::try_from(seen(&lift(&file))).unwrap();
    let changes = zumen_drawing::Changes {
        edits: vec![
            // 表にないレイヤ
            zumen_drawing::Edit::Restyle {
                target: line,
                style: Restyle {
                    layer: Some(LayerId(99)),
                    ..Restyle::default()
                },
            },
            // 線幅の値の範囲の外
            zumen_drawing::Edit::Restyle {
                target: circle,
                style: Restyle {
                    line_weight: Some(Inherit::Own(LineWeight::Width(zumen_drawing::PaperLength(
                        9.99,
                    )))),
                    ..Restyle::default()
                },
            },
        ],
        document: None,
    };
    let mut budget = Budget::default();
    let mut diag = Diagnostics::new();
    let result = zumen_sdk::Apply::apply(
        &mut file,
        &changes,
        &mut Cx::new(&mut budget, &mut diag, &CancelToken::new()),
    );
    assert_eq!(result, Err(Error::Rejected));
    assert_eq!(
        diag.iter().map(|d| d.code()).collect::<Vec<_>>(),
        [codes::CHANGE_EDIT_UNSUPPORTED]
    );
    assert_eq!(testkit::write::<Dxf>(&file).unwrap(), src);
}

#[test]
fn restyle_can_change_color_on_an_undefined_existing_layer() {
    use zumen_drawing::{Color, ColorCode, ColorSystem, Inherit, Restyle};
    let src = String::from_utf8(styled())
        .unwrap()
        .replacen("  8\n0\n 62\n5\n", "  8\nGhost\n 62\n5\n", 1)
        .into_bytes();
    let mut file = testkit::read::<Dxf>(&src).0.unwrap();
    let line = seen(&lift(&file)).remove(0).0;
    let color = Color {
        code: Some(ColorCode {
            system: ColorSystem::Aci,
            value: 1,
        }),
        rgb: None,
    };
    let changes = zumen_drawing::Changes {
        edits: vec![zumen_drawing::Edit::Restyle {
            target: line,
            style: Restyle {
                color: Some(Inherit::Own(color)),
                ..Restyle::default()
            },
        }],
        document: None,
    };
    let mut budget = Budget::default();
    let mut diagnostics = Diagnostics::new();
    let cancel = CancelToken::new();
    assert!(
        zumen_sdk::Apply::apply(
            &mut file,
            &changes,
            &mut Cx::new(&mut budget, &mut diagnostics, &cancel)
        )
        .is_ok(),
        "{diagnostics:?}"
    );
    let written = testkit::write::<Dxf>(&file).unwrap();
    assert!(String::from_utf8_lossy(&written).contains("  8\nGhost\n"));
    let reloaded = testkit::read::<Dxf>(&written).0.unwrap();
    let drawing = lift(&reloaded);
    let line = &drawing.spaces[0].elements[0];
    assert_eq!(drawing.layer(line.layer).unwrap().name, "Ghost");
    assert_eq!(line.style.color, Inherit::Own(color));
}

#[test]
fn restyle_keeps_undefined_line_type_and_handles_implicit_layer() {
    use zumen_drawing::{Color, ColorCode, ColorSystem, Inherit, Restyle};
    let base = String::from_utf8(styled()).unwrap();
    let sources = [
        base.replacen("  8\n0\n 62\n5\n", "  8\n0\n  6\nGhostLine\n 62\n5\n", 1),
        base.replacen("  8\n0\n 62\n5\n", " 62\n5\n", 1),
    ];
    for source in sources {
        let mut file = testkit::read::<Dxf>(source.as_bytes()).0.unwrap();
        let before = lift(&file);
        let line = seen(&before).remove(0).0;
        let color = Color {
            code: Some(ColorCode {
                system: ColorSystem::Aci,
                value: 1,
            }),
            rgb: None,
        };
        let changes = zumen_drawing::Changes {
            edits: vec![zumen_drawing::Edit::Restyle {
                target: line,
                style: Restyle {
                    color: Some(Inherit::Own(color)),
                    ..Restyle::default()
                },
            }],
            document: None,
        };
        let mut budget = Budget::default();
        let mut diagnostics = Diagnostics::new();
        let cancel = CancelToken::new();
        zumen_sdk::Apply::apply(
            &mut file,
            &changes,
            &mut Cx::new(&mut budget, &mut diagnostics, &cancel),
        )
        .unwrap();
        let written = testkit::write::<Dxf>(&file).unwrap();
        let reloaded = lift(&testkit::read::<Dxf>(&written).0.unwrap());
        let old = &before.spaces[0].elements[0];
        let new = &reloaded.spaces[0].elements[0];
        assert_eq!(new.shape, old.shape);
        assert_eq!(new.style.color, Inherit::Own(color));
        assert_eq!(new.style.line_type, old.style.line_type);
        if source.contains("GhostLine") {
            assert!(
                String::from_utf8(written)
                    .unwrap()
                    .contains("  6\nGhostLine\n")
            );
        }
    }
}

/// ブロック B を定義し、図形とともに動く XDATA を持つ LINE、ARC、属性を 1 つ持つ INSERT を持つ DXF
#[cfg(test)]
fn movable() -> Vec<u8> {
    let tags: &[(i16, &str)] = &[
        (0, "SECTION"),
        (2, "HEADER"),
        (9, "$ACADVER"),
        (1, "AC1015"),
        (9, "$INSUNITS"),
        (70, "4"),
        (9, "$HANDSEED"),
        (5, "40"),
        (0, "ENDSEC"),
        (0, "SECTION"),
        (2, "BLOCKS"),
        (0, "BLOCK"),
        (5, "30"),
        (8, "0"),
        (2, "B"),
        (70, "2"),
        (10, "0.0"),
        (20, "0.0"),
        (30, "0.0"),
        (0, "ENDBLK"),
        (5, "31"),
        (8, "0"),
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
        (1001, "ZUMEN"),
        (1011, "1.0"),
        (1021, "1.0"),
        (1031, "0.0"),
        (1012, "1.0"),
        (1022, "0.0"),
        (1032, "0.0"),
        (1013, "1.0"),
        (1023, "0.0"),
        (1033, "0.0"),
        (1041, "2.0"),
        (1042, "3.0"),
        (0, "ARC"),
        (5, "1B"),
        (8, "0"),
        (10, "0.0"),
        (20, "0.0"),
        (30, "0.0"),
        (40, "1.0"),
        (50, "0.0"),
        (51, "90.0"),
        (0, "INSERT"),
        (5, "1C"),
        (8, "0"),
        (66, "1"),
        (2, "B"),
        (10, "0.0"),
        (20, "0.0"),
        (30, "0.0"),
        (0, "ATTRIB"),
        (5, "1D"),
        (8, "0"),
        (10, "1.0"),
        (20, "0.0"),
        (30, "0.0"),
        (40, "2.5"),
        (1, "x"),
        (2, "A"),
        (70, "0"),
        (0, "SEQEND"),
        (5, "1E"),
        (8, "0"),
        (0, "ENDSEC"),
        (0, "EOF"),
    ];
    tags.iter()
        .map(|(code, value)| format!("{code:>3}\n{value}\n"))
        .collect::<String>()
        .into_bytes()
}

#[test]
fn transforms_move_shapes_attributes_and_the_xdata_that_moves_with_them() {
    use zumen_drawing::{Affine, transform::transform};
    let mut file = testkit::read::<Dxf>(&movable()).0.unwrap();
    let before = seen(&lift(&file));
    // 2 倍にして 90° 回し、X に 10 動かす
    let affine = Affine {
        m: [0.0, -2.0, 10.0, 2.0, 0.0, 0.0],
    };
    let changes = zumen_drawing::Changes {
        edits: before
            .iter()
            .map(|(target, _)| zumen_drawing::Edit::Transform {
                target: target.clone(),
                transform: affine,
            })
            .collect(),
        document: None,
    };
    let mut budget = Budget::default();
    let mut diag = Diagnostics::new();
    let applied = zumen_sdk::Apply::apply(
        &mut file,
        &changes,
        &mut Cx::new(&mut budget, &mut diag, &CancelToken::new()),
    )
    .unwrap();
    assert_eq!(diag.iter().count(), 0);

    let written = testkit::write::<Dxf>(&file).unwrap();
    let after = seen(&lift(&testkit::read::<Dxf>(&written).0.unwrap()));
    assert_eq!(after.len(), before.len());
    for (((was_id, was), (now_id, now)), applied) in before.iter().zip(&after).zip(&applied) {
        // 形は写した形になり、ID は変わらず、返すハッシュは読み直したときと同じ
        assert_eq!(now.shape, transform(&was.shape, &affine).unwrap());
        assert_eq!(now_id.id, was_id.id);
        assert_eq!(applied.id.as_deref(), Some(was_id.id.as_str()));
        assert_eq!(applied.hash, Some(now_id.hash));
    }
    let text = String::from_utf8(written).unwrap();
    // 位置は写し、変位は平行移動を除いて写し、向きは元の長さにそろえ、距離と倍率に尺度を掛ける
    assert!(
        text.contains(
            "1001\nZUMEN\n1011\n8.0\n1021\n2.0\n1031\n0.0\n1012\n0.0\n1022\n2.0\n1032\n0.0\n\
             1013\n0.0\n1023\n1.0\n1033\n0.0\n1041\n4.0\n1042\n6.0\n"
        ),
        "{text}"
    );
    // 属性は置く点を写す（(1, 0) は (10, 2) に）
    assert!(
        text.contains("  0\nATTRIB\n  5\n1D\n  8\n0\n 10\n10.0\n 20\n2.0\n"),
        "{text}"
    );
}

#[test]
fn transforms_the_records_cannot_hold_change_nothing() {
    use zumen_drawing::Affine;
    let src = movable();
    let mut file = testkit::read::<Dxf>(&src).0.unwrap();
    let [(line, _), (arc, _), _] = <[_; 3]>::try_from(seen(&lift(&file))).unwrap();
    let transform = |target: &zumen_drawing::Seen, m: [f64; 6]| zumen_drawing::Edit::Transform {
        target: target.clone(),
        transform: Affine { m },
    };
    let sheared = [1.0, 1.0, 0.0, 0.0, 1.0, 0.0];
    let flattened = [1.0, 0.0, 0.0, 0.0, 0.0, 0.0];
    for edit in [
        // 円弧はずらすと楕円になる
        transform(&arc, sheared),
        // 距離（1041）は、相似でない変換では直せない
        transform(&line, sheared),
        // 向きがつぶれる
        transform(&line, flattened),
    ] {
        let mut budget = Budget::default();
        let mut diag = Diagnostics::new();
        let result = zumen_sdk::Apply::apply(
            &mut file,
            &zumen_drawing::Changes {
                edits: vec![edit],
                document: None,
            },
            &mut Cx::new(&mut budget, &mut diag, &CancelToken::new()),
        );
        assert_eq!(result.unwrap_err(), Error::Rejected);
        assert_eq!(
            diag.iter().map(|d| d.code()).collect::<Vec<_>>(),
            [codes::CHANGE_EDIT_UNSUPPORTED]
        );
    }
    assert_eq!(testkit::write::<Dxf>(&file).unwrap(), src);
}

#[test]
fn scaling_mtext_rejects_absolute_and_unparsed_formatting() {
    use zumen_drawing::{Affine, Edit as Change};
    for (content, scale, rejected) in [
        ("\\H2;A", 2.0, true),
        ("\\H2;A", 1.0, false),
        ("\\H2x;A", 2.0, false),
        ("\\Habc;A", 2.0, true),
    ] {
        let src = versioned(
            "AC1021",
            &[
                (0, "MTEXT"),
                (5, "1A"),
                (8, "0"),
                (10, "0.0"),
                (20, "0.0"),
                (40, "2.5"),
                (1, content),
            ],
        );
        let mut file = testkit::read::<Dxf>(&src).0.unwrap();
        let target = seen(&lift(&file)).remove(0).0;
        let changes = zumen_drawing::Changes {
            edits: vec![Change::Transform {
                target,
                transform: Affine {
                    m: [scale, 0.0, 1.0, 0.0, scale, 0.0],
                },
            }],
            document: None,
        };
        let mut budget = Budget::default();
        let mut diag = Diagnostics::new();
        let result = zumen_sdk::Apply::apply(
            &mut file,
            &changes,
            &mut Cx::new(&mut budget, &mut diag, &CancelToken::new()),
        );
        if rejected {
            assert_eq!(result, Err(Error::Rejected), "{content}");
            assert!(
                diag.iter()
                    .any(|d| d.code() == codes::CHANGE_EDIT_UNSUPPORTED)
            );
            assert_eq!(testkit::write::<Dxf>(&file).unwrap(), src);
        } else {
            assert!(result.is_ok(), "{content}: {result:?}");
        }
    }
}

#[test]
fn similarity_preserves_full_ellipses_and_arcs() {
    use zumen_drawing::{Affine, Edge, Edit as Change};
    let transforms = [
        Affine {
            m: [1.0, 0.0, 0.0, 0.0, -1.0, 0.0],
        },
        Affine {
            m: [0.0, -1.0, 0.0, 1.0, 0.0, 0.0],
        },
        Affine {
            m: [2.0, 0.0, 0.0, 0.0, 2.0, 0.0],
        },
        Affine {
            m: [0.0, -2.0, 0.0, 2.0, 0.0, 0.0],
        },
    ];
    for (kind, tags) in [
        (
            "ELLIPSE",
            vec![
                (0, "ELLIPSE"),
                (5, "1A"),
                (8, "0"),
                (10, "0"),
                (20, "0"),
                (11, "2"),
                (21, "0"),
                (40, "0.5"),
                (41, "0"),
                (42, "6.283185307179586"),
            ],
        ),
        (
            "ARC",
            vec![
                (0, "ARC"),
                (5, "1A"),
                (8, "0"),
                (10, "0"),
                (20, "0"),
                (40, "2"),
                (50, "0"),
                (51, "360"),
            ],
        ),
    ] {
        for transform in transforms {
            let src = versioned("AC1021", &tags);
            let mut file = testkit::read::<Dxf>(&src).0.unwrap();
            let target = seen(&lift(&file)).remove(0).0;
            let changes = zumen_drawing::Changes {
                edits: vec![Change::Transform { target, transform }],
                document: None,
            };
            let mut budget = Budget::default();
            let mut diag = Diagnostics::new();
            zumen_sdk::Apply::apply(
                &mut file,
                &changes,
                &mut Cx::new(&mut budget, &mut diag, &CancelToken::new()),
            )
            .unwrap();
            let written = testkit::write::<Dxf>(&file).unwrap();
            let file = testkit::read::<Dxf>(&written).0.unwrap();
            let after = seen(&lift(&file));
            let (start, end) = match after[0].1.shape {
                Shape::Curve(Edge::Elliptic { start, end, .. } | Edge::Arc { start, end, .. }) => {
                    (start, end)
                }
                _ => panic!(),
            };
            assert!(
                (end - start).abs() >= 360.0,
                "{kind} {transform:?}: start={start}, end={end}"
            );
        }
    }
}

#[test]
fn rotating_aligned_text_also_rotates_its_first_point() {
    use zumen_drawing::{Affine, Edit as Change};
    let src = versioned(
        "AC1021",
        &[
            (0, "TEXT"),
            (5, "1A"),
            (8, "0"),
            (10, "0"),
            (20, "0"),
            (11, "2"),
            (21, "0"),
            (40, "2"),
            (1, "A"),
            (72, "1"),
        ],
    );
    let mut file = testkit::read::<Dxf>(&src).0.unwrap();
    let target = seen(&lift(&file)).remove(0).0;
    let changes = zumen_drawing::Changes {
        edits: vec![Change::Transform {
            target,
            transform: Affine {
                m: [0.0, -1.0, 0.0, 1.0, 0.0, 0.0],
            },
        }],
        document: None,
    };
    let mut budget = Budget::default();
    let mut diag = Diagnostics::new();
    zumen_sdk::Apply::apply(
        &mut file,
        &changes,
        &mut Cx::new(&mut budget, &mut diag, &CancelToken::new()),
    )
    .unwrap();
    let written = testkit::write::<Dxf>(&file).unwrap();
    let file = testkit::read::<Dxf>(&written).0.unwrap();
    let record = file
        .records()
        .iter()
        .find(|r| file.kind(r) == b"TEXT")
        .unwrap();
    let find = |code| {
        record
            .tags()
            .iter()
            .find(|t| t.code() == code)
            .map(|t| file.value(t))
            .unwrap()
    };
    assert_eq!(
        std::str::from_utf8(find(10))
            .unwrap()
            .parse::<f64>()
            .unwrap(),
        0.0
    );
    assert_eq!(
        std::str::from_utf8(find(20))
            .unwrap()
            .parse::<f64>()
            .unwrap(),
        0.0
    );
}

#[test]
fn transform_moves_vertically_aligned_attributes() {
    use zumen_drawing::{Affine, Edit as Change, Placement};
    let original = String::from_utf8(movable()).unwrap();
    let old = "  0\nATTRIB\n  5\n1D\n  8\n0\n 10\n1.0\n 20\n0.0\n";
    let new = "  0\nATTRIB\n  5\n1D\n  8\n0\n 10\n1.0\n 20\n0.0\n 11\n2.0\n 21\n0.0\n 74\n2\n";
    assert!(original.contains(old));
    let src = original.replace(old, new).into_bytes();
    for (transform, expected_anchor, expected_first) in [
        (Affine::translation(5.0, 3.0), (7.0, 3.0), (6.0, 3.0)),
        (
            Affine {
                m: [0.0, -1.0, 0.0, 1.0, 0.0, 0.0],
            },
            (0.0, 2.0),
            (0.0, 1.0),
        ),
    ] {
        let mut file = testkit::read::<Dxf>(&src).0.unwrap();
        let target = seen(&lift(&file))[2].0.clone();
        let changes = zumen_drawing::Changes {
            edits: vec![Change::Transform { target, transform }],
            document: None,
        };
        let mut budget = Budget::default();
        let mut diag = Diagnostics::new();
        zumen_sdk::Apply::apply(
            &mut file,
            &changes,
            &mut Cx::new(&mut budget, &mut diag, &CancelToken::new()),
        )
        .unwrap();
        let written = testkit::write::<Dxf>(&file).unwrap();
        let file = testkit::read::<Dxf>(&written).0.unwrap();
        let after = seen(&lift(&file));
        let Shape::Symbol(symbol) = &after[2].1.shape else {
            panic!()
        };
        let Placement::Anchor { point, .. } = symbol.attributes[0].text.placement else {
            panic!()
        };
        assert_eq!((point.x, point.y), expected_anchor);
        let record = file
            .records()
            .iter()
            .find(|r| file.kind(r) == b"ATTRIB")
            .unwrap();
        let value = |code| {
            let tag = record.tags().iter().find(|t| t.code() == code).unwrap();
            std::str::from_utf8(file.value(tag))
                .unwrap()
                .parse::<f64>()
                .unwrap()
        };
        assert_eq!((value(10), value(20)), expected_first);
    }
}

#[test]
fn transforms_count_attribute_searches_and_xdata_before_writing() {
    use zumen_base::{Kind, Limit, Limits};
    use zumen_drawing::{Affine, Edit as Change};
    let source = String::from_utf8(movable()).unwrap();
    let transform = |src: &[u8], limit: Option<u64>| {
        let mut file = testkit::read::<Dxf>(src).0.unwrap();
        let target = seen(&lift(&file)).last().unwrap().0.clone();
        let changes = zumen_drawing::Changes {
            edits: vec![Change::Transform {
                target,
                transform: Affine::translation(1.0, 0.0),
            }],
            document: None,
        };
        let limits = match limit {
            Some(limit) => Limits::default().with(Kind::Work, Limit::Finite(limit)),
            None => Limits::default(),
        };
        let mut budget = Budget::new(limits);
        let mut diag = Diagnostics::new();
        let result = zumen_sdk::Apply::apply(
            &mut file,
            &changes,
            &mut Cx::new(&mut budget, &mut diag, &CancelToken::new()),
        );
        (
            result,
            budget.used(Kind::Work),
            testkit::write::<Dxf>(&file).unwrap(),
        )
    };
    let (result, baseline, _) = transform(source.as_bytes(), None);
    assert!(result.is_ok());
    let marker = " 70\n0\n  0\nSEQEND\n";
    assert!(source.contains(marker));
    let payload = format!(
        " 70\n0\n1001\nAPP\n{}  0\nSEQEND\n",
        "1000\nx\n".repeat(300)
    );
    let enlarged = source.replacen(marker, &payload, 1).into_bytes();
    let (result, _, written) = transform(&enlarged, Some(baseline + 30));
    assert!(matches!(result, Err(Error::Stopped(_))));
    assert_eq!(written, enlarged);
}

/// ハンドルを持つブロック B（LINE と CIRCLE）を定義し、配置する DXF
#[cfg(test)]
fn defined() -> Vec<u8> {
    let tags: &[(i16, &str)] = &[
        (0, "SECTION"),
        (2, "HEADER"),
        (9, "$ACADVER"),
        (1, "AC1015"),
        (9, "$INSUNITS"),
        (70, "4"),
        (9, "$HANDSEED"),
        (5, "40"),
        (0, "ENDSEC"),
        (0, "SECTION"),
        (2, "BLOCKS"),
        (0, "BLOCK"),
        (5, "30"),
        (8, "0"),
        (2, "B"),
        (70, "0"),
        (10, "0.0"),
        (20, "0.0"),
        (30, "0.0"),
        (0, "LINE"),
        (5, "31"),
        (8, "0"),
        (10, "0.0"),
        (20, "0.0"),
        (30, "0.0"),
        (11, "1.0"),
        (21, "0.0"),
        (31, "0.0"),
        (0, "CIRCLE"),
        (5, "32"),
        (8, "0"),
        (10, "0.0"),
        (20, "0.0"),
        (30, "0.0"),
        (40, "1.0"),
        (0, "ENDBLK"),
        (5, "33"),
        (8, "0"),
        (0, "ENDSEC"),
        (0, "SECTION"),
        (2, "ENTITIES"),
        (0, "INSERT"),
        (5, "1A"),
        (8, "0"),
        (2, "B"),
        (10, "5.0"),
        (20, "5.0"),
        (30, "0.0"),
        (0, "ENDSEC"),
        (0, "EOF"),
    ];
    tags.iter()
        .map(|(code, value)| format!("{code:>3}\n{value}\n"))
        .collect::<String>()
        .into_bytes()
}

#[test]
fn transforms_mirror_texts_and_attributes_by_turning_them_upside_down() {
    use zumen_drawing::{Affine, transform::transform};
    // X 軸で鏡映し、X に 10 動かす
    let mirrored = Affine {
        m: [1.0, 0.0, 10.0, 0.0, -1.0, 0.0],
    };
    for src in [texts("AC1015"), movable()] {
        let mut file = testkit::read::<Dxf>(&src).0.unwrap();
        let before = seen(&lift(&file));
        let targets: Vec<_> = before
            .iter()
            .filter(|(_, element)| matches!(element.shape, Shape::Text(_) | Shape::Symbol(_)))
            .collect();
        let mut budget = Budget::default();
        let mut diag = Diagnostics::new();
        let applied = zumen_sdk::Apply::apply(
            &mut file,
            &zumen_drawing::Changes {
                edits: targets
                    .iter()
                    .map(|(target, _)| zumen_drawing::Edit::Transform {
                        target: target.clone(),
                        transform: mirrored,
                    })
                    .collect(),
                document: None,
            },
            &mut Cx::new(&mut budget, &mut diag, &CancelToken::new()),
        )
        .unwrap();
        assert_eq!(diag.iter().count(), 0);
        let written = testkit::write::<Dxf>(&file).unwrap();
        let after = seen(&lift(&testkit::read::<Dxf>(&written).0.unwrap()));
        for ((target, was), applied) in targets.iter().zip(&applied) {
            let (now_id, now) = after.iter().find(|(id, _)| id.id == target.id).unwrap();
            assert_eq!(now.shape, transform(&was.shape, &mirrored).unwrap());
            assert_eq!(applied.hash, Some(now_id.hash));
        }
        // 文字と属性は、上下の反転の印（71 の 4）を持つ
        let text = String::from_utf8(written).unwrap();
        assert!(
            text.contains(
                " 71
4
"
            ),
            "{text}"
        );
        if let Some(Shape::Text(text)) = after.first().map(|(_, element)| &element.shape) {
            assert!(text.upside_down);
            assert_eq!(text.rotation, 0.0);
        }
    }
}

#[test]
fn transforms_mirror_symbols_without_attributes_by_flipping_the_y_scale() {
    use zumen_drawing::{Affine, Shape, transform::transform};
    let mut file = testkit::read::<Dxf>(&defined()).0.unwrap();
    let [(insert, before)] = <[_; 1]>::try_from(seen(&lift(&file))).unwrap();
    // 直線 y = x で鏡映する。(5, 5) の配置は動かず、定義の (1, 0) は (5, 6) に写る
    let mirrored = Affine {
        m: [0.0, 1.0, 0.0, 1.0, 0.0, 0.0],
    };
    let mut budget = Budget::default();
    let mut diag = Diagnostics::new();
    let applied = zumen_sdk::Apply::apply(
        &mut file,
        &zumen_drawing::Changes {
            edits: vec![zumen_drawing::Edit::Transform {
                target: insert.clone(),
                transform: mirrored,
            }],
            document: None,
        },
        &mut Cx::new(&mut budget, &mut diag, &CancelToken::new()),
    )
    .unwrap();
    assert_eq!(diag.iter().count(), 0);
    let written = testkit::write::<Dxf>(&file).unwrap();
    let [(after_id, after)] =
        <[_; 1]>::try_from(seen(&lift(&testkit::read::<Dxf>(&written).0.unwrap()))).unwrap();
    assert_eq!(after.shape, transform(&before.shape, &mirrored).unwrap());
    let Shape::Symbol(symbol) = &after.shape else {
        panic!("{:?}", after.shape);
    };
    assert_eq!(symbol.scale, [1.0, -1.0]);
    assert_eq!(symbol.rotation, 90.0);
    let place = |[x, y]: [f64; 2]| {
        let (sin, cos) = (
            libm::sin(symbol.rotation.to_radians()),
            libm::cos(symbol.rotation.to_radians()),
        );
        let [sx, sy] = symbol.scale;
        let (x, y) = (x * sx, y * sy);
        [
            symbol.position.x + x * cos - y * sin,
            symbol.position.y + x * sin + y * cos,
        ]
    };
    let rounded = |[x, y]: [f64; 2]| [(x * 1e9).round() / 1e9, (y * 1e9).round() / 1e9];
    assert_eq!(rounded(place([1.0, 0.0])), [5.0, 6.0]);
    assert_eq!(rounded(place([0.0, 1.0])), [6.0, 5.0]);
    assert_eq!(applied[0].id.as_deref(), Some(insert.id.as_str()));
    assert_eq!(applied[0].hash, Some(after_id.hash));
}

/// 記号の定義の要素の ID とハッシュ
#[cfg(test)]
fn defined_elements(drawing: &Drawing2D) -> Vec<(zumen_drawing::Seen, Element<ModelPoint>)> {
    use zumen_drawing::{Hash, Seen, id};
    let definition = drawing.symbols.first().unwrap();
    let place = id::DefinitionNamer::default().next(definition);
    let mut namer = id::Namer::new(&place);
    definition
        .space
        .elements
        .iter()
        .enumerate()
        .map(|(index, element)| {
            let seen = Seen {
                id: namer.next(index, element),
                hash: Hash::element(element),
            };
            (seen, element.clone())
        })
        .collect()
}

#[test]
fn elements_of_symbol_definitions_are_changed_by_id() {
    use zumen_drawing::{Affine, Inherit, Restyle, id, transform::transform};
    let file = testkit::read::<Dxf>(&defined()).0.unwrap();
    let drawing = lift(&file);
    assert_eq!(
        id::DefinitionNamer::default().next(&drawing.symbols[0]),
        "space/h30"
    );
    let [(line, line_before), (circle, _)] =
        <[_; 2]>::try_from(defined_elements(&drawing)).unwrap();
    assert_eq!(line.id, "space/h30/h31");
    let moved = Affine::translation(2.0, 3.0);
    let mut file = testkit::read::<Dxf>(&defined()).0.unwrap();
    let mut budget = Budget::default();
    let mut diag = Diagnostics::new();
    let applied = zumen_sdk::Apply::apply(
        &mut file,
        &zumen_drawing::Changes {
            edits: vec![
                zumen_drawing::Edit::Transform {
                    target: line.clone(),
                    transform: moved,
                },
                zumen_drawing::Edit::Restyle {
                    target: circle.clone(),
                    style: Restyle {
                        color: Some(Inherit::Block),
                        ..Restyle::default()
                    },
                },
            ],
            document: None,
        },
        &mut Cx::new(&mut budget, &mut diag, &CancelToken::new()),
    )
    .unwrap();
    assert_eq!(diag.iter().count(), 0);
    let written = testkit::write::<Dxf>(&file).unwrap();
    let after = defined_elements(&lift(&testkit::read::<Dxf>(&written).0.unwrap()));
    let [(line_id, line_after), (circle_id, circle_after)] = <[_; 2]>::try_from(after).unwrap();
    assert_eq!(
        line_after.shape,
        transform(&line_before.shape, &moved).unwrap()
    );
    assert_eq!(circle_after.style.color, Inherit::Block);
    assert_eq!(applied[0].id.as_deref(), Some(line.id.as_str()));
    assert_eq!(applied[0].hash, Some(line_id.hash));
    assert_eq!(applied[1].hash, Some(circle_id.hash));

    // 定義の要素は除ける。定義の空間に足すことは、まだしない
    let mut file = testkit::read::<Dxf>(&defined()).0.unwrap();
    let mut budget = Budget::default();
    let mut diag = Diagnostics::new();
    let refused = zumen_sdk::Apply::apply(
        &mut file,
        &zumen_drawing::Changes {
            edits: vec![zumen_drawing::Edit::Add {
                place: "space/h30".to_owned(),
                element: Box::new(zumen_drawing::Placed::Model(line_before.clone())),
            }],
            document: None,
        },
        &mut Cx::new(&mut budget, &mut diag, &CancelToken::new()),
    );
    assert_eq!(refused.unwrap_err(), Error::Rejected);
    assert_eq!(
        diag.iter().map(|d| d.code()).collect::<Vec<_>>(),
        [codes::CHANGE_EDIT_UNSUPPORTED]
    );
    let mut budget = Budget::default();
    let mut diag = Diagnostics::new();
    zumen_sdk::Apply::apply(
        &mut file,
        &zumen_drawing::Changes {
            edits: vec![zumen_drawing::Edit::Remove { target: circle }],
            document: None,
        },
        &mut Cx::new(&mut budget, &mut diag, &CancelToken::new()),
    )
    .unwrap();
    let written = testkit::write::<Dxf>(&file).unwrap();
    let after = defined_elements(&lift(&testkit::read::<Dxf>(&written).0.unwrap()));
    assert_eq!(after.len(), 1);
    assert_eq!(after[0].0.id, "space/h30/h31");
}

#[test]
fn mirror_symbols_preserves_world_geometry_for_rotations_scales_and_block_bases() {
    use zumen_drawing::{Affine, Shape};
    let template = String::from_utf8(defined()).unwrap();
    let original = "  2\nB\n 70\n0\n 10\n0.0\n 20\n0.0\n";
    let with_base = "  2\nB\n 70\n0\n 10\n1.25\n 20\n-2.5\n";
    assert!(template.contains(original));
    let template = template.replacen(original, with_base, 1);
    let insert = "  0\nINSERT\n  5\n1A\n  8\n0\n  2\nB\n 10\n5.0\n 20\n5.0\n 30\n0.0\n";
    assert!(template.contains(insert));
    let matrix = Affine {
        m: [1.5, 2.0, 7.0, 2.0, -1.5, -2.0],
    };
    for (angle, sx, sy) in [
        (30.0, 2.0, 3.0),
        (123.0, -2.0, 3.0),
        (270.0, 2.0, -3.0),
        (315.0, -2.0, -3.0),
    ] {
        let extra = format!("{insert} 41\n{sx}\n 42\n{sy}\n 50\n{angle}\n");
        let src = template.replacen(insert, &extra, 1);
        let mut file = testkit::read::<Dxf>(src.as_bytes()).0.unwrap();
        let before_drawing = lift(&file);
        let before = &before_drawing.spaces[0].elements[0];
        let before_symbol = match &before.shape {
            Shape::Symbol(symbol) => symbol,
            other => panic!("{other:?}"),
        };
        let before_base = before_drawing
            .symbols
            .iter()
            .find(|symbol| symbol.key == before_symbol.key)
            .unwrap()
            .base
            .unwrap();
        let target = seen(&before_drawing).remove(0).0;
        let mut budget = Budget::default();
        let mut diag = Diagnostics::new();
        let cancel = CancelToken::new();
        let applied = zumen_sdk::Apply::apply(
            &mut file,
            &zumen_drawing::Changes {
                edits: vec![zumen_drawing::Edit::Transform {
                    target: target.clone(),
                    transform: matrix,
                }],
                document: None,
            },
            &mut Cx::new(&mut budget, &mut diag, &cancel),
        )
        .unwrap();
        let written = testkit::write::<Dxf>(&file).unwrap();
        let after_drawing = lift(&testkit::read::<Dxf>(&written).0.unwrap());
        let after = &after_drawing.spaces[0].elements[0];
        let after_symbol = match &after.shape {
            Shape::Symbol(symbol) => symbol,
            other => panic!("{other:?}"),
        };
        let after_base = after_drawing
            .symbols
            .iter()
            .find(|symbol| symbol.key == after_symbol.key)
            .unwrap()
            .base
            .unwrap();
        let world =
            |symbol: &zumen_drawing::Symbol<ModelPoint>, base: ModelPoint, [x, y]: [f64; 2]| {
                let theta = symbol.rotation.to_radians();
                let (sin, cos) = (libm::sin(theta), libm::cos(theta));
                let x = (x - base.x) * symbol.scale[0];
                let y = (y - base.y) * symbol.scale[1];
                [
                    symbol.position.x + cos * x - sin * y,
                    symbol.position.y + sin * x + cos * y,
                ]
            };
        for point in [[1.25, -2.5], [0.0, 0.0], [1.0, 0.0], [3.0, -4.0]] {
            let [x, y] = world(before_symbol, before_base, point);
            let expected = matrix.apply(x, y);
            let found = world(after_symbol, after_base, point);
            assert!(
                (found[0] - expected.0).abs() < 1e-9 && (found[1] - expected.1).abs() < 1e-9,
                "angle={angle} scale=({sx},{sy}) point={point:?} found={found:?} expected={expected:?}"
            );
        }
        assert_eq!(applied[0].id.as_deref(), Some(target.id.as_str()));
        assert_eq!(applied[0].hash, Some(seen(&after_drawing)[0].0.hash));
    }
}

#[test]
fn reflection_of_an_already_flipped_rotated_text_preserves_other_flags() {
    use zumen_drawing::{Affine, Edit as Change, Placement};
    let src = versioned(
        "AC1021",
        &[
            (0, "TEXT"),
            (5, "1A"),
            (8, "0"),
            (10, "2.0"),
            (20, "3.0"),
            (40, "2.0"),
            (1, "A"),
            (50, "30.0"),
            (71, "6"),
            (51, "12.0"),
        ],
    );
    let mut file = testkit::read::<Dxf>(&src).0.unwrap();
    let target = seen(&lift(&file)).remove(0).0;
    let changes = zumen_drawing::Changes {
        edits: vec![Change::Transform {
            target,
            transform: Affine {
                m: [0.0, 1.0, 0.0, 1.0, 0.0, 0.0],
            },
        }],
        document: None,
    };
    let mut budget = Budget::default();
    let mut diag = Diagnostics::new();
    zumen_sdk::Apply::apply(
        &mut file,
        &changes,
        &mut Cx::new(&mut budget, &mut diag, &CancelToken::new()),
    )
    .unwrap();
    let written = testkit::write::<Dxf>(&file).unwrap();
    let after = seen(&lift(&testkit::read::<Dxf>(&written).0.unwrap()));
    let Shape::Text(text) = &after[0].1.shape else {
        panic!()
    };
    let Placement::Anchor { point, .. } = text.placement else {
        panic!()
    };
    assert_eq!((point.x, point.y), (3.0, 2.0));
    assert_eq!(text.rotation, 60.0);
    assert!(text.backward);
    assert!(!text.upside_down);
    assert_eq!(text.oblique, 12.0);
}
