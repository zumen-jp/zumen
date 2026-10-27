//! 試験の中で作った小さな DXF で、図面モデル（L2）への変換を確かめる。
//! 対応表（docs/formats/drawing2d-mapping.md）の E1〜E3・E9 のうち、#3 の範囲と、#28 のレイヤと描く順・
//! 色と線種・E4 の寸法と引出線を試す。
#![cfg(feature = "drawing")]

use zumen_acad::Dxf;
use zumen_base::{Budget, CancelToken, Code, Cx, Diagnostics, Kind, Limit, Limits, Ref, codes};
use zumen_drawing::{
    Axis, Color, ColorCode, ColorSystem, Dimension, DimensionKind, DimensionText, Drawing2D,
    Element, Fit, Height, HeightMeaning, Horizontal, Inherit, Label, Layer, LayerId, LayerState,
    Leader, Length, LineTypeDefinition, LineTypeId, LineWeight, Measured, ModelLength, ModelPoint,
    PaperLength, PaperPoint, PatternBasis, Placement, Shape, SheetId, SheetItem, SpaceId, Style,
    Text, Unit, Vertical,
};
use zumen_sdk::{Error, Lift, testkit};

const MODEL: SpaceId = SpaceId(0);
const SHEET: SheetId = SheetId(0);

/// 見出しの変数と図形を、タグの組から作る。図形は `(0, 種類)` で区切る
fn dxf(header: &[(i16, &str)], entities: &[(i16, &str)]) -> Vec<u8> {
    sections(&[("HEADER", header), ("ENTITIES", entities)])
}

/// 名前とタグの組から、区切りを並べた DXF を作る
fn sections(sections: &[(&str, &[(i16, &str)])]) -> Vec<u8> {
    let mut tags = Vec::new();
    for (name, body) in sections {
        tags.extend_from_slice(&[(0, "SECTION"), (2, *name)]);
        tags.extend_from_slice(body);
        tags.push((0, "ENDSEC"));
    }
    tags.push((0, "EOF"));
    tags.iter()
        .map(|(code, value)| format!("{code:>3}\n{value}\n"))
        .collect::<String>()
        .into_bytes()
}

const MM: &[(i16, &str)] = &[(9, "$INSUNITS"), (70, "4")];

#[cfg(test)]
fn lift(src: &[u8], budget: &mut Budget) -> (Result<Drawing2D, Error>, Diagnostics) {
    let file = testkit::read::<Dxf>(src).0.unwrap();
    let mut diag = Diagnostics::new();
    let result = file.lift(&mut Cx::new(budget, &mut diag, &CancelToken::new()));
    (result, diag)
}

/// 診断の code・引数・件数・場所
fn found(diag: &Diagnostics) -> Vec<(Code, String, u64, Vec<Ref>)> {
    diag.iter()
        .map(|d| {
            let params: Vec<String> = d.params().iter().map(|(_, v)| format!("{v:?}")).collect();
            (d.code(), params.join(" "), d.count(), d.places().to_vec())
        })
        .collect()
}

/// どれもレイヤから継承する指定
fn by_layer() -> Style {
    Style {
        color: Inherit::Layer,
        line_type: Inherit::Layer,
        line_type_scale: 1.0,
        line_weight: Some(Inherit::Layer),
    }
}

fn at(x: f64, y: f64) -> ModelPoint {
    ModelPoint { space: MODEL, x, y }
}

fn on(x: f64, y: f64) -> PaperPoint {
    PaperPoint { sheet: SHEET, x, y }
}

#[test]
fn e1_an_isotropic_view_places_the_model_on_the_sheet() {
    let mut header = MM.to_vec();
    header.extend_from_slice(&[
        (9, "$PLIMMIN"),
        (10, "0.0"),
        (20, "0.0"),
        (9, "$PLIMMAX"),
        (10, "420.0"),
        (20, "297.0"),
    ]);
    let src = dxf(
        &header,
        &[
            (0, "LINE"),
            (8, "0"),
            (10, "0.0"),
            (20, "0.0"),
            (11, "1000.0"),
            (21, "500.0"),
            // 用紙そのもののビューポートは、ビューにしない
            (0, "VIEWPORT"),
            (67, "1"),
            (69, "1"),
            (10, "210.0"),
            (20, "148.5"),
            (40, "420.0"),
            (41, "297.0"),
            (45, "297.0"),
            // 実寸を 0.01 倍にして、用紙の（20,30）〜（30,35）に置く
            (0, "VIEWPORT"),
            (67, "1"),
            (69, "2"),
            (10, "25.0"),
            (20, "32.5"),
            (40, "10.0"),
            (41, "5.0"),
            (12, "500.0"),
            (22, "250.0"),
            (45, "500.0"),
        ],
    );
    let (drawing, diag) = lift(&src, &mut Budget::default());
    let drawing = drawing.unwrap();
    assert_eq!(found(&diag), []);

    let [space] = drawing.spaces.as_slice() else {
        panic!("{drawing:?}")
    };
    assert_eq!(space.unit, Unit::Millimeter);
    assert_eq!(
        space.elements,
        [Element {
            source: Ref::at_record(3),
            layer: LayerId(0),
            style: by_layer(),
            shape: Shape::Line {
                start: at(0.0, 0.0),
                end: at(1000.0, 500.0),
            },
        }]
    );

    let [sheet] = drawing.sheets.as_slice() else {
        panic!("{drawing:?}")
    };
    assert_eq!(sheet.size, Some([PaperLength(420.0), PaperLength(297.0)]));
    let [view] = sheet.views.as_slice() else {
        panic!("{sheet:?}")
    };
    assert_eq!(view.space, MODEL);
    assert_eq!(view.source, Ref::at_record(5));
    let [a, b, _, d, e, _] = view.transform.m;
    assert_eq!((a, b, d, e), (0.01, 0.0, 0.0, 0.01));
    assert_eq!(view.transform.apply(0.0, 0.0), (20.0, 30.0));
    assert_eq!(view.transform.apply(1000.0, 500.0), (30.0, 35.0));
    assert_eq!(
        view.clip,
        [
            on(20.0, 30.0),
            on(30.0, 30.0),
            on(30.0, 35.0),
            on(20.0, 35.0)
        ]
    );
}

fn text<P>(height: Length, placement: Placement<P>) -> Text<P> {
    Text {
        content: "A".to_owned(),
        height: Height {
            value: height,
            meaning: HeightMeaning::Declared("dxf.TEXT.40"),
        },
        placement,
        rotation: 0.0,
        width_factor: 1.0,
        oblique: 0.0,
        backward: false,
        upside_down: false,
        style: "STANDARD".to_owned(),
    }
}

fn anchor<P>(point: P) -> Placement<P> {
    Placement::Anchor {
        point,
        horizontal: Horizontal::Left,
        vertical: Vertical::Baseline,
    }
}

#[test]
fn e2_text_heights_keep_their_space() {
    for (unit, height, mm) in [("4", "2.5", 2.5), ("1", "0.125", 3.175)] {
        let src = dxf(
            &[(9, "$INSUNITS"), (70, unit)],
            &[
                (0, "TEXT"),
                (8, "0"),
                (10, "0.0"),
                (20, "0.0"),
                (40, "250.0"),
                (1, "A"),
                (0, "TEXT"),
                (67, "1"),
                (8, "0"),
                (10, "1.0"),
                (20, "2.0"),
                (40, height),
                (1, "A"),
            ],
        );
        let (drawing, diag) = lift(&src, &mut Budget::default());
        let drawing = drawing.unwrap();
        assert_eq!(found(&diag), []);
        assert_eq!(
            drawing.spaces[0].elements[0].shape,
            Shape::Text(text(
                Length::Model(ModelLength {
                    space: MODEL,
                    value: 250.0
                }),
                anchor(at(0.0, 0.0))
            ))
        );
        let scale = mm / height.parse::<f64>().unwrap();
        assert_eq!(
            drawing.sheets[0].elements[0].shape,
            Shape::Text(text(
                Length::Paper(PaperLength(mm)),
                anchor(on(scale, 2.0 * scale))
            ))
        );
    }
}

#[test]
fn e3_text_placements_are_kept_apart() {
    let entity = |h: &'static str, v: &'static str| {
        [
            (0, "TEXT"),
            (8, "0"),
            (10, "1.0"),
            (20, "2.0"),
            (11, "3.0"),
            (21, "4.0"),
            (40, "2.5"),
            (1, "A"),
            (72, h),
            (73, v),
        ]
    };
    let cases = [
        (
            "0",
            "0",
            Placement::Anchor {
                point: at(1.0, 2.0),
                horizontal: Horizontal::Left,
                vertical: Vertical::Baseline,
            },
        ),
        (
            "1",
            "2",
            Placement::Anchor {
                point: at(3.0, 4.0),
                horizontal: Horizontal::Center,
                vertical: Vertical::Middle,
            },
        ),
        (
            "2",
            "3",
            Placement::Anchor {
                point: at(3.0, 4.0),
                horizontal: Horizontal::Right,
                vertical: Vertical::Top,
            },
        ),
        (
            "4",
            "0",
            Placement::Anchor {
                point: at(3.0, 4.0),
                horizontal: Horizontal::Middle,
                vertical: Vertical::Baseline,
            },
        ),
        (
            "0",
            "1",
            Placement::Anchor {
                point: at(3.0, 4.0),
                horizontal: Horizontal::Left,
                vertical: Vertical::Bottom,
            },
        ),
        (
            "3",
            "0",
            Placement::Between {
                first: at(1.0, 2.0),
                second: at(3.0, 4.0),
                fit: Fit::Aligned,
            },
        ),
        (
            "5",
            "0",
            Placement::Between {
                first: at(1.0, 2.0),
                second: at(3.0, 4.0),
                fit: Fit::Fit,
            },
        ),
    ];
    for (h, v, placement) in cases {
        let (drawing, diag) = lift(&dxf(MM, &entity(h, v)), &mut Budget::default());
        assert_eq!(found(&diag), [], "{h} {v}");
        let Shape::Text(text) = &drawing.unwrap().spaces[0].elements[0].shape else {
            panic!()
        };
        assert_eq!(text.placement, placement, "{h} {v}");
    }

    let mut flagged = entity("0", "0").to_vec();
    flagged.extend_from_slice(&[
        (71, "6"),
        (50, "30.0"),
        (41, "0.8"),
        (51, "15.0"),
        (7, "ROMANS"),
    ]);
    let (drawing, _) = lift(&dxf(MM, &flagged), &mut Budget::default());
    let Shape::Text(text) = &drawing.unwrap().spaces[0].elements[0].shape else {
        panic!()
    };
    assert!(text.backward && text.upside_down);
    assert_eq!(
        (
            text.rotation,
            text.width_factor,
            text.oblique,
            text.style.as_str()
        ),
        (30.0, 0.8, 15.0, "ROMANS")
    );
}

#[test]
fn e9_unknown_units_are_not_taken_as_millimeters() {
    let line = |paper: &'static str| {
        [
            (0, "LINE"),
            (67, paper),
            (8, "0"),
            (10, "0.0"),
            (20, "0.0"),
            (11, "1.0"),
            (21, "1.0"),
        ]
    };
    let mut entities = line("0").to_vec();
    entities.extend_from_slice(&line("1"));
    let (drawing, diag) = lift(
        &dxf(&[(9, "$INSUNITS"), (70, "0")], &entities),
        &mut Budget::default(),
    );
    let drawing = drawing.unwrap();
    assert_eq!(drawing.spaces[0].unit, Unit::Unknown);
    assert_eq!(drawing.spaces[0].elements.len(), 1);
    assert_eq!(drawing.sheets, []);
    assert_eq!(
        found(&diag),
        [
            (codes::DXF_UNITS_MISSING, String::new(), 1, vec![]),
            (
                codes::DXF_PAPER_SPACE_DROPPED,
                r#"Text("LINE")"#.to_owned(),
                1,
                vec![Ref::at_record(4)]
            ),
        ]
    );
}

#[test]
fn e9_what_cannot_be_lifted_is_reported_per_record() {
    let viewport = |extra: &[(i16, &'static str)]| {
        let mut tags = vec![
            (0, "VIEWPORT"),
            (67, "1"),
            (10, "0.0"),
            (20, "0.0"),
            (40, "10.0"),
            (41, "10.0"),
            (45, "100.0"),
        ];
        tags.extend_from_slice(extra);
        tags
    };
    let mut entities = vec![
        // 3: Z のある線分
        (0, "LINE"),
        (8, "0"),
        (10, "0.0"),
        (20, "0.0"),
        (30, "5.0"),
        (11, "1.0"),
        (21, "1.0"),
        // 4: 押し出しの向きが逆の文字
        (0, "TEXT"),
        (8, "0"),
        (10, "0.0"),
        (20, "0.0"),
        (40, "1.0"),
        (1, "A"),
        (230, "-1.0"),
        // 5: まだ移さない種類
        (0, "CIRCLE"),
        (8, "0"),
        // 6: 画層のない線分
        (0, "LINE"),
        (10, "0.0"),
        (20, "0.0"),
        (11, "1.0"),
        (21, "1.0"),
        // 7: 範囲の外の位置合わせ
        (0, "TEXT"),
        (8, "0"),
        (10, "0.0"),
        (20, "0.0"),
        (11, "0.0"),
        (21, "0.0"),
        (40, "1.0"),
        (1, "A"),
        (72, "9"),
        // 8: コード・ページの版の ASCII でない文字
        (0, "TEXT"),
        (8, "0"),
        (10, "0.0"),
        (20, "0.0"),
        (40, "1.0"),
        (1, "\u{56f3}"),
    ];
    // 9〜12: 特異な縮尺・有限でない縮尺・ねじれ・視点の向き
    entities.extend(
        viewport(&[])
            .into_iter()
            .map(|t| if t.0 == 45 { (45, "0.0") } else { t }),
    );
    entities.extend(
        viewport(&[])
            .into_iter()
            .map(|t| if t.0 == 45 { (45, "inf") } else { t }),
    );
    entities.extend(viewport(&[(51, "30.0")]));
    entities.extend(viewport(&[(16, "1.0")]));
    let (drawing, diag) = lift(&dxf(MM, &entities), &mut Budget::default());
    let drawing = drawing.unwrap();
    assert_eq!(drawing.spaces[0].elements, []);
    assert_eq!(drawing.sheets, []);

    let text = |s: &str| format!("Text({s:?})");
    let expected = [
        (codes::DXF_NON_PLANAR_DROPPED, text("LINE"), 1, vec![3]),
        (codes::DXF_NON_PLANAR_DROPPED, text("TEXT"), 1, vec![4]),
        (codes::DXF_LIFT_DROPPED, text("CIRCLE"), 1, vec![5]),
        (
            codes::DXF_LIFT_INVALID,
            format!("{} Int(8)", text("LINE")),
            1,
            vec![6],
        ),
        (
            codes::DXF_LIFT_INVALID,
            format!("{} Int(72)", text("TEXT")),
            1,
            vec![7],
        ),
        (codes::DXF_CODE_PAGE_UNSUPPORTED, text("TEXT"), 1, vec![8]),
        (
            codes::DXF_LIFT_INVALID,
            format!("{} Int(45)", text("VIEWPORT")),
            2,
            vec![9, 10],
        ),
        (
            codes::DXF_VIEWPORT_UNSUPPORTED,
            "Int(51)".to_owned(),
            1,
            vec![11],
        ),
        (
            codes::DXF_VIEWPORT_UNSUPPORTED,
            "Int(16)".to_owned(),
            1,
            vec![12],
        ),
    ];
    let expected: Vec<_> = expected
        .into_iter()
        .map(|(c, p, n, at)| (c, p, n, at.into_iter().map(Ref::at_record).collect()))
        .collect();
    assert_eq!(found(&diag), expected);
}

#[test]
fn text_after_2007_is_read_as_utf8() {
    let mut header = vec![(9, "$ACADVER"), (1, "AC1032")];
    header.extend_from_slice(MM);
    let entities = [
        (0, "TEXT"),
        (8, "\u{5916}\u{5f62}"),
        (10, "0.0"),
        (20, "0.0"),
        (40, "1.0"),
        (1, "\u{56f3}\u{9762}"),
    ];
    let (drawing, diag) = lift(&dxf(&header, &entities), &mut Budget::default());
    assert_eq!(found(&diag), []);
    let drawing = drawing.unwrap();
    let element = &drawing.spaces[0].elements[0];
    assert_eq!(
        drawing
            .layer(element.layer)
            .map(|layer| layer.name.as_str()),
        Some("\u{5916}\u{5f62}")
    );
    let Shape::Text(text) = &element.shape else {
        panic!()
    };
    assert_eq!(text.content, "\u{56f3}\u{9762}");
}

#[test]
fn e9_lifting_stops_at_the_work_limit() {
    let src = dxf(MM, &[(0, "LINE"), (8, "0")]);
    let mut budget = Budget::new(Limits::default().with(Kind::Work, Limit::Finite(2)));
    let (result, diag) = lift(&src, &mut budget);
    assert!(matches!(result, Err(Error::Stopped(_))));
    assert_eq!(diag.iter().count(), 1);
}

/// 用紙の線分。ハンドルを持つ
fn paper_line(handle: &'static str) -> Vec<(i16, &'static str)> {
    vec![
        (0, "LINE"),
        (5, handle),
        (8, "0"),
        (67, "1"),
        (10, "0.0"),
        (20, "0.0"),
        (11, "1.0"),
        (21, "1.0"),
    ]
}

/// 実寸の線分。ハンドルを持つ
fn model_line(handle: &'static str, layer: &'static str) -> Vec<(i16, &'static str)> {
    vec![
        (0, "LINE"),
        (5, handle),
        (8, layer),
        (10, "0.0"),
        (20, "0.0"),
        (11, "1.0"),
        (21, "1.0"),
    ]
}

/// 用紙の、ビューになるビューポート。ハンドルと、ほかのタグを持つ
fn paper_view(handle: &'static str, extra: &[(i16, &'static str)]) -> Vec<(i16, &'static str)> {
    let mut tags = vec![
        (0, "VIEWPORT"),
        (5, handle),
        (8, "0"),
        (67, "1"),
        (69, "2"),
        (10, "5.0"),
        (20, "5.0"),
        (40, "10.0"),
        (41, "10.0"),
        (45, "100.0"),
    ];
    tags.extend_from_slice(extra);
    tags
}

#[test]
fn layers_keep_their_table_and_views_hide_layers() {
    let tables = [
        (0, "TABLE"),
        (2, "LAYER"),
        (0, "LAYER"),
        (5, "10"),
        (2, "0"),
        (70, "0"),
        (62, "7"),
        // 凍結
        (0, "LAYER"),
        (5, "11"),
        (2, "Hidden"),
        (70, "1"),
        (62, "1"),
        // 表示せず、印刷しない
        (0, "LAYER"),
        (5, "12"),
        (2, "Off"),
        (70, "0"),
        (62, "-3"),
        (290, "0"),
        // ロック
        (0, "LAYER"),
        (5, "13"),
        (2, "Locked"),
        (70, "4"),
        (62, "7"),
        (0, "ENDTAB"),
    ];
    let mut entities = model_line("20", "hidden");
    entities.extend(model_line("21", "Extra"));
    // 凍結した画層の 1 つ目は表にあり、2 つ目は表にない
    entities.extend(paper_view("22", &[(341, "12"), (341, "99")]));
    let (drawing, diag) = lift(
        &sections(&[("HEADER", MM), ("TABLES", &tables), ("ENTITIES", &entities)]),
        &mut Budget::default(),
    );
    let drawing = drawing.unwrap();
    assert_eq!(
        found(&diag),
        [(
            codes::DXF_LIFT_INVALID,
            r#"Text("VIEWPORT") Int(341)"#.to_owned(),
            1,
            vec![Ref::at_record(13)],
        )]
    );

    let state = |visible: bool, printable: bool, editable: bool| {
        Some(LayerState {
            visible,
            printable,
            editable,
        })
    };
    let states: Vec<_> = drawing
        .layers
        .iter()
        .map(|layer| (layer.id, layer.name.as_str(), layer.state))
        .collect();
    assert_eq!(
        states,
        [
            (LayerId(0), "0", state(true, true, true)),
            (LayerId(1), "Hidden", state(false, true, true)),
            (LayerId(2), "Off", state(false, false, true)),
            (LayerId(3), "Locked", state(true, true, false)),
            // 表にない名前は、定義のない画層にする
            (LayerId(4), "Extra", None),
        ]
    );
    assert_ne!(drawing.layers[0].source, Ref::default());
    assert_eq!(
        drawing.layers[4],
        Layer {
            id: LayerId(4),
            name: "Extra".to_owned(),
            state: None,
            style: None,
            source: Ref::default(),
        }
    );
    // 名前は大文字と小文字を区別せずに引く
    let layers: Vec<_> = drawing.spaces[0].elements.iter().map(|e| e.layer).collect();
    assert_eq!(layers, [LayerId(1), LayerId(4)]);
    assert_eq!(drawing.sheets[0].views[0].hidden_layers, [LayerId(2)]);
}

#[test]
fn sortents_tables_order_the_model_and_the_sheet() {
    let tables = [
        (0, "TABLE"),
        (2, "BLOCK_RECORD"),
        (0, "BLOCK_RECORD"),
        (5, "1F"),
        (2, "*Model_Space"),
        (0, "BLOCK_RECORD"),
        (5, "1E"),
        (2, "*PAPER_SPACE"),
        (0, "BLOCK_RECORD"),
        (5, "1D"),
        (2, "*Paper_Space0"),
        (0, "ENDTAB"),
    ];
    let mut entities = model_line("30", "0");
    entities.extend(model_line("31", "0"));
    entities.extend(model_line("32", "0"));
    entities.extend(paper_line("40"));
    entities.extend(paper_view("41", &[]));
    entities.extend(paper_line("42"));
    let objects = [
        // モデル空間: 30 を最後に、31 を最初に描く
        (0, "SORTENTSTABLE"),
        (5, "50"),
        (100, "AcDbSortentsTable"),
        (330, "1F"),
        (331, "30"),
        (5, "99"),
        (331, "31"),
        (5, "2"),
        // ペーパー空間: ビューポートを最初に描く
        (0, "SORTENTSTABLE"),
        (5, "51"),
        (100, "AcDbSortentsTable"),
        (330, "1E"),
        (331, "41"),
        (5, "1"),
        // 移さないレイアウトの表は使わない
        (0, "SORTENTSTABLE"),
        (5, "52"),
        (100, "AcDbSortentsTable"),
        (330, "1D"),
        (331, "32"),
        (5, "0"),
        // 組の欠けた表は、表ごと使わない
        (0, "SORTENTSTABLE"),
        (5, "53"),
        (100, "AcDbSortentsTable"),
        (330, "1F"),
        (331, "32"),
    ];
    let (drawing, diag) = lift(
        &sections(&[
            ("HEADER", MM),
            ("TABLES", &tables),
            ("ENTITIES", &entities),
            ("OBJECTS", &objects),
        ]),
        &mut Budget::default(),
    );
    let drawing = drawing.unwrap();
    assert_eq!(
        found(&diag),
        [(
            codes::DXF_LIFT_INVALID,
            r#"Text("SORTENTSTABLE") Int(5)"#.to_owned(),
            1,
            vec![Ref::at_record(21)],
        )]
    );
    // 要素は入力の順のまま
    assert_eq!(drawing.spaces[0].elements.len(), 3);
    assert_eq!(drawing.spaces[0].order, [1, 2, 0]);
    assert_eq!(
        drawing.sheets[0].order,
        [
            SheetItem::View(0),
            SheetItem::Element(0),
            SheetItem::Element(1)
        ]
    );
}

#[test]
fn without_a_sortents_table_the_input_order_is_kept() {
    let mut entities = model_line("30", "0");
    entities.extend(model_line("2F", "0"));
    entities.extend(paper_line("40"));
    entities.extend(paper_view("3F", &[]));
    let (drawing, diag) = lift(&dxf(MM, &entities), &mut Budget::default());
    let drawing = drawing.unwrap();
    assert_eq!(found(&diag), []);
    assert_eq!(drawing.spaces[0].order, [0, 1]);
    assert_eq!(
        drawing.sheets[0].order,
        [SheetItem::Element(0), SheetItem::View(0)]
    );
}

#[test]
fn elements_without_handles_are_not_sorted() {
    let tables = [
        (0, "TABLE"),
        (2, "BLOCK_RECORD"),
        (0, "BLOCK_RECORD"),
        (5, "1F"),
        (2, "*Model_Space"),
        (0, "ENDTAB"),
    ];
    let mut entities = model_line("30", "0");
    entities.extend([
        (0, "LINE"),
        (8, "0"),
        (10, "0.0"),
        (20, "0.0"),
        (11, "1.0"),
        (21, "1.0"),
    ]);
    let objects = [
        (0, "SORTENTSTABLE"),
        (5, "50"),
        (100, "AcDbSortentsTable"),
        (330, "1F"),
        (331, "30"),
        (5, "99"),
    ];
    let (drawing, diag) = lift(
        &sections(&[
            ("HEADER", MM),
            ("TABLES", &tables),
            ("ENTITIES", &entities),
            ("OBJECTS", &objects),
        ]),
        &mut Budget::default(),
    );
    assert_eq!(
        found(&diag),
        [(
            codes::DXF_LIFT_INVALID,
            r#"Text("SORTENTSTABLE") Int(331)"#.to_owned(),
            1,
            vec![Ref::at_record(12)],
        )]
    );
    assert_eq!(drawing.unwrap().spaces[0].order, [0, 1]);
}

#[test]
fn colors_line_types_and_line_weights_keep_their_source_and_inheritance() {
    let mut header = MM.to_vec();
    header.extend_from_slice(&[(9, "$LTSCALE"), (40, "2.0"), (9, "$LWDEFAULT"), (370, "25")]);
    let tables = [
        (0, "TABLE"),
        (2, "LTYPE"),
        (0, "LTYPE"),
        (5, "14"),
        (2, "Continuous"),
        (3, "Solid line"),
        (73, "0"),
        (0, "LTYPE"),
        (5, "15"),
        (2, "Dashed"),
        (3, "__ __"),
        (73, "2"),
        (49, "0.5"),
        (74, "0"),
        (49, "-0.25"),
        (74, "0"),
        // 文字を持つ要素は、長さだけを移す
        (0, "LTYPE"),
        (5, "16"),
        (2, "Gas"),
        (73, "1"),
        (49, "1.0"),
        (74, "2"),
        (75, "0"),
        (9, "GAS"),
        (0, "ENDTAB"),
        (0, "TABLE"),
        (2, "LAYER"),
        // 表示しない画層の色は 62 の絶対値
        (0, "LAYER"),
        (5, "10"),
        (2, "0"),
        (62, "-5"),
        (6, "DASHED"),
        (370, "-3"),
        (0, "LAYER"),
        (5, "11"),
        (2, "T"),
        (62, "1"),
        (420, "1193046"),
        (6, "Continuous"),
        (370, "25"),
        (0, "ENDTAB"),
    ];
    let mut entities = model_line("20", "0");
    entities.extend(model_line("21", "T"));
    entities.extend([(62, "0"), (6, "ByBlock"), (370, "-2")]);
    entities.extend(model_line("22", "X"));
    entities.extend([
        (62, "3"),
        (420, "65280"),
        (6, "Hidden"),
        (48, "0.5"),
        (370, "50"),
    ]);
    entities.extend(model_line("23", "X"));
    // 範囲の外の色と線幅
    entities.extend(model_line("24", "0"));
    entities.push((62, "300"));
    entities.extend(model_line("25", "0"));
    entities.push((370, "300"));
    let (drawing, diag) = lift(
        &sections(&[
            ("HEADER", &header),
            ("TABLES", &tables),
            ("ENTITIES", &entities),
        ]),
        &mut Budget::default(),
    );
    let drawing = drawing.unwrap();
    assert_eq!(
        found(&diag),
        [
            (
                codes::DXF_LINE_TYPE_ELEMENT_DROPPED,
                r#"Text("Gas")"#.to_owned(),
                1,
                vec![Ref::at_record(6)],
            ),
            (
                codes::DXF_LIFT_INVALID,
                r#"Text("LINE") Int(62)"#.to_owned(),
                1,
                vec![Ref::at_record(18)],
            ),
            (
                codes::DXF_LIFT_INVALID,
                r#"Text("LINE") Int(370)"#.to_owned(),
                1,
                vec![Ref::at_record(19)],
            ),
        ]
    );
    assert_eq!(drawing.line_type_scale, Some(2.0));
    assert_eq!(drawing.default_line_weight, Some(PaperLength(0.25)));

    let names: Vec<_> = drawing
        .line_types
        .iter()
        .map(|t| (t.id, t.name.as_str(), t.definition.is_some()))
        .collect();
    assert_eq!(
        names,
        [
            (LineTypeId(0), "Continuous", true),
            (LineTypeId(1), "Dashed", true),
            (LineTypeId(2), "Gas", true),
            // 表にない名前は、定義のない線種にする
            (LineTypeId(3), "Hidden", false),
        ]
    );
    assert_eq!(
        drawing.line_types[1].definition,
        Some(LineTypeDefinition {
            description: "__ __".to_owned(),
            pattern: vec![0.5, -0.25],
            basis: PatternBasis::Element,
        })
    );
    assert_eq!(
        drawing.line_types[2]
            .definition
            .as_ref()
            .map(|d| d.pattern.clone()),
        Some(vec![1.0])
    );

    let aci = |value: u32| {
        Some(ColorCode {
            system: ColorSystem::Aci,
            value,
        })
    };
    let resolved: Vec<_> = drawing.spaces[0]
        .elements
        .iter()
        .map(|e| {
            (
                drawing.color_of(e),
                drawing.line_type_of(e),
                drawing.line_weight_of(e),
                e.style.line_type_scale,
            )
        })
        .collect();
    assert_eq!(
        resolved,
        [
            // レイヤから継承する
            (
                Inherit::Own(Color {
                    code: aci(5),
                    rgb: None
                }),
                Inherit::Own(LineTypeId(1)),
                Some(Inherit::Own(LineWeight::Default)),
                1.0,
            ),
            // 部品の配置から継承するものは解かない
            (Inherit::Block, Inherit::Block, Some(Inherit::Block), 1.0),
            // 要素が持つ値。符号と RGB の両方を残す
            (
                Inherit::Own(Color {
                    code: aci(3),
                    rgb: Some([0, 255, 0])
                }),
                Inherit::Own(LineTypeId(3)),
                Some(Inherit::Own(LineWeight::Width(PaperLength(0.5)))),
                0.5,
            ),
            // 定義のない画層からの継承は、継承のまま
            (Inherit::Layer, Inherit::Layer, Some(Inherit::Layer), 1.0),
        ]
    );
    let t = drawing.layers[1].style.unwrap();
    assert_eq!(
        t.color,
        Some(Color {
            code: aci(1),
            rgb: Some([0x12, 0x34, 0x56])
        })
    );
    assert_eq!(t.line_type, Some(LineTypeId(0)));
    assert_eq!(t.line_weight, Some(LineWeight::Width(PaperLength(0.25))));
    assert_eq!(drawing.layers[2].style, None);
}

/// 図形を種類とタグの組から作る。画層は 0
fn entity(kind: &'static str, tags: &[(i16, &'static str)]) -> Vec<(i16, &'static str)> {
    let mut entity = vec![(0, kind), (8, "0")];
    entity.extend_from_slice(tags);
    entity
}

/// 実寸の空間の要素の形
fn model_shapes(drawing: &Drawing2D) -> Vec<Shape<ModelPoint>> {
    drawing
        .spaces
        .iter()
        .flat_map(|space| &space.elements)
        .map(|element| element.shape.clone())
        .collect()
}

#[test]
fn e4_dimensions_keep_the_measurement_and_the_override_apart() {
    // 1000 mm 離れた 2 点の線形寸法。測定値は 1000 で、文字は「995」に上書きし、利用者が動かした（128）。
    // 部品は寸法だけが使う（32）
    let linear = entity(
        "DIMENSION",
        &[
            (2, "*D1"),
            (3, "ISO-25"),
            (10, "0.0"),
            (20, "100.0"),
            (11, "520.0"),
            (21, "110.0"),
            (70, "160"),
            (71, "5"),
            (42, "1000.0"),
            (1, "995"),
            (13, "0.0"),
            (23, "0.0"),
            (14, "1000.0"),
            (24, "0.0"),
        ],
    );
    let label = |text: &'static str| {
        let mut dimension = entity(
            "DIMENSION",
            &[
                (10, "0.0"),
                (20, "100.0"),
                (70, "1"),
                (13, "0.0"),
                (23, "0.0"),
                (14, "1000.0"),
                (24, "0.0"),
            ],
        );
        dimension.push((1, text));
        dimension
    };
    let mut entities = linear;
    for text in ["<>", " ", "<> mm"] {
        entities.extend(label(text));
    }
    let (drawing, diag) = lift(&dxf(MM, &entities), &mut Budget::default());
    let drawing = drawing.unwrap();
    assert!(found(&diag).is_empty(), "{:?}", found(&diag));
    let shapes = model_shapes(&drawing);
    assert_eq!(
        shapes[0],
        Shape::Dimension(Dimension {
            kind: DimensionKind::Linear {
                first: at(0.0, 0.0),
                second: at(1000.0, 0.0),
                line: at(0.0, 100.0),
                rotation: 0.0,
                oblique: 0.0,
            },
            measurement: Some(Measured {
                value: 1000.0,
                key: "dxf.DIMENSION.42",
            }),
            label: Label::Override("995".to_owned()),
            text: DimensionText {
                position: Some(at(520.0, 110.0)),
                moved: true,
                rotation: None,
                attachment: Some((Horizontal::Center, Vertical::Middle)),
            },
            style: "ISO-25".to_owned(),
            block: Some("*D1".to_owned()),
        })
    );
    let labels: Vec<_> = shapes[1..]
        .iter()
        .map(|shape| match shape {
            Shape::Dimension(dimension) => (dimension.label.clone(), dimension.measurement),
            other => panic!("{other:?}"),
        })
        .collect();
    assert_eq!(
        labels,
        [
            (Label::Measurement, None),
            (Label::Hidden, None),
            (Label::Override("<> mm".to_owned()), None),
        ]
    );
}

#[test]
fn e4_each_dimension_kind_keeps_its_definition_points() {
    let points = |tags: &[(i16, &'static str)]| entity("DIMENSION", tags);
    let mut entities = Vec::new();
    // 2 線の角度
    entities.extend(points(&[
        (70, "2"),
        (13, "0.0"),
        (23, "0.0"),
        (14, "10.0"),
        (24, "0.0"),
        (15, "0.0"),
        (25, "0.0"),
        (10, "0.0"),
        (20, "10.0"),
        (16, "5.0"),
        (26, "5.0"),
    ]));
    // 直径
    entities.extend(points(&[
        (70, "3"),
        (15, "10.0"),
        (25, "0.0"),
        (10, "-10.0"),
        (20, "0.0"),
        (40, "3.0"),
    ]));
    // 3 点の角度
    entities.extend(points(&[
        (70, "5"),
        (15, "0.0"),
        (25, "0.0"),
        (13, "10.0"),
        (23, "0.0"),
        (14, "0.0"),
        (24, "10.0"),
        (10, "7.0"),
        (20, "7.0"),
        (42, "1.5707963267948966"),
    ]));
    // X の座標寸法（6 + 64）
    entities.extend(points(&[
        (70, "70"),
        (10, "0.0"),
        (20, "0.0"),
        (13, "30.0"),
        (23, "40.0"),
        (14, "30.0"),
        (24, "60.0"),
    ]));
    // 回した線形寸法と、傾けた補助線
    entities.extend(points(&[
        (70, "0"),
        (13, "0.0"),
        (23, "0.0"),
        (14, "10.0"),
        (24, "10.0"),
        (10, "0.0"),
        (20, "5.0"),
        (50, "90.0"),
        (52, "30.0"),
        (53, "45.0"),
    ]));
    // 用紙の上の半径寸法。引出線の長さは用紙の長さになる
    entities.extend(points(&[
        (67, "1"),
        (70, "4"),
        (10, "50.0"),
        (20, "50.0"),
        (15, "60.0"),
        (25, "50.0"),
        (40, "5.0"),
    ]));
    let (drawing, diag) = lift(&dxf(MM, &entities), &mut Budget::default());
    let drawing = drawing.unwrap();
    assert!(found(&diag).is_empty(), "{:?}", found(&diag));
    let kinds: Vec<_> = model_shapes(&drawing)
        .into_iter()
        .map(|shape| match shape {
            Shape::Dimension(dimension) => dimension.kind,
            other => panic!("{other:?}"),
        })
        .collect();
    let model = |value: f64| {
        Length::Model(ModelLength {
            space: MODEL,
            value,
        })
    };
    assert_eq!(
        kinds,
        [
            DimensionKind::AngularLines {
                first: [at(0.0, 0.0), at(10.0, 0.0)],
                second: [at(0.0, 0.0), at(0.0, 10.0)],
                arc: at(5.0, 5.0),
            },
            DimensionKind::Diameter {
                chord: at(10.0, 0.0),
                opposite: at(-10.0, 0.0),
                leader: model(3.0),
            },
            DimensionKind::Angular {
                vertex: at(0.0, 0.0),
                first: at(10.0, 0.0),
                second: at(0.0, 10.0),
                arc: at(7.0, 7.0),
            },
            DimensionKind::Ordinate {
                origin: at(0.0, 0.0),
                feature: at(30.0, 40.0),
                leader_end: at(30.0, 60.0),
                axis: Axis::X,
            },
            DimensionKind::Linear {
                first: at(0.0, 0.0),
                second: at(10.0, 10.0),
                line: at(0.0, 5.0),
                rotation: 90.0,
                oblique: 30.0,
            },
        ]
    );
    let Shape::Dimension(rotated) = &drawing.spaces[0].elements[4].shape else {
        panic!();
    };
    assert_eq!(rotated.text.rotation, Some(45.0));
    let Shape::Dimension(radius) = &drawing.sheets[0].elements[0].shape else {
        panic!();
    };
    assert_eq!(
        radius.kind,
        DimensionKind::Radius {
            center: on(50.0, 50.0),
            chord: on(60.0, 50.0),
            leader: Length::Paper(PaperLength(5.0)),
        }
    );
}

#[test]
fn e4_leaders_keep_their_path_and_point_to_the_annotation() {
    let entities = [
        entity(
            "MTEXT",
            &[
                (5, "A1"),
                (10, "20.0"),
                (20, "10.0"),
                (40, "2.5"),
                (1, "注記"),
            ],
        ),
        entity(
            "LEADER",
            &[
                (3, "ISO-25"),
                (71, "1"),
                (72, "1"),
                (76, "3"),
                (10, "0.0"),
                (20, "0.0"),
                (30, "0.0"),
                (10, "10.0"),
                (20, "10.0"),
                (30, "0.0"),
                (10, "20.0"),
                (20, "10.0"),
                (30, "0.0"),
                (340, "A1"),
            ],
        ),
        // 矢印も注記も持たない折れ線
        entity(
            "LEADER",
            &[
                (76, "2"),
                (10, "0.0"),
                (20, "0.0"),
                (10, "5.0"),
                (20, "0.0"),
            ],
        ),
    ]
    .concat();
    let (drawing, diag) = lift(&dxf(MM, &entities), &mut Budget::default());
    let drawing = drawing.unwrap();
    // 注記の MTEXT はまだ移さないが、引出線はその出所を指す
    let found = found(&diag);
    let [(code, _, 1, mtext)] = found.as_slice() else {
        panic!("{found:?}");
    };
    assert_eq!(*code, codes::DXF_LIFT_DROPPED);
    assert_eq!(
        model_shapes(&drawing),
        [
            Shape::Leader(Leader {
                path: vec![at(0.0, 0.0), at(10.0, 10.0), at(20.0, 10.0)],
                spline: true,
                arrow: Some(true),
                annotation: Some(mtext[0].clone()),
                style: "ISO-25".to_owned(),
            }),
            Shape::Leader(Leader {
                path: vec![at(0.0, 0.0), at(5.0, 0.0)],
                spline: false,
                arrow: None,
                annotation: None,
                style: "STANDARD".to_owned(),
            }),
        ]
    );
}

#[test]
fn e9_dimensions_and_leaders_that_cannot_be_read_are_reported() {
    let dimension = |tags: &[(i16, &'static str)]| {
        let mut all = vec![
            (13, "0.0"),
            (23, "0.0"),
            (14, "1.0"),
            (24, "0.0"),
            (10, "0.0"),
            (20, "1.0"),
        ];
        all.extend_from_slice(tags);
        entity("DIMENSION", &all)
    };
    let entities = [
        // 種類の値がない
        dimension(&[(70, "7")]),
        // 文字の位置合わせが範囲の外
        dimension(&[(71, "10")]),
        // Z のある定義点
        dimension(&[(33, "1.0")]),
        // 頂点の数が合わない
        entity(
            "LEADER",
            &[
                (76, "3"),
                (10, "0.0"),
                (20, "0.0"),
                (10, "1.0"),
                (20, "0.0"),
            ],
        ),
        // 指すオブジェクトのない注記
        entity(
            "LEADER",
            &[
                (76, "2"),
                (10, "0.0"),
                (20, "0.0"),
                (10, "1.0"),
                (20, "0.0"),
                (340, "FF"),
            ],
        ),
        // Z のある頂点
        entity(
            "LEADER",
            &[
                (76, "2"),
                (10, "0.0"),
                (20, "0.0"),
                (30, "1.0"),
                (10, "1.0"),
                (20, "0.0"),
            ],
        ),
    ]
    .concat();
    let (drawing, diag) = lift(&dxf(MM, &entities), &mut Budget::default());
    assert_eq!(drawing.unwrap().spaces[0].elements, []);
    let params: Vec<_> = found(&diag)
        .into_iter()
        .map(|(c, p, n, _)| (c, p, n))
        .collect();
    let invalid = |kind: &str, code: i64| {
        (
            codes::DXF_LIFT_INVALID,
            format!("Text({kind:?}) Int({code})"),
            1,
        )
    };
    let non_planar = |kind: &str| (codes::DXF_NON_PLANAR_DROPPED, format!("Text({kind:?})"), 1);
    assert_eq!(
        params,
        [
            invalid("DIMENSION", 70),
            invalid("DIMENSION", 71),
            non_planar("DIMENSION"),
            invalid("LEADER", 76),
            invalid("LEADER", 340),
            non_planar("LEADER"),
        ]
    );
}
