//! 試験の中で作った小さな DXF で、図面モデル（L2）への変換を確かめる。
//! 対応表（docs/formats/drawing2d-mapping.md）の E1〜E3・E9 のうち、#3 の範囲と、#28 のレイヤと描く順・
//! 色と線種・E5 の形式固有の情報・E6 の同じ見た目の定義と符号・E8 の穴のある塗り・E9 の循環と欠けた定義・E3 の書式つきの文字と複数行の属性・E4 の寸法と寸法値の文字と引出線と端部記号・引出線つきの注記・
//! 記号と属性・ハッチング・E8 の曲線と記号の定義・E10 の表と表題欄・E7 の下敷きを試す。
#![cfg(feature = "drawing")]

use zumen_acad::Dxf;
use zumen_base::{Budget, CancelToken, Code, Cx, Diagnostics, Kind, Limit, Limits, Ref, codes};
use zumen_drawing::{
    Arrowhead, Axis, Background, BackgroundFill, Basis, Boundary, Branch, CalloutContent, Cell,
    CellContent, Clip, Color, ColorCode, ColorSystem, Dimension, DimensionKind, DimensionStyle,
    DimensionText, Dogleg, Drawing2D, Edge, Element, Extension, Field, FieldValue, Fill, Fit, Flow,
    Format, FormattedText, Gradient, Grid, Hatch, HatchLoop, Height, HeightMeaning, Horizontal,
    Inherit, Islands, Label, Layer, LayerId, LayerState, Leader, Length, LineSpacing,
    LineTypeDefinition, LineTypeId, LineWeight, Measured, ModelLength, ModelPoint, PaperLength,
    PaperPoint, PathKind, Pattern, PatternBasis, PatternLine, PatternOrigin, Piece, Placement,
    Polyline, Resource, Rotation, Shape, SheetId, SheetItem, SpaceId, Span, Spline, Style,
    SymbolKey, Table, Target, Text, Underlay, UnderlayFrame, UnderlayKind, Unit, Vertex, Vertical,
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

/// 定義のないブロックの診断を除いた診断。ブロックの定義を書かない試験で使う
fn found_but_missing_blocks(diag: &Diagnostics) -> Vec<(Code, String, u64, Vec<Ref>)> {
    let mut found = found(diag);
    found.retain(|(code, ..)| *code != codes::DXF_BLOCK_MISSING);
    found
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
        (0, "POINT"),
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
        (codes::DXF_LIFT_DROPPED, text("POINT"), 1, vec![5]),
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
    // 止めたときに扱っていたレコードを場所に持つ（決定 0119）
    let places: Vec<_> = diag.iter().flat_map(|d| d.places().to_vec()).collect();
    assert_eq!(places, [Ref::at_record(2)]);
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
fn sortents_tables_order_the_block_definitions() {
    let tables = [
        (0, "TABLE"),
        (2, "BLOCK_RECORD"),
        (0, "BLOCK_RECORD"),
        (5, "B1"),
        (2, "Door"),
        (0, "BLOCK_RECORD"),
        (5, "C1"),
        (2, "Window"),
        (0, "ENDTAB"),
    ];
    let mut blocks = vec![
        (0, "BLOCK"),
        (5, "B2"),
        (330, "B1"),
        (8, "0"),
        (2, "Door"),
        (70, "0"),
        (10, "0.0"),
        (20, "0.0"),
    ];
    blocks.extend(model_line("B3", "0"));
    blocks.extend(model_line("B4", "0"));
    blocks.extend(model_line("B5", "0"));
    blocks.extend([(0, "ENDBLK"), (5, "B6"), (330, "B1"), (8, "0")]);
    blocks.extend([
        (0, "BLOCK"),
        (5, "C2"),
        (330, "C1"),
        (8, "0"),
        (2, "Window"),
        (70, "0"),
        (10, "0.0"),
        (20, "0.0"),
    ]);
    blocks.extend(model_line("C3", "0"));
    blocks.extend(model_line("C4", "0"));
    blocks.extend([(0, "ENDBLK"), (5, "C5"), (330, "C1"), (8, "0")]);
    let objects = [
        // Door: B3 を最後に、B5 を最初に描く。Window は表がないので入力の順
        (0, "SORTENTSTABLE"),
        (5, "50"),
        (100, "AcDbSortentsTable"),
        (330, "B1"),
        (331, "B3"),
        (5, "FF"),
        (331, "B5"),
        (5, "1"),
    ];
    let (drawing, diag) = lift(
        &sections(&[
            ("HEADER", MM),
            ("TABLES", &tables),
            ("BLOCKS", &blocks),
            ("ENTITIES", &[]),
            ("OBJECTS", &objects),
        ]),
        &mut Budget::default(),
    );
    assert!(found(&diag).is_empty(), "{:?}", found(&diag));
    let drawing = drawing.unwrap();
    let order = |name: &str| {
        let symbol = drawing
            .symbols
            .iter()
            .find(|symbol| symbol.key.name == name)
            .unwrap();
        (symbol.space.elements.len(), symbol.space.order.clone())
    };
    assert_eq!(order("Door"), (3, vec![2, 1, 0]));
    assert_eq!(order("Window"), (2, vec![0, 1]));
}

#[test]
fn block_record_units_set_the_unit_of_the_definition() {
    // 文書は mm。Door は inch、Window は単位なし（0）、Frame は長さでない単位（ミル 9）、Sash は 70 を持たない
    let mut tables = vec![(0, "TABLE"), (2, "BLOCK_RECORD")];
    let names = [
        ("B1", "Door", Some("1")),
        ("C1", "Window", Some("0")),
        ("D1", "Frame", Some("9")),
        ("E1", "Sash", None),
    ];
    let mut blocks = Vec::new();
    for (handle, name, units) in names {
        tables.extend([(0, "BLOCK_RECORD"), (5, handle), (2, name)]);
        if let Some(units) = units {
            tables.push((70, units));
        }
        blocks.extend([
            (0, "BLOCK"),
            (330, handle),
            (8, "0"),
            (2, name),
            (70, "0"),
            (10, "0.0"),
            (20, "0.0"),
            (0, "ENDBLK"),
            (330, handle),
            (8, "0"),
        ]);
    }
    tables.push((0, "ENDTAB"));
    let (drawing, diag) = lift(
        &sections(&[
            ("HEADER", MM),
            ("TABLES", &tables),
            ("BLOCKS", &blocks),
            ("ENTITIES", &[]),
        ]),
        &mut Budget::default(),
    );
    assert!(found(&diag).is_empty(), "{:?}", found(&diag));
    let drawing = drawing.unwrap();
    let unit = |name: &str| {
        drawing
            .symbols
            .iter()
            .find(|symbol| symbol.key.name == name)
            .unwrap()
            .space
            .unit
    };
    assert_eq!(unit("Door"), Unit::Inch);
    assert_eq!(unit("Window"), Unit::Millimeter);
    assert_eq!(unit("Frame"), Unit::Unknown);
    assert_eq!(unit("Sash"), Unit::Millimeter);
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
    assert!(
        found_but_missing_blocks(&diag).is_empty(),
        "{:?}",
        found_but_missing_blocks(&diag)
    );
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
            label: Label::Override {
                content: "995".to_owned(),
                spans: vec![span(words("995"), Format::default())],
            },
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
            (
                Label::Override {
                    content: "<> mm".to_owned(),
                    spans: vec![
                        span(Piece::Measurement, Format::default()),
                        span(words(" mm"), Format::default()),
                    ],
                },
                None
            ),
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
                (1, "NOTE"),
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
    // 引出線は、注記の MTEXT の出所を指す
    assert_eq!(found(&diag), []);
    let mtext = &drawing.spaces[0].elements[0];
    assert!(matches!(mtext.shape, Shape::Formatted(_)));
    assert_eq!(
        model_shapes(&drawing)[1..],
        [
            Shape::Leader(Leader {
                path: vec![at(0.0, 0.0), at(10.0, 10.0), at(20.0, 10.0)],
                spline: true,
                arrow: Some(true),
                annotation: Some(mtext.source.clone()),
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

fn block(name: &str) -> SymbolKey {
    SymbolKey {
        namespace: "dxf.block",
        name: name.to_owned(),
    }
}

#[test]
fn e4_dimension_styles_keep_their_arrowheads() {
    let tables = [
        (0, "TABLE"),
        (2, "BLOCK_RECORD"),
        (0, "BLOCK_RECORD"),
        (5, "1A"),
        (2, "_DOT"),
        (0, "BLOCK_RECORD"),
        (5, "1B"),
        (2, "_ArchTick"),
        (0, "ENDTAB"),
        (0, "TABLE"),
        (2, "DIMSTYLE"),
        // 値を持たないので、既定の矢印と DXF の既定の値
        (0, "DIMSTYLE"),
        (105, "20"),
        (2, "Standard"),
        // 両端を別にし（173）、1 本目は点、2 本目は既定、引出線は斜線の記号。大きさ 2.5 を 100 倍する
        (0, "DIMSTYLE"),
        (105, "21"),
        (2, "ISO"),
        (40, "100.0"),
        (41, "2.5"),
        (173, "1"),
        (342, "1B"),
        (343, "1A"),
        (344, "0"),
        (341, "1B"),
        // 両端を別にしないので、両端とも 342。矢印の代わりに大きさ 1.5 の斜線を描く
        (0, "DIMSTYLE"),
        (105, "22"),
        (2, "Arch"),
        (142, "1.5"),
        (342, "1B"),
        (343, "1A"),
        // 指すブロックのレコードがない
        (0, "DIMSTYLE"),
        (105, "23"),
        (2, "Broken"),
        (342, "FF"),
        (0, "ENDTAB"),
    ];
    let (drawing, diag) = lift(
        &sections(&[("HEADER", MM), ("TABLES", &tables)]),
        &mut Budget::default(),
    );
    let drawing = drawing.unwrap();
    let found = found_but_missing_blocks(&diag);
    let [(code, params, 1, broken)] = found.as_slice() else {
        panic!("{found:?}");
    };
    assert_eq!(
        (*code, params.as_str()),
        (codes::DXF_LIFT_INVALID, r#"Text("DIMSTYLE") Int(342)"#)
    );
    let styles: Vec<_> = drawing
        .dimension_styles
        .iter()
        .map(|style| DimensionStyle {
            source: Ref::default(),
            ..style.clone()
        })
        .collect();
    let style = |name: &str, ends: [Arrowhead; 2], leader, size, tick, scale| DimensionStyle {
        name: name.to_owned(),
        ends,
        leader,
        size,
        tick,
        scale,
        source: Ref::default(),
    };
    let dot = || Arrowhead::Symbol(block("_DOT"));
    let arch = || Arrowhead::Symbol(block("_ArchTick"));
    assert_eq!(
        styles,
        [
            style(
                "Standard",
                [Arrowhead::Default, Arrowhead::Default],
                Arrowhead::Default,
                0.18,
                None,
                1.0
            ),
            style("ISO", [dot(), Arrowhead::Default], arch(), 2.5, None, 100.0),
            style(
                "Arch",
                [arch(), arch()],
                Arrowhead::Default,
                0.18,
                Some(1.5),
                1.0
            ),
        ]
    );
    assert_eq!(drawing.dimension_style("ISO").map(|s| s.size), Some(2.5));
    assert_eq!(drawing.dimension_style("iso"), None);
    assert_ne!(broken[0], Ref::default());
}

#[test]
fn symbols_keep_the_block_key_the_placement_and_the_attributes() {
    let attrib = |tag: &'static str, value: &'static str, flags: &'static str| {
        entity(
            "ATTRIB",
            &[
                (10, "100.0"),
                (20, "50.0"),
                (40, "2.5"),
                (1, value),
                (2, tag),
                (70, flags),
                (72, "1"),
                (11, "100.0"),
                (21, "50.0"),
                (74, "2"),
            ],
        )
    };
    let entities = [
        // 属性の続く記号。2 倍・反転・30 度回し、表示する属性と表示しない属性を持つ
        entity(
            "INSERT",
            &[
                (66, "1"),
                (2, "部品A"),
                (10, "100.0"),
                (20, "50.0"),
                (41, "2.0"),
                (42, "-2.0"),
                (50, "30.0"),
            ],
        ),
        attrib("NO", "A-1", "0"),
        attrib("MAKER", "社名", "1"),
        vec![(0, "SEQEND"), (8, "0")],
        // 3 列 2 行の格子に並べる
        entity(
            "INSERT",
            &[
                (2, "柱"),
                (10, "0.0"),
                (20, "0.0"),
                (70, "3"),
                (71, "2"),
                (44, "6000.0"),
                (45, "4500.0"),
            ],
        ),
        // 尺度が 0
        entity("INSERT", &[(2, "x"), (10, "0.0"), (20, "0.0"), (41, "0.0")]),
        // 続く INSERT のない属性
        attrib("LOST", "?", "0"),
    ]
    .concat();
    let header = [MM, &[(9, "$ACADVER"), (1, "AC1021")]].concat();
    let (drawing, diag) = lift(&dxf(&header, &entities), &mut Budget::default());
    let drawing = drawing.unwrap();
    let params: Vec<_> = found(&diag)
        .into_iter()
        .map(|(c, p, n, _)| (c, p, n))
        .collect();
    assert_eq!(
        params,
        [
            (
                codes::DXF_LIFT_INVALID,
                r#"Text("INSERT") Int(41)"#.to_owned(),
                1
            ),
            (codes::DXF_LIFT_DROPPED, r#"Text("ATTRIB")"#.to_owned(), 1),
            // BLOCKS を持たないので、どの配置も定義を指さない
            (codes::DXF_BLOCK_MISSING, r#"Text("部品A")"#.to_owned(), 1),
            (codes::DXF_BLOCK_MISSING, r#"Text("柱")"#.to_owned(), 1),
        ]
    );
    let shapes = model_shapes(&drawing);
    let [Shape::Symbol(part), Shape::Symbol(columns)] = shapes.as_slice() else {
        panic!("{shapes:?}");
    };
    assert_eq!(part.key, block("部品A"));
    assert_eq!(part.position, at(100.0, 50.0));
    assert_eq!(
        (part.scale, part.rotation, part.grid),
        ([2.0, -2.0], 30.0, None)
    );
    let attributes: Vec<_> = part
        .attributes
        .iter()
        .map(|a| (a.tag.as_str(), a.text.content.as_str(), a.hidden))
        .collect();
    assert_eq!(attributes, [("NO", "A-1", false), ("MAKER", "社名", true)]);
    // 属性の縦の位置合わせは 74 にあり、高さの欄は ATTRIB のもの
    let first = &part.attributes[0].text;
    assert_eq!(
        first.placement,
        Placement::Anchor {
            point: at(100.0, 50.0),
            horizontal: Horizontal::Center,
            vertical: Vertical::Middle,
        }
    );
    assert_eq!(
        first.height.meaning,
        HeightMeaning::Declared("dxf.ATTRIB.40")
    );
    assert_eq!(part.attributes[0].style, by_layer());
    let model = |value: f64| {
        Length::Model(ModelLength {
            space: MODEL,
            value,
        })
    };
    assert_eq!(
        columns.grid,
        Some(Grid {
            columns: 3,
            rows: 2,
            column_spacing: model(6000.0),
            row_spacing: model(4500.0),
        })
    );
    assert_eq!(columns.attributes, []);
}

#[test]
fn hatches_keep_their_boundaries_and_patterns() {
    let entities = [
        entity(
            "LINE",
            &[
                (5, "A1"),
                (10, "0.0"),
                (20, "0.0"),
                (11, "10.0"),
                (21, "0.0"),
            ],
        ),
        // 2 つの輪を持つ ANSI31。外の輪は膨らみのある折れ線で、線分の図形から作った。内の輪は線分・円弧・
        // 楕円弧の辺。島は入れ子に埋める
        entity(
            "HATCH",
            &[
                (100, "AcDbHatch"),
                (10, "0.0"),
                (20, "0.0"),
                (30, "0.0"),
                (2, "ANSI31"),
                (70, "0"),
                (71, "1"),
                (91, "2"),
                (92, "3"),
                (72, "1"),
                (73, "1"),
                (93, "3"),
                (10, "0.0"),
                (20, "0.0"),
                (42, "0.0"),
                (10, "10.0"),
                (20, "0.0"),
                (42, "1.0"),
                (10, "10.0"),
                (20, "10.0"),
                (42, "0.0"),
                (97, "1"),
                (330, "A1"),
                (92, "16"),
                (93, "3"),
                (72, "1"),
                (10, "1.0"),
                (20, "1.0"),
                (11, "2.0"),
                (21, "1.0"),
                (72, "2"),
                (10, "2.0"),
                (20, "2.0"),
                (40, "1.0"),
                (50, "270.0"),
                (51, "90.0"),
                (73, "1"),
                (72, "3"),
                (10, "2.0"),
                (20, "3.0"),
                (11, "1.0"),
                (21, "0.0"),
                (40, "0.5"),
                (50, "0.0"),
                (51, "180.0"),
                (73, "1"),
                (97, "0"),
                (75, "0"),
                (76, "1"),
                (52, "15.0"),
                (41, "2.0"),
                (77, "0"),
                (78, "1"),
                (53, "60.0"),
                (43, "0.0"),
                (44, "0.0"),
                (45, "-2.2"),
                (46, "3.8"),
                (79, "2"),
                (49, "5.0"),
                (49, "-1.0"),
                (98, "1"),
                (10, "5.0"),
                (20, "5.0"),
            ],
        ),
        // 塗り。外の輪だけを埋め、パターンの欄も書く形式
        entity(
            "HATCH",
            &[
                (2, "SOLID"),
                (70, "1"),
                (91, "1"),
                (92, "2"),
                (72, "0"),
                (73, "1"),
                (93, "2"),
                (10, "0.0"),
                (20, "0.0"),
                (10, "1.0"),
                (20, "0.0"),
                (97, "0"),
                (75, "1"),
                (76, "1"),
                (52, "0.0"),
                (41, "1.0"),
                (77, "0"),
                (78, "0"),
                (47, "0.1"),
                (98, "0"),
            ],
        ),
        // 2 色のグラデーション
        entity(
            "HATCH",
            &[
                (2, "SOLID"),
                (70, "1"),
                (91, "0"),
                (75, "2"),
                (76, "1"),
                (98, "0"),
                (450, "1"),
                (451, "0"),
                (452, "0"),
                (453, "2"),
                (463, "0.0"),
                (63, "5"),
                (421, "255"),
                (463, "1.0"),
                (63, "2"),
                (421, "16776960"),
                (460, "0.0"),
                (461, "0.0"),
                (462, "1.0"),
                (470, "LINEAR"),
            ],
        ),
    ]
    .concat();
    let (drawing, diag) = lift(&dxf(MM, &entities), &mut Budget::default());
    let drawing = drawing.unwrap();
    assert_eq!(found(&diag), []);
    let line = drawing.spaces[0].elements[0].source.clone();
    let shapes = model_shapes(&drawing);
    let model = |value: f64| {
        Length::Model(ModelLength {
            space: MODEL,
            value,
        })
    };
    let vertex = |x: f64, y: f64, bulge: f64| Vertex {
        point: at(x, y),
        bulge,
    };
    let edge_loop = |boundary, outermost, sources| HatchLoop {
        boundary,
        external: false,
        outermost,
        derived: false,
        text_box: false,
        sources,
    };
    assert_eq!(
        shapes[1..],
        [
            Shape::Hatch(Hatch {
                loops: vec![
                    HatchLoop {
                        external: true,
                        ..edge_loop(
                            Boundary::Polyline {
                                vertices: vec![
                                    vertex(0.0, 0.0, 0.0),
                                    vertex(10.0, 0.0, 1.0),
                                    vertex(10.0, 10.0, 0.0),
                                ],
                                closed: true,
                            },
                            false,
                            vec![line],
                        )
                    },
                    edge_loop(
                        Boundary::Edges(vec![
                            Edge::Line {
                                start: at(1.0, 1.0),
                                end: at(2.0, 1.0),
                            },
                            Edge::Arc {
                                center: at(2.0, 2.0),
                                radius: model(1.0),
                                start: 270.0,
                                end: 90.0,
                                counterclockwise: true,
                            },
                            Edge::Elliptic {
                                center: at(2.0, 3.0),
                                major: at(3.0, 3.0),
                                ratio: 0.5,
                                start: 0.0,
                                end: 180.0,
                                counterclockwise: true,
                            },
                        ]),
                        true,
                        Vec::new(),
                    ),
                ],
                islands: Islands::Nested,
                fill: Fill::Pattern(Pattern {
                    name: "ANSI31".to_owned(),
                    origin: PatternOrigin::Predefined,
                    angle: 15.0,
                    scale: 2.0,
                    double: false,
                    lines: vec![PatternLine {
                        angle: 60.0,
                        base: [0.0, 0.0],
                        offset: [-2.2, 3.8],
                        dashes: vec![5.0, -1.0],
                    }],
                }),
                associative: true,
                seeds: vec![at(5.0, 5.0)],
            }),
            Shape::Hatch(Hatch {
                loops: vec![edge_loop(
                    Boundary::Polyline {
                        vertices: vec![vertex(0.0, 0.0, 0.0), vertex(1.0, 0.0, 0.0)],
                        closed: true,
                    },
                    false,
                    Vec::new(),
                )],
                islands: Islands::Outer,
                fill: Fill::Solid,
                associative: false,
                seeds: Vec::new(),
            }),
            Shape::Hatch(Hatch {
                loops: Vec::new(),
                islands: Islands::Ignore,
                fill: Fill::Gradient(Gradient {
                    name: "LINEAR".to_owned(),
                    angle: 0.0,
                    shift: 0.0,
                    colors: vec![[0, 0, 255], [255, 255, 0]],
                    tint: None,
                }),
                associative: false,
                seeds: Vec::new(),
            }),
        ]
    );
}

#[test]
fn e9_hatches_and_symbols_that_cannot_be_read_are_reported() {
    let hatch = |loops: &[(i16, &'static str)]| {
        let mut all = vec![(2, "SOLID"), (70, "1"), (91, "1")];
        all.extend_from_slice(loops);
        all.extend_from_slice(&[(75, "0"), (76, "1"), (98, "0")]);
        entity("HATCH", &all)
    };
    let entities = [
        // 制御点も当てはめる点もない SPLINE の辺
        hatch(&[
            (92, "0"),
            (93, "1"),
            (72, "4"),
            (94, "3"),
            (73, "0"),
            (74, "0"),
            (95, "0"),
            (96, "0"),
            (97, "0"),
        ]),
        // 境界を作った図形の数が合わない
        hatch(&[
            (92, "2"),
            (72, "0"),
            (73, "1"),
            (93, "1"),
            (10, "0.0"),
            (20, "0.0"),
            (97, "2"),
        ]),
        // 辺の種類がない
        hatch(&[(92, "0"), (93, "1"), (72, "9"), (97, "0")]),
        // 標高が 0 でない
        entity("HATCH", &[(30, "1.0"), (70, "1"), (91, "0")]),
        // 名前のない記号
        entity("INSERT", &[(10, "0.0"), (20, "0.0")]),
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
    assert_eq!(
        params,
        [
            invalid("HATCH", 10),
            invalid("HATCH", 97),
            invalid("HATCH", 72),
            (
                codes::DXF_NON_PLANAR_DROPPED,
                r#"Text("HATCH")"#.to_owned(),
                1
            ),
            invalid("INSERT", 2),
        ]
    );
}

fn model(value: f64) -> Length {
    Length::Model(ModelLength {
        space: MODEL,
        value,
    })
}

fn span(piece: Piece, format: Format) -> Span {
    Span { piece, format }
}

fn words(text: &str) -> Piece {
    Piece::Text(text.to_owned())
}

/// 対応表の E3 のうち書式と配置: MTEXT と複数行の属性
#[test]
fn e3_formatted_text_keeps_the_content_the_spans_and_the_frame() {
    let entities = [
        // 縦書きで、枠の中央に置き、X 軸を上に向け、色で塗って枠の線を描く
        entity(
            "MTEXT",
            &[
                (10, "5.0"),
                (20, "6.0"),
                (40, "2.5"),
                (41, "100.0"),
                (71, "5"),
                (72, "3"),
                (3, "前半{\\L"),
                (1, "下線}\\P後半\\pxi-3;"),
                (7, "明朝"),
                (11, "0.0"),
                (21, "1.0"),
                (73, "2"),
                (44, "1.5"),
                (90, "17"),
                (63, "1"),
                (45, "1.2"),
                (441, "0"),
            ],
        ),
        // 用紙の上の MTEXT
        entity(
            "MTEXT",
            &[(67, "1"), (10, "10.0"), (20, "20.0"), (40, "3.5"), (1, "A")],
        ),
        // 複数行の属性。埋め込みの印の前の 71 と 72 は、1 行の値の欄にしない
        entity("INSERT", &[(66, "1"), (2, "枠"), (10, "0.0"), (20, "0.0")]),
        entity(
            "ATTRIB",
            &[
                (10, "0.0"),
                (20, "0.0"),
                (40, "2.5"),
                (1, "一行目"),
                (2, "NOTE"),
                (71, "2"),
                (72, "0"),
                (101, "Embedded Object"),
                (10, "1.0"),
                (20, "2.0"),
                (40, "2.5"),
                (41, "50.0"),
                (71, "1"),
                (72, "1"),
                (1, "一行目\\P二行目"),
            ],
        ),
        vec![(0, "SEQEND"), (8, "0")],
    ]
    .concat();
    let header = [MM, &[(9, "$ACADVER"), (1, "AC1021")]].concat();
    let (drawing, diag) = lift(&dxf(&header, &entities), &mut Budget::default());
    // BLOCKS を持たないので、配置は定義を指さない
    let missing: Vec<_> = found(&diag)
        .into_iter()
        .map(|(c, p, _, _)| (c, p))
        .collect();
    assert_eq!(
        missing,
        [(codes::DXF_BLOCK_MISSING, r#"Text("枠")"#.to_owned())]
    );
    let drawing = drawing.unwrap();
    let shapes = model_shapes(&drawing);
    let [Shape::Formatted(vertical), Shape::Symbol(frame)] = shapes.as_slice() else {
        panic!("{shapes:?}");
    };
    let underline = Format {
        underline: true,
        ..Format::default()
    };
    assert_eq!(
        *vertical,
        FormattedText {
            content: "前半{\\L下線}\\P後半\\pxi-3;".to_owned(),
            spans: vec![
                span(words("前半"), Format::default()),
                span(words("下線"), underline),
                span(Piece::Paragraph, Format::default()),
                span(words("後半"), Format::default()),
                span(Piece::Unparsed("\\pxi-3;".to_owned()), Format::default()),
            ],
            height: Height {
                value: model(2.5),
                meaning: HeightMeaning::Declared("dxf.MTEXT.40"),
            },
            point: at(5.0, 6.0),
            horizontal: Horizontal::Center,
            vertical: Vertical::Middle,
            rotation: Rotation::Direction([0.0, 1.0]),
            width: Some(model(100.0)),
            frame_height: None,
            flow: Flow::TopToBottom,
            line_spacing: LineSpacing {
                factor: 1.5,
                exact: true,
            },
            background: Some(Background {
                fill: Some(BackgroundFill::Color(Color {
                    code: Some(ColorCode {
                        system: ColorSystem::Aci,
                        value: 1,
                    }),
                    rgb: None,
                })),
                frame: true,
                scale: 1.2,
                transparency: Some(0),
            }),
            style: "明朝".to_owned(),
        }
    );
    let Shape::Formatted(paper) = &drawing.sheets[0].elements[0].shape else {
        panic!("{:?}", drawing.sheets[0].elements);
    };
    assert_eq!(paper.point, on(10.0, 20.0));
    assert_eq!(paper.height.value, Length::Paper(PaperLength(3.5)));
    assert_eq!(
        (paper.rotation, paper.flow, paper.background),
        (Rotation::Angle(0.0), Flow::LeftToRight, None)
    );
    let [note] = frame.attributes.as_slice() else {
        panic!("{:?}", frame.attributes);
    };
    // 1 行の値は、属性の種類を TEXT の欄と取り違えない
    assert_eq!(note.text.content, "一行目");
    assert!(!note.text.backward);
    assert_eq!(
        note.text.placement,
        Placement::Anchor {
            point: at(0.0, 0.0),
            horizontal: Horizontal::Left,
            vertical: Vertical::Baseline,
        }
    );
    let formatted = note.formatted.as_ref().unwrap();
    assert_eq!(
        formatted.spans,
        [
            span(words("一行目"), Format::default()),
            span(Piece::Paragraph, Format::default()),
            span(words("二行目"), Format::default()),
        ]
    );
    assert_eq!(
        (formatted.point, formatted.horizontal, formatted.vertical),
        (at(1.0, 2.0), Horizontal::Left, Vertical::Top)
    );
    assert_eq!(formatted.width, Some(model(50.0)));
}

#[test]
fn e9_formatted_text_that_cannot_be_read_is_reported() {
    let mtext = |tags: &[(i16, &'static str)]| {
        let mut all = vec![(10, "0.0"), (20, "0.0"), (40, "2.5")];
        all.extend_from_slice(tags);
        entity("MTEXT", &all)
    };
    let entities = [
        // 段組み
        mtext(&[(1, "a"), (75, "1")]),
        mtext(&[(1, "a"), (101, "Embedded Object"), (70, "1")]),
        // 位置合わせ・内容の順・背景の値が読めない
        mtext(&[(1, "a"), (71, "10")]),
        mtext(&[(1, "a"), (3, "b")]),
        mtext(&[(3, "a")]),
        mtext(&[(1, "a"), (90, "4")]),
        mtext(&[(1, "a"), (90, "1")]),
    ]
    .concat();
    let (drawing, diag) = lift(&dxf(MM, &entities), &mut Budget::default());
    assert_eq!(drawing.unwrap().spaces[0].elements, []);
    let params: Vec<_> = found(&diag)
        .into_iter()
        .map(|(c, p, n, _)| (c, p, n))
        .collect();
    let invalid = |code: i64| {
        (
            codes::DXF_LIFT_INVALID,
            format!("Text(\"MTEXT\") Int({code})"),
            1,
        )
    };
    assert_eq!(
        params,
        [
            (codes::DXF_MTEXT_COLUMNS_UNSUPPORTED, String::new(), 2),
            invalid(71),
            invalid(3),
            invalid(1),
            invalid(90),
            invalid(63),
        ]
    );
}

#[test]
fn e9_formatted_text_counts_its_chunks_as_work() {
    let run = |chunks: &[(i16, &'static str)], limit: u64| {
        let mut tags = vec![(10, "0.0"), (20, "0.0"), (40, "2.5")];
        tags.extend_from_slice(chunks);
        tags.push((1, "c"));
        let src = dxf(MM, &entity("MTEXT", &tags));
        let mut budget = Budget::new(Limits::default().with(Kind::Work, Limit::Finite(limit)));
        lift(&src, &mut budget).0.is_ok()
    };
    let least = (1..100).find(|limit| run(&[], *limit)).unwrap();
    let chunks = [(3, "a"), (3, "b")];
    assert!(!run(&chunks, least + 1));
    assert!(run(&chunks, least + 2));
}

/// 引出線つきの注記の値。文脈の中に注記と枝を置く
fn multileader(
    entity: &[(i16, &'static str)],
    context: &[(i16, &'static str)],
    leaders: &[(i16, &'static str)],
) -> Vec<(i16, &'static str)> {
    let mut all = vec![
        (0, "MULTILEADER"),
        (5, "M1"),
        (8, "0"),
        (300, "CONTEXT_DATA{"),
    ];
    all.extend_from_slice(context);
    all.extend_from_slice(leaders);
    all.push((301, "}"));
    all.extend_from_slice(entity);
    all
}

/// 対応表の E4 のうち引出線とバルーン: MULTILEADER の枝・線・折れ曲がりと、文字か記号の注記
#[test]
fn e4_callouts_keep_their_branches_and_content() {
    // 2 本の線が 1 点に集まり、右へ折れ曲がる枝
    let branch: &[(i16, &str)] = &[
        (302, "LEADER{"),
        (290, "1"),
        (291, "1"),
        (10, "30.0"),
        (20, "20.0"),
        (30, "0.0"),
        (11, "1.0"),
        (21, "0.0"),
        (31, "0.0"),
        (90, "0"),
        (40, "5.0"),
        (304, "LEADER_LINE{"),
        (10, "0.0"),
        (20, "0.0"),
        (30, "0.0"),
        (91, "0"),
        (305, "}"),
        (304, "LEADER_LINE{"),
        (10, "0.0"),
        (20, "10.0"),
        (30, "0.0"),
        (10, "10.0"),
        (20, "15.0"),
        (30, "0.0"),
        (91, "1"),
        (305, "}"),
        (303, "}"),
    ];
    let text = multileader(
        &[(170, "2"), (342, "0"), (42, "2.5"), (290, "1"), (172, "2")],
        &[
            (40, "2.0"),
            (10, "36.0"),
            (20, "20.0"),
            (30, "0.0"),
            (41, "3.5"),
            (140, "1.0"),
            (290, "1"),
            (304, "注記\\P2 行目"),
            (12, "36.0"),
            (22, "21.0"),
            (32, "0.0"),
            (13, "1.0"),
            (23, "0.0"),
            (33, "0.0"),
            (42, "0.0"),
            (43, "0.0"),
            (44, "0.0"),
            (45, "1.0"),
            (170, "1"),
            (171, "1"),
            (172, "1"),
            (292, "1"),
            (291, "0"),
            (91, "-1023410175"),
            (141, "1.5"),
            (92, "0"),
            (173, "0"),
            (296, "0"),
        ],
        branch,
    );
    // 記号（バルーン）の注記。属性の値は、定義の出所で指す
    let balloon = multileader(
        &[(172, "1"), (330, "AD"), (177, "1"), (44, "0.0"), (302, "1")],
        &[
            (40, "1.0"),
            (140, "2.0"),
            (290, "0"),
            (296, "1"),
            (341, "B1"),
            (15, "50.0"),
            (25, "60.0"),
            (35, "0.0"),
            (16, "2.0"),
            (26, "2.0"),
            (36, "1.0"),
            (46, "0.0"),
        ],
        &[],
    );
    let blocks = [
        (0, "BLOCK_RECORD"),
        (5, "B1"),
        (2, "_TagCircle"),
        (0, "BLOCK"),
        (5, "B2"),
        (330, "B1"),
        (8, "0"),
        (2, "_TagCircle"),
        (70, "2"),
        (10, "0.0"),
        (20, "0.0"),
        (0, "ATTDEF"),
        (5, "AD"),
        (330, "B1"),
        (8, "0"),
        (10, "0.0"),
        (20, "0.0"),
        (40, "1.0"),
        (1, ""),
        (2, "NO"),
        (3, "番号"),
        (70, "0"),
        (0, "ENDBLK"),
        (5, "B3"),
        (330, "B1"),
        (8, "0"),
    ];
    let header = [MM, &[(9, "$ACADVER"), (1, "AC1021")]].concat();
    let src = sections(&[
        ("HEADER", &header),
        ("TABLES", &blocks[..3]),
        ("BLOCKS", &blocks[3..]),
        ("ENTITIES", &[text, balloon].concat()),
    ]);
    let (drawing, diag) = lift(&src, &mut Budget::default());
    assert_eq!(found(&diag), []);
    let drawing = drawing.unwrap();
    let shapes = model_shapes(&drawing);
    let [Shape::Callout(text), Shape::Callout(balloon)] = shapes.as_slice() else {
        panic!("{shapes:?}");
    };
    assert_eq!(
        text.branches,
        [Branch {
            lines: vec![vec![at(0.0, 0.0)], vec![at(0.0, 10.0), at(10.0, 15.0)]],
            end: Some(at(30.0, 20.0)),
            dogleg: Some(Dogleg {
                direction: [1.0, 0.0],
                length: model(5.0),
            }),
        }]
    );
    assert_eq!(
        (
            text.path,
            &text.arrowhead,
            text.arrow_size,
            text.landing,
            text.scale
        ),
        (PathKind::Spline, &Arrowhead::Default, model(2.5), true, 2.0)
    );
    let CalloutContent::Text(note) = &text.content else {
        panic!("{:?}", text.content);
    };
    assert_eq!(note.content, "注記\\P2 行目");
    assert_eq!(
        note.spans,
        [
            span(words("注記"), Format::default()),
            span(Piece::Paragraph, Format::default()),
            span(words("2 行目"), Format::default()),
        ]
    );
    assert_eq!(
        (note.point, note.horizontal, note.vertical, note.rotation),
        (
            at(36.0, 21.0),
            Horizontal::Left,
            Vertical::Top,
            Rotation::Direction([1.0, 0.0])
        )
    );
    assert_eq!(
        note.height.meaning,
        HeightMeaning::Declared("dxf.MULTILEADER.41")
    );
    // 0xC3 は色の番号
    assert_eq!(
        note.background.map(|b| b.fill),
        Some(Some(BackgroundFill::Color(Color {
            code: Some(ColorCode {
                system: ColorSystem::Aci,
                value: 1,
            }),
            rgb: None,
        })))
    );
    assert_eq!(balloon.branches, []);
    // 端部記号の大きさは、図形になければ文脈の 140
    assert_eq!(balloon.arrow_size, model(2.0));
    let CalloutContent::Symbol { symbol, values } = &balloon.content else {
        panic!("{:?}", balloon.content);
    };
    assert_eq!(symbol.key, block("_TagCircle"));
    assert_eq!(
        (symbol.position, symbol.scale, symbol.rotation),
        (at(50.0, 60.0), [2.0, 2.0], 0.0)
    );
    let [value] = values.as_slice() else {
        panic!("{values:?}");
    };
    assert_eq!(value.text, "1");
    // 定義は BLOCKS の ATTDEF を指す
    assert!(value.definition.record().is_some());
}

#[test]
fn e9_callouts_that_cannot_be_read_are_reported() {
    let entities = [
        // 文脈の区切りが閉じない
        vec![
            (0, "MULTILEADER"),
            (8, "0"),
            (300, "CONTEXT_DATA{"),
            (40, "1.0"),
        ],
        // 文脈がない
        entity("MULTILEADER", &[(170, "1")]),
        // 公差の注記
        multileader(&[(172, "3"), (42, "1.0")], &[], &[]),
        // 線の頂点がない
        multileader(
            &[(42, "1.0")],
            &[],
            &[
                (302, "LEADER{"),
                (304, "LEADER_LINE{"),
                (305, "}"),
                (303, "}"),
            ],
        ),
        // 段組みの文字
        multileader(
            &[(42, "1.0")],
            &[
                (290, "1"),
                (304, "a"),
                (41, "1.0"),
                (12, "0.0"),
                (22, "0.0"),
                (173, "1"),
            ],
            &[],
        ),
    ]
    .concat();
    let (drawing, diag) = lift(&dxf(MM, &entities), &mut Budget::default());
    assert_eq!(drawing.unwrap().spaces[0].elements, []);
    let params: Vec<_> = found(&diag)
        .into_iter()
        .map(|(c, p, n, _)| (c, p, n))
        .collect();
    let invalid = |code: i64| {
        (
            codes::DXF_LIFT_INVALID,
            format!("Text(\"MULTILEADER\") Int({code})"),
            1,
        )
    };
    assert_eq!(
        params,
        [
            invalid(301),
            invalid(300),
            invalid(172),
            invalid(10),
            (codes::DXF_MTEXT_COLUMNS_UNSUPPORTED, String::new(), 1),
        ]
    );
}

#[test]
fn e8_curves_keep_their_definition() {
    let entities = [
        entity("CIRCLE", &[(10, "1.0"), (20, "2.0"), (40, "3.0")]),
        entity(
            "ARC",
            &[
                (10, "0.0"),
                (20, "0.0"),
                (40, "5.0"),
                (50, "0.0"),
                (51, "90.0"),
            ],
        ),
        // 始まりと終わりのパラメーターはラジアン
        entity(
            "ELLIPSE",
            &[
                (10, "1.0"),
                (20, "1.0"),
                (11, "4.0"),
                (21, "0.0"),
                (40, "0.5"),
                (41, "0.0"),
                (42, "3.141592653589793"),
            ],
        ),
        // 閉じていて、最初の頂点だけが幅と膨らみを持つ
        entity(
            "LWPOLYLINE",
            &[
                (90, "3"),
                (70, "1"),
                (10, "0.0"),
                (20, "0.0"),
                (40, "1.0"),
                (41, "2.0"),
                (42, "1.0"),
                (10, "10.0"),
                (20, "0.0"),
                (10, "10.0"),
                (20, "10.0"),
            ],
        ),
        // 一様な幅（43）を持つ開いた折れ線
        entity(
            "LWPOLYLINE",
            &[
                (90, "2"),
                (70, "0"),
                (43, "0.5"),
                (10, "0.0"),
                (20, "0.0"),
                (10, "1.0"),
                (20, "1.0"),
            ],
        ),
        // 有理の 2 次。節点・制御点・重み
        entity(
            "SPLINE",
            &[
                (70, "4"),
                (71, "2"),
                (72, "6"),
                (73, "3"),
                (74, "0"),
                (40, "0.0"),
                (40, "0.0"),
                (40, "0.0"),
                (40, "1.0"),
                (40, "1.0"),
                (40, "1.0"),
                (10, "0.0"),
                (20, "0.0"),
                (30, "0.0"),
                (10, "5.0"),
                (20, "5.0"),
                (30, "0.0"),
                (10, "10.0"),
                (20, "0.0"),
                (30, "0.0"),
                (41, "1.0"),
                (41, "0.5"),
                (41, "1.0"),
            ],
        ),
        // 当てはめる点と接線だけを持つ、閉じた 3 次
        entity(
            "SPLINE",
            &[
                (70, "1"),
                (71, "3"),
                (72, "0"),
                (73, "0"),
                (74, "2"),
                (12, "1.0"),
                (22, "0.0"),
                (32, "0.0"),
                (13, "0.0"),
                (23, "1.0"),
                (33, "0.0"),
                (11, "0.0"),
                (21, "0.0"),
                (31, "0.0"),
                (11, "10.0"),
                (21, "10.0"),
                (31, "0.0"),
            ],
        ),
    ]
    .concat();
    let (drawing, diag) = lift(&dxf(MM, &entities), &mut Budget::default());
    assert_eq!(found(&diag), []);
    let width = |start, end| [model(start), model(end)];
    assert_eq!(
        model_shapes(&drawing.unwrap()),
        [
            Shape::Circle {
                center: at(1.0, 2.0),
                radius: model(3.0),
            },
            Shape::Curve(Edge::Arc {
                center: at(0.0, 0.0),
                radius: model(5.0),
                start: 0.0,
                end: 90.0,
                counterclockwise: true,
            }),
            Shape::Curve(Edge::Elliptic {
                center: at(1.0, 1.0),
                major: at(5.0, 1.0),
                ratio: 0.5,
                start: 0.0,
                end: 180.0,
                counterclockwise: true,
            }),
            Shape::Polyline(Polyline {
                vertices: vec![
                    Vertex {
                        point: at(0.0, 0.0),
                        bulge: 1.0,
                    },
                    Vertex {
                        point: at(10.0, 0.0),
                        bulge: 0.0,
                    },
                    Vertex {
                        point: at(10.0, 10.0),
                        bulge: 0.0,
                    },
                ],
                closed: true,
                width: None,
                vertex_widths: vec![width(1.0, 2.0), width(0.0, 0.0), width(0.0, 0.0)],
            }),
            Shape::Polyline(Polyline {
                vertices: vec![
                    Vertex {
                        point: at(0.0, 0.0),
                        bulge: 0.0,
                    },
                    Vertex {
                        point: at(1.0, 1.0),
                        bulge: 0.0,
                    },
                ],
                closed: false,
                width: Some(model(0.5)),
                vertex_widths: vec![],
            }),
            Shape::Curve(Edge::Spline(Spline {
                degree: 2,
                knots: vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
                control: vec![at(0.0, 0.0), at(5.0, 5.0), at(10.0, 0.0)],
                weights: vec![1.0, 0.5, 1.0],
                fit: vec![],
                start_tangent: None,
                end_tangent: None,
                closed: false,
                periodic: false,
            })),
            Shape::Curve(Edge::Spline(Spline {
                degree: 3,
                knots: vec![],
                control: vec![],
                weights: vec![],
                fit: vec![at(0.0, 0.0), at(10.0, 10.0)],
                start_tangent: Some([1.0, 0.0]),
                end_tangent: Some([0.0, 1.0]),
                closed: true,
                periodic: false,
            })),
        ]
    );
}

/// SPLINE の辺 1 つの輪を持つ HATCH
fn spline_hatch(fit: &[(i16, &'static str)]) -> Vec<(i16, &'static str)> {
    let mut tags = vec![
        (2, "SOLID"),
        (70, "1"),
        (91, "1"),
        (92, "1"),
        (93, "1"),
        (72, "4"),
        (94, "1"),
        (73, "1"),
        (74, "0"),
        (95, "4"),
        (96, "2"),
        (40, "0.0"),
        (40, "0.0"),
        (40, "1.0"),
        (40, "1.0"),
        (10, "0.0"),
        (20, "0.0"),
        (42, "1.0"),
        (10, "10.0"),
        (20, "0.0"),
        (42, "2.0"),
    ];
    tags.extend_from_slice(fit);
    tags.extend_from_slice(&[(97, "0"), (75, "0"), (76, "1"), (98, "0")]);
    entity("HATCH", &tags)
}

#[test]
fn e8_hatch_spline_edges_read_fit_data_from_2010() {
    let edges = |header: &[(i16, &str)], fit: &[(i16, &'static str)]| {
        let (drawing, diag) = lift(&dxf(header, &spline_hatch(fit)), &mut Budget::default());
        assert_eq!(found(&diag), []);
        match model_shapes(&drawing.unwrap()).as_slice() {
            [Shape::Hatch(hatch)] => match hatch.loops.as_slice() {
                [
                    HatchLoop {
                        boundary: Boundary::Edges(edges),
                        ..
                    },
                ] => edges.clone(),
                other => panic!("{other:?}"),
            },
            other => panic!("{other:?}"),
        }
    };
    let spline = |fit, start_tangent| {
        vec![Edge::Spline(Spline {
            degree: 1,
            knots: vec![0.0, 0.0, 1.0, 1.0],
            control: vec![at(0.0, 0.0), at(10.0, 0.0)],
            weights: vec![1.0, 2.0],
            fit,
            start_tangent,
            end_tangent: None,
            closed: false,
            periodic: false,
        })]
    };
    // 2010 より前は、当てはめる点を持たない。続く 97 は、境界を作った図形の数
    assert_eq!(edges(MM, &[]), spline(vec![], None));
    let header = [MM, &[(9, "$ACADVER"), (1, "AC1024")]].concat();
    assert_eq!(
        edges(
            &header,
            &[
                (97, "1"),
                (11, "5.0"),
                (21, "0.0"),
                (12, "1.0"),
                (22, "0.0")
            ]
        ),
        spline(vec![at(5.0, 0.0)], Some([1.0, 0.0]))
    );
}

#[test]
fn e9_curves_that_cannot_be_read_are_reported() {
    let entities = [
        // 半径が 0
        entity("CIRCLE", &[(10, "0.0"), (20, "0.0"), (40, "0.0")]),
        // 比が 1 より大きい
        entity(
            "ELLIPSE",
            &[
                (10, "0.0"),
                (20, "0.0"),
                (11, "1.0"),
                (21, "0.0"),
                (40, "2.0"),
            ],
        ),
        // 頂点の数が合わない
        entity("LWPOLYLINE", &[(90, "2"), (10, "0.0"), (20, "0.0")]),
        // 標高が 0 でない
        entity(
            "LWPOLYLINE",
            &[(90, "1"), (38, "1.0"), (10, "0.0"), (20, "0.0")],
        ),
        // 節点の数が制御点の数と次数に合わない
        entity(
            "SPLINE",
            &[
                (71, "1"),
                (40, "0.0"),
                (40, "1.0"),
                (10, "0.0"),
                (20, "0.0"),
                (10, "1.0"),
                (20, "0.0"),
            ],
        ),
        // 制御点が次数より多くない
        entity("SPLINE", &[(71, "3"), (10, "0.0"), (20, "0.0")]),
        // 数えた節点の数（72）が合わない
        entity("SPLINE", &[(71, "1"), (72, "3"), (11, "0.0"), (21, "0.0")]),
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
    assert_eq!(
        params,
        [
            invalid("CIRCLE", 40),
            invalid("ELLIPSE", 40),
            invalid("LWPOLYLINE", 90),
            (
                codes::DXF_NON_PLANAR_DROPPED,
                r#"Text("LWPOLYLINE")"#.to_owned(),
                1
            ),
            invalid("SPLINE", 40),
            invalid("SPLINE", 10),
            invalid("SPLINE", 72),
        ]
    );
}

#[test]
fn e8_symbol_definitions_keep_their_contents_apart_from_placements() {
    let blocks = [
        // 空間のブロックは、記号の定義にしない
        entity(
            "BLOCK",
            &[(2, "*Model_Space"), (70, "0"), (10, "0.0"), (20, "0.0")],
        ),
        entity("ENDBLK", &[]),
        // 基点を持ち、線分と円と属性の定義を中身に持つ
        entity("BLOCK", &[(2, "DOOR"), (70, "2"), (10, "5.0"), (20, "5.0")]),
        entity(
            "LINE",
            &[(10, "0.0"), (20, "0.0"), (11, "10.0"), (21, "0.0")],
        ),
        entity(
            "ATTDEF",
            &[
                (10, "1.0"),
                (20, "2.0"),
                (40, "2.5"),
                (1, "900"),
                (2, "WIDTH"),
                (3, "Width?"),
                (70, "10"),
            ],
        ),
        entity("CIRCLE", &[(10, "5.0"), (20, "5.0"), (40, "1.0")]),
        entity("ENDBLK", &[]),
        // 名前のない定義
        entity("BLOCK", &[(2, "*U1"), (70, "1"), (10, "0.0"), (20, "0.0")]),
        entity(
            "LINE",
            &[(10, "0.0"), (20, "0.0"), (11, "1.0"), (21, "1.0")],
        ),
        entity("ENDBLK", &[]),
        // 外部参照。外部のファイルは読まない
        entity(
            "BLOCK",
            &[
                (2, "XREF"),
                (70, "4"),
                (10, "0.0"),
                (20, "0.0"),
                (1, "parts.dxf"),
            ],
        ),
        entity("ENDBLK", &[]),
    ]
    .concat();
    // 同じ定義の 2 つの配置
    let entities = [
        entity("INSERT", &[(2, "DOOR"), (10, "100.0"), (20, "0.0")]),
        entity(
            "INSERT",
            &[(2, "DOOR"), (10, "200.0"), (20, "0.0"), (50, "90.0")],
        ),
    ]
    .concat();
    let src = sections(&[("HEADER", MM), ("BLOCKS", &blocks), ("ENTITIES", &entities)]);
    let (drawing, diag) = lift(&src, &mut Budget::default());
    assert_eq!(found(&diag), []);
    let drawing = drawing.unwrap();
    let keys: Vec<_> = drawing
        .symbols
        .iter()
        .map(|s| s.key.name.as_str())
        .collect();
    assert_eq!(keys, ["DOOR", "*U1", "XREF"]);
    let placed: Vec<_> = model_shapes(&drawing)
        .into_iter()
        .map(|shape| match shape {
            Shape::Symbol(symbol) => (symbol.key, symbol.position),
            other => panic!("{other:?}"),
        })
        .collect();
    assert_eq!(
        placed,
        [
            (block("DOOR"), at(100.0, 0.0)),
            (block("DOOR"), at(200.0, 0.0)),
        ]
    );
    // 中身は、図面の実寸の空間とは別の、定義ごとの空間の点で持つ
    let door = drawing.symbol(&block("DOOR")).unwrap();
    let local = |x: f64, y: f64| ModelPoint {
        space: SpaceId(2),
        x,
        y,
    };
    assert_eq!(door.space.id, SpaceId(2));
    assert_eq!(door.space.unit, Unit::Millimeter);
    assert_eq!(door.base, Some(local(5.0, 5.0)));
    assert_eq!(door.source, Ref::at_record(5));
    let shapes: Vec<_> = door
        .space
        .elements
        .iter()
        .map(|e| e.shape.clone())
        .collect();
    assert_eq!(
        shapes,
        [
            Shape::Line {
                start: local(0.0, 0.0),
                end: local(10.0, 0.0),
            },
            Shape::Circle {
                center: local(5.0, 5.0),
                radius: Length::Model(ModelLength {
                    space: SpaceId(2),
                    value: 1.0,
                }),
            },
        ]
    );
    assert_eq!(door.space.order, [0, 1]);
    let [width] = door.attributes.as_slice() else {
        panic!("{:?}", door.attributes);
    };
    assert_eq!(
        (
            width.attribute.tag.as_str(),
            width.attribute.text.content.as_str(),
            width.prompt.as_str()
        ),
        ("WIDTH", "900", "Width?")
    );
    assert_eq!(
        (
            width.constant,
            width.verify,
            width.preset,
            width.attribute.hidden
        ),
        (true, false, true, false)
    );
    assert!(!door.anonymous && door.external.is_none());
    let [_, anonymous, external] = drawing.symbols.as_slice() else {
        panic!();
    };
    assert_eq!(anonymous.space.id, SpaceId(3));
    assert!(anonymous.anonymous);
    assert_eq!(external.external.as_deref(), Some("parts.dxf"));
    assert_eq!(external.space.elements, []);
}

#[test]
fn e9_symbol_definitions_that_cannot_be_read_are_reported() {
    let blocks = [
        // BLOCK の外の図形は、どの定義のものか分からない
        entity(
            "LINE",
            &[(10, "0.0"), (20, "0.0"), (11, "1.0"), (21, "1.0")],
        ),
        // 名前のない BLOCK は、中身ごと移さない
        entity("BLOCK", &[(70, "0"), (10, "0.0"), (20, "0.0")]),
        entity("ENDBLK", &[]),
        // まだ移さない種類の中身は、定義を残して落とす
        entity("BLOCK", &[(2, "MARK"), (70, "0"), (10, "0.0"), (20, "0.0")]),
        entity("POINT", &[(10, "0.0"), (20, "0.0")]),
        entity("ENDBLK", &[]),
    ]
    .concat();
    let src = sections(&[("HEADER", MM), ("BLOCKS", &blocks), ("ENTITIES", &[])]);
    let (drawing, diag) = lift(&src, &mut Budget::default());
    let drawing = drawing.unwrap();
    assert_eq!(drawing.symbols.len(), 1);
    assert_eq!(drawing.symbols[0].space.elements, []);
    let record = |at: u64| vec![Ref::at_record(at)];
    assert_eq!(
        found(&diag),
        [
            (
                codes::DXF_LIFT_INVALID,
                r#"Text("LINE") Int(330)"#.to_owned(),
                1,
                record(3)
            ),
            (
                codes::DXF_LIFT_INVALID,
                r#"Text("BLOCK") Int(2)"#.to_owned(),
                1,
                record(4)
            ),
            (
                codes::DXF_LIFT_DROPPED,
                r#"Text("POINT")"#.to_owned(),
                1,
                record(7)
            ),
        ]
    );
}

/// 行の高さ・列の幅とセルを持つ ACAD_TABLE
fn table(sizes: &[(i16, &'static str)], cells: &[(i16, &'static str)]) -> Vec<(i16, &'static str)> {
    let mut tags = vec![
        (100, "AcDbBlockReference"),
        (2, "*T1"),
        (10, "0.0"),
        (20, "100.0"),
        (100, "AcDbTable"),
        (280, "0"),
        (11, "1.0"),
        (21, "0.0"),
        (90, "22"),
    ];
    tags.extend_from_slice(sizes);
    tags.extend_from_slice(cells);
    entity("ACAD_TABLE", &tags)
}

#[test]
fn tables_dimensions_and_styles_report_blocks_without_definitions() {
    let tables = [
        (0, "TABLE"),
        (2, "BLOCK_RECORD"),
        (0, "BLOCK_RECORD"),
        (5, "1A"),
        (2, "_DOT"),
        (0, "BLOCK_RECORD"),
        (5, "B1"),
        (2, "MARK"),
        (0, "ENDTAB"),
        (0, "TABLE"),
        (2, "DIMSTYLE"),
        (0, "DIMSTYLE"),
        (105, "20"),
        (2, "Dotted"),
        (342, "1A"),
        (0, "ENDTAB"),
    ];
    // MARK だけを定義する
    let blocks = [
        (0, "BLOCK"),
        (5, "B2"),
        (330, "B1"),
        (8, "0"),
        (2, "MARK"),
        (70, "0"),
        (10, "0.0"),
        (20, "0.0"),
        (0, "ENDBLK"),
        (5, "B3"),
        (330, "B1"),
        (8, "0"),
    ];
    // 表を描いた *T1 と、寸法を描いた *D1 は定義がない。セルの MARK は定義がある
    let mut entities = table(
        &[(91, "1"), (92, "1"), (141, "5.0"), (142, "5.0")],
        &[(171, "2"), (340, "B1")],
    );
    entities.extend(entity(
        "DIMENSION",
        &[
            (2, "*D1"),
            (10, "0.0"),
            (20, "100.0"),
            (70, "0"),
            (13, "0.0"),
            (23, "0.0"),
            (14, "1000.0"),
            (24, "0.0"),
        ],
    ));
    let src = sections(&[
        ("HEADER", MM),
        ("TABLES", &tables),
        ("BLOCKS", &blocks),
        ("ENTITIES", &entities),
    ]);
    let (drawing, diag) = lift(&src, &mut Budget::default());
    let drawing = drawing.unwrap();
    let style = drawing.dimension_styles[0].source.clone();
    let shapes = &drawing.spaces[0].elements;
    let missing = |name: &str, place: &Ref| {
        (
            codes::DXF_BLOCK_MISSING,
            format!("Text({name:?})"),
            1,
            vec![place.clone()],
        )
    };
    assert_eq!(
        found(&diag),
        [
            missing("*T1", &shapes[0].source),
            missing("*D1", &shapes[1].source),
            missing("_DOT", &style),
        ]
    );
}

#[test]
fn old_polylines_become_polylines() {
    let vertex = |tags: &[(i16, &'static str)]| entity("VERTEX", tags);
    let polyline = |flags: &'static str,
                    extra: &[(i16, &'static str)],
                    vertices: &[Vec<(i16, &'static str)>]| {
        let mut tags = vec![(66, "1"), (10, "0.0"), (20, "0.0"), (70, flags)];
        tags.extend_from_slice(extra);
        let mut all = entity("POLYLINE", &tags);
        for v in vertices {
            all.extend(v.iter().copied());
        }
        all.extend(entity("SEQEND", &[]));
        all
    };
    let entities = [
        // 閉じていて、最初の頂点だけが幅と膨らみを持つ。既定の幅（40）は使わない
        polyline(
            "1",
            &[(40, "9.0")],
            &[
                vertex(&[
                    (10, "0.0"),
                    (20, "0.0"),
                    (40, "1.0"),
                    (41, "2.0"),
                    (42, "1.0"),
                ]),
                vertex(&[(10, "10.0"), (20, "0.0")]),
                vertex(&[(10, "10.0"), (20, "10.0")]),
            ],
        ),
        // どの頂点も同じ幅なので、一定の幅にする
        polyline(
            "0",
            &[],
            &[
                vertex(&[(10, "0.0"), (20, "0.0"), (40, "0.5"), (41, "0.5")]),
                vertex(&[(10, "1.0"), (20, "1.0"), (40, "0.5"), (41, "0.5")]),
            ],
        ),
        // Z が 0 の 3D の折れ線
        polyline(
            "8",
            &[],
            &[
                vertex(&[(10, "0.0"), (20, "0.0"), (30, "0.0"), (70, "32")]),
                vertex(&[(10, "2.0"), (20, "3.0"), (30, "0.0"), (70, "32")]),
            ],
        ),
        // 曲線で当てはめたものと、網は移さない
        polyline("2", &[], &[vertex(&[(10, "0.0"), (20, "0.0")])]),
        polyline("16", &[(71, "2"), (72, "2")], &[]),
        // 標高か頂点の Z が 0 でない
        polyline("0", &[(30, "1.0")], &[vertex(&[(10, "0.0"), (20, "0.0")])]),
        polyline(
            "8",
            &[],
            &[vertex(&[(10, "0.0"), (20, "0.0"), (30, "1.0")])],
        ),
    ]
    .concat();
    let (drawing, diag) = lift(&dxf(MM, &entities), &mut Budget::default());
    let params: Vec<_> = found(&diag)
        .into_iter()
        .map(|(c, p, n, _)| (c, p, n))
        .collect();
    assert_eq!(
        params,
        [
            (codes::DXF_LIFT_DROPPED, r#"Text("POLYLINE")"#.to_owned(), 2),
            (
                codes::DXF_NON_PLANAR_DROPPED,
                r#"Text("POLYLINE")"#.to_owned(),
                2
            ),
        ]
    );
    let corner = |x, y, bulge| Vertex {
        point: at(x, y),
        bulge,
    };
    assert_eq!(
        model_shapes(&drawing.unwrap()),
        [
            Shape::Polyline(Polyline {
                vertices: vec![
                    corner(0.0, 0.0, 1.0),
                    corner(10.0, 0.0, 0.0),
                    corner(10.0, 10.0, 0.0),
                ],
                closed: true,
                width: None,
                vertex_widths: vec![
                    [model(1.0), model(2.0)],
                    [model(0.0), model(0.0)],
                    [model(0.0), model(0.0)],
                ],
            }),
            Shape::Polyline(Polyline {
                vertices: vec![corner(0.0, 0.0, 0.0), corner(1.0, 1.0, 0.0)],
                closed: false,
                width: Some(model(0.5)),
                vertex_widths: Vec::new(),
            }),
            Shape::Polyline(Polyline {
                vertices: vec![corner(0.0, 0.0, 0.0), corner(2.0, 3.0, 0.0)],
                closed: false,
                width: None,
                vertex_widths: Vec::new(),
            }),
        ]
    );
}

#[test]
fn e10_tables_keep_rows_columns_merges_and_contents() {
    let sizes = [
        (91, "2"),
        (92, "2"),
        (93, "0"),
        (141, "10.0"),
        (141, "8.0"),
        (142, "30.0"),
        (142, "20.0"),
    ];
    let cells = [
        // 2 列に広げた見出し。セルの中の 91 は、行の数ではない
        (171, "1"),
        (172, "0"),
        (173, "0"),
        (175, "2"),
        (176, "1"),
        (91, "0"),
        (1, "PARTS"),
        (7, "Standard"),
        (140, "2.5"),
        // 見出しに覆われる
        (171, "1"),
        (173, "1"),
        (175, "1"),
        (176, "1"),
        (171, "1"),
        (2, "BO"),
        (1, "LT"),
        // 2007 からの値の文字
        (171, "1"),
        (301, "CELL_VALUE"),
        (93, "6"),
        (90, "4"),
        (302, "M8"),
        (304, "ACVALUE_END"),
    ];
    let (drawing, diag) = lift(&dxf(MM, &table(&sizes, &cells)), &mut Budget::default());
    assert_eq!(found_but_missing_blocks(&diag), []);
    let text = |content: &str| CellContent::Text {
        content: content.to_owned(),
        spans: vec![span(Piece::Text(content.to_owned()), Format::default())],
    };
    let cell = |content, span, covered| Cell {
        content,
        span,
        covered,
    };
    assert_eq!(
        model_shapes(&drawing.unwrap()),
        [Shape::Table(Table {
            position: at(0.0, 100.0),
            direction: [1.0, 0.0],
            row_heights: vec![model(10.0), model(8.0)],
            column_widths: vec![model(30.0), model(20.0)],
            cells: vec![
                cell(text("PARTS"), [2, 1], false),
                cell(CellContent::Empty, [1, 1], true),
                cell(text("BOLT"), [1, 1], false),
                cell(text("M8"), [1, 1], false),
            ],
            style: None,
            drawn: Some(block("*T1")),
        })]
    );
}

#[test]
fn e10_table_cells_can_hold_symbols() {
    let record = [(0, "BLOCK_RECORD"), (5, "B1"), (2, "MARK")];
    let entities = table(
        &[(91, "1"), (92, "1"), (141, "5.0"), (142, "5.0")],
        &[(171, "2"), (340, "B1")],
    );
    let src = sections(&[("HEADER", MM), ("TABLES", &record), ("ENTITIES", &entities)]);
    let (drawing, diag) = lift(&src, &mut Budget::default());
    assert_eq!(found_but_missing_blocks(&diag), []);
    match model_shapes(&drawing.unwrap()).as_slice() {
        [Shape::Table(table)] => assert_eq!(
            table.cells,
            [Cell {
                content: CellContent::Symbol(block("MARK")),
                span: [1, 1],
                covered: false,
            }]
        ),
        other => panic!("{other:?}"),
    }
}

#[test]
fn e10_title_blocks_keep_the_item_and_the_shown_value_apart() {
    // DXF は表題欄の役割を持たないので、属性を持つ記号のまま持ち、表題欄と推定しない
    let blocks = [
        entity(
            "BLOCK",
            &[(2, "TITLE"), (70, "2"), (10, "0.0"), (20, "0.0")],
        ),
        entity(
            "ATTDEF",
            &[
                (10, "0.0"),
                (20, "0.0"),
                (40, "3.5"),
                (1, ""),
                (2, "DWG_NO"),
                (3, "図面番号"),
                (70, "0"),
            ],
        ),
        entity("ENDBLK", &[]),
    ]
    .concat();
    let entities = [
        entity(
            "INSERT",
            &[(66, "1"), (2, "TITLE"), (10, "380.0"), (20, "10.0")],
        ),
        entity(
            "ATTRIB",
            &[
                (10, "390.0"),
                (20, "12.0"),
                (40, "3.5"),
                (1, "A-101"),
                (2, "DWG_NO"),
                (70, "0"),
            ],
        ),
        entity("SEQEND", &[]),
    ]
    .concat();
    let header = [MM, &[(9, "$ACADVER"), (1, "AC1021")]].concat();
    let src = sections(&[
        ("HEADER", &header),
        ("BLOCKS", &blocks),
        ("ENTITIES", &entities),
    ]);
    let (drawing, diag) = lift(&src, &mut Budget::default());
    assert_eq!(found(&diag), []);
    let drawing = drawing.unwrap();
    let shapes = model_shapes(&drawing);
    let [Shape::Symbol(title)] = shapes.as_slice() else {
        panic!("{shapes:?}");
    };
    let [value] = title.attributes.as_slice() else {
        panic!("{:?}", title.attributes);
    };
    assert_eq!(
        (value.tag.as_str(), value.text.content.as_str()),
        ("DWG_NO", "A-101")
    );
    let [item] = drawing.symbol(&title.key).unwrap().attributes.as_slice() else {
        panic!();
    };
    assert_eq!(
        (item.attribute.tag.as_str(), item.prompt.as_str()),
        ("DWG_NO", "図面番号")
    );
}

#[test]
fn e9_tables_that_cannot_be_read_are_reported() {
    let entities = [
        // AcDbTable の印がない
        entity("ACAD_TABLE", &[(10, "0.0"), (20, "0.0")]),
        // 行の高さの数が合わない
        table(
            &[(91, "2"), (92, "1"), (141, "5.0"), (142, "5.0")],
            &[(171, "1"), (171, "1")],
        ),
        // セルの数が合わない
        table(
            &[
                (91, "1"),
                (92, "2"),
                (141, "5.0"),
                (142, "5.0"),
                (142, "5.0"),
            ],
            &[(171, "1")],
        ),
        // 結合して広げる数が 0
        table(
            &[(91, "1"), (92, "1"), (141, "5.0"), (142, "5.0")],
            &[(171, "1"), (175, "0")],
        ),
    ]
    .concat();
    let (drawing, diag) = lift(&dxf(MM, &entities), &mut Budget::default());
    assert_eq!(drawing.unwrap().spaces[0].elements, []);
    let params: Vec<_> = found(&diag)
        .into_iter()
        .map(|(c, p, n, _)| (c, p, n))
        .collect();
    let invalid = |code: i64| {
        (
            codes::DXF_LIFT_INVALID,
            format!("Text(\"ACAD_TABLE\") Int({code})"),
            1,
        )
    };
    assert_eq!(
        params,
        [invalid(100), invalid(141), invalid(171), invalid(175)]
    );
}

#[test]
fn e7_underlays_keep_their_resource_page_frame_clip_and_order() {
    let objects = [
        (0, "IMAGEDEF"),
        (5, "D1"),
        (1, "site.png"),
        (10, "640.0"),
        (20, "480.0"),
        (0, "PDFDEFINITION"),
        (5, "D2"),
        (1, "plan.pdf"),
        (2, "2"),
    ];
    let entities = [
        entity(
            "LINE",
            &[(10, "0.0"), (20, "0.0"), (11, "10.0"), (21, "0.0")],
        ),
        // 1 画素が 0.5 の画像。三角形で切り抜き、内側を隠す
        entity(
            "IMAGE",
            &[
                (100, "AcDbRasterImage"),
                (90, "0"),
                (10, "100.0"),
                (20, "0.0"),
                (11, "0.5"),
                (21, "0.0"),
                (12, "0.0"),
                (22, "0.5"),
                (13, "640.0"),
                (23, "480.0"),
                (340, "D1"),
                (70, "7"),
                (280, "1"),
                (71, "2"),
                (91, "3"),
                (14, "0.0"),
                (24, "0.0"),
                (14, "640.0"),
                (24, "0.0"),
                (14, "0.0"),
                (24, "480.0"),
                (290, "1"),
            ],
        ),
        entity(
            "TEXT",
            &[(10, "0.0"), (20, "0.0"), (40, "2.5"), (1, "NOTE")],
        ),
        // 2 ページ目を 30 度回し、長方形の内側を切り抜く（280 の 16）
        entity(
            "PDFUNDERLAY",
            &[
                (340, "D2"),
                (10, "0.0"),
                (20, "300.0"),
                (41, "25.4"),
                (42, "25.4"),
                (50, "30.0"),
                (280, "19"),
                (11, "0.0"),
                (21, "0.0"),
                (11, "8.5"),
                (21, "11.0"),
            ],
        ),
    ]
    .concat();
    let src = sections(&[
        ("HEADER", MM),
        ("ENTITIES", &entities),
        ("OBJECTS", &objects),
    ]);
    let (drawing, diag) = lift(&src, &mut Budget::default());
    assert_eq!(found(&diag), []);
    let drawing = drawing.unwrap();
    assert_eq!(drawing.spaces[0].order, [0, 1, 2, 3]);
    let shapes = model_shapes(&drawing);
    let [
        Shape::Line { .. },
        Shape::Underlay(image),
        Shape::Text(_),
        Shape::Underlay(pdf),
    ] = shapes.as_slice()
    else {
        panic!("{shapes:?}");
    };
    assert_eq!(
        *image,
        Underlay {
            kind: UnderlayKind::Image,
            resource: Resource {
                path: "site.png".to_owned(),
                definition: Ref::at_record(9),
            },
            page: None,
            frame: UnderlayFrame::Pixels {
                size: [640.0, 480.0],
                origin: at(100.0, 0.0),
                right: at(420.0, 0.0),
                top: at(100.0, 240.0),
            },
            clip: Some(Clip {
                vertices: vec![[0.0, 0.0], [640.0, 0.0], [0.0, 480.0]],
                inverted: true,
            }),
            visible: true,
        }
    );
    assert_eq!(
        *pdf,
        Underlay {
            kind: UnderlayKind::Pdf,
            resource: Resource {
                path: "plan.pdf".to_owned(),
                definition: Ref::at_record(10),
            },
            page: Some("2".to_owned()),
            frame: UnderlayFrame::Scaled {
                origin: at(0.0, 300.0),
                scale: [model(25.4), model(25.4)],
                rotation: 30.0,
            },
            clip: Some(Clip {
                vertices: vec![[0.0, 0.0], [8.5, 11.0]],
                inverted: true,
            }),
            visible: true,
        }
    );
}

#[test]
fn e9_underlays_that_cannot_be_read_are_reported() {
    let objects = [
        (0, "IMAGEDEF"),
        (5, "D1"),
        (1, "site.png"),
        (0, "PDFDEFINITION"),
        (5, "D2"),
    ];
    let image = |tags: &[(i16, &'static str)]| {
        let mut all = vec![
            (10, "0.0"),
            (20, "0.0"),
            (11, "1.0"),
            (21, "0.0"),
            (12, "0.0"),
            (22, "1.0"),
        ];
        all.extend_from_slice(tags);
        entity("IMAGE", &all)
    };
    let entities = [
        // 定義を指さない
        image(&[(13, "1.0"), (23, "1.0")]),
        // 画素の数が 0
        image(&[(13, "0.0"), (23, "1.0"), (340, "D1")]),
        // 切り抜きの頂点の数が合わない
        image(&[
            (13, "1.0"),
            (23, "1.0"),
            (340, "D1"),
            (280, "1"),
            (71, "1"),
            (91, "3"),
            (14, "0.0"),
            (24, "0.0"),
            (14, "1.0"),
            (24, "1.0"),
        ]),
        // 定義が参照名を持たない
        entity("PDFUNDERLAY", &[(340, "D2"), (10, "0.0"), (20, "0.0")]),
        // 定義の種類が違う
        entity("PDFUNDERLAY", &[(340, "D1"), (10, "0.0"), (20, "0.0")]),
    ]
    .concat();
    let src = sections(&[
        ("HEADER", MM),
        ("ENTITIES", &entities),
        ("OBJECTS", &objects),
    ]);
    let (drawing, diag) = lift(&src, &mut Budget::default());
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
    assert_eq!(
        params,
        [
            invalid("IMAGE", 340),
            invalid("IMAGE", 13),
            invalid("IMAGE", 91),
            invalid("PDFDEFINITION", 1),
            invalid("PDFUNDERLAY", 340),
        ]
    );
}

/// XDATA の値
fn field(key: &'static str, value: FieldValue) -> Field {
    Field { key, value }
}

#[test]
fn e5_extensions_keep_their_namespace_version_and_values_and_expire_when_edited() {
    let header = [MM, &[(9, "$ACADVER"), (1, "AC1027")]].concat();
    let blocks = [
        entity("BLOCK", &[(2, "DOOR"), (70, "0"), (10, "0.0"), (20, "0.0")]),
        entity(
            "LINE",
            &[
                (10, "0.0"),
                (20, "0.0"),
                (11, "1.0"),
                (21, "0.0"),
                (1001, "PARTS"),
                (1071, "7"),
            ],
        ),
        entity("ENDBLK", &[]),
    ]
    .concat();
    let entities = [
        // 2 つの登録アプリケーション名の XDATA を持つ文字
        entity(
            "TEXT",
            &[
                (10, "10.0"),
                (20, "20.0"),
                (40, "2.5"),
                (1, "居間"),
                (1001, "ROOM"),
                (1000, "部屋"),
                (1002, "{"),
                (1070, "3"),
                (1003, "A-AREA"),
                (1002, "}"),
                (1011, "10.0"),
                (1021, "20.0"),
                (1031, "0.0"),
                (1040, "0.5"),
                (1041, "2.5"),
                (1042, "1.5"),
                (1005, "1F"),
                (1004, "0AFF"),
                (1001, "EMPTY"),
            ],
        ),
        entity(
            "LINE",
            &[(10, "0.0"), (20, "0.0"), (11, "10.0"), (21, "0.0")],
        ),
    ]
    .concat();
    let src = sections(&[
        ("HEADER", &header),
        ("BLOCKS", &blocks),
        ("ENTITIES", &entities),
    ]);
    let (drawing, diag) = lift(&src, &mut Budget::default());
    assert_eq!(found(&diag), []);
    let mut drawing = drawing.unwrap();
    let text = drawing.spaces[0].elements[0].clone();
    let door = drawing.symbols[0].space.clone();
    let extension = |name: &str, target, basis, fields, source| Extension {
        namespace: "dxf.xdata",
        name: name.to_owned(),
        version: Some("AC1027".to_owned()),
        schema: 1,
        target,
        basis,
        fields,
        source,
    };
    let room = Target::Model {
        space: MODEL,
        index: 0,
    };
    assert_eq!(
        drawing.extensions,
        [
            extension(
                "ROOM",
                room,
                Basis::Model(text.clone()),
                vec![
                    field("string", FieldValue::Text("部屋".to_owned())),
                    field("control", FieldValue::Text("{".to_owned())),
                    field("integer", FieldValue::Integer(3)),
                    field("layer", FieldValue::Text("A-AREA".to_owned())),
                    field("control", FieldValue::Text("}".to_owned())),
                    field("position", FieldValue::Point([10.0, 20.0, 0.0])),
                    field("real", FieldValue::Real(0.5)),
                    field("distance", FieldValue::Real(2.5)),
                    field("scale", FieldValue::Real(1.5)),
                    field("handle", FieldValue::Handle(0x1F)),
                    field("binary", FieldValue::Bytes(vec![0x0A, 0xFF])),
                ],
                text.source.clone(),
            ),
            extension(
                "EMPTY",
                room,
                Basis::Model(text.clone()),
                vec![],
                text.source.clone()
            ),
            extension(
                "PARTS",
                Target::Model {
                    space: door.id,
                    index: 0
                },
                Basis::Model(door.elements[0].clone()),
                vec![field("long", FieldValue::Integer(7))],
                door.elements[0].source.clone(),
            ),
        ]
    );
    assert_eq!(drawing.current_extensions().count(), 3);

    // 付く先でない要素の編集は、どれも失効させない
    drawing.spaces[0].elements[1].layer = LayerId(9);
    assert_eq!(drawing.stale_extensions().count(), 0);
    // 文字を動かすと、文字の XDATA は失効し、書き戻す一覧から外れる。動かす前の位置は戻らない
    let Shape::Text(moved) = &mut drawing.spaces[0].elements[0].shape else {
        panic!("文字ではない");
    };
    moved.placement = anchor(at(30.0, 20.0));
    let names = |extensions: Vec<&Extension>| -> Vec<String> {
        extensions.iter().map(|e| e.name.clone()).collect()
    };
    assert_eq!(
        names(drawing.stale_extensions().collect()),
        ["ROOM", "EMPTY"]
    );
    assert_eq!(names(drawing.current_extensions().collect()), ["PARTS"]);
    // 定義の中身を消すと、定義の中身の XDATA も失効する
    drawing.symbols[0].space.elements.clear();
    assert_eq!(drawing.current_extensions().count(), 0);
}

#[test]
fn e9_xdata_that_cannot_be_read_is_reported() {
    let entities = [entity(
        "LINE",
        &[
            (10, "0.0"),
            (20, "0.0"),
            (11, "1.0"),
            (21, "0.0"),
            // 登録アプリケーション名より前の XDATA
            (1000, "orphan"),
            // Z のない点
            (1001, "A"),
            (1010, "1.0"),
            (1020, "2.0"),
            // 括弧でない制御の値
            (1001, "B"),
            (1002, "["),
            // XDATA にない番号
            (1001, "C"),
            (1099, "x"),
            // 読める名前は移す
            (1001, "D"),
            (1070, "1"),
        ],
    )]
    .concat();
    let (drawing, diag) = lift(&dxf(MM, &entities), &mut Budget::default());
    let drawing = drawing.unwrap();
    assert_eq!(drawing.spaces[0].elements.len(), 1);
    let names: Vec<&str> = drawing.extensions.iter().map(|e| e.name.as_str()).collect();
    assert_eq!(names, ["D"]);
    let source = drawing.spaces[0].elements[0].source.clone();
    let invalid = |code: i64| {
        (
            codes::DXF_LIFT_INVALID,
            format!("Text(\"LINE\") Int({code})"),
            1,
            vec![source.clone()],
        )
    };
    assert_eq!(
        found(&diag),
        [invalid(1000), invalid(1030), invalid(1002), invalid(1099)]
    );
}

#[test]
fn e6_definitions_and_codes_with_the_same_look_are_kept_apart() {
    let dashes = |handle: &'static str, name: &'static str| {
        vec![
            (0, "LTYPE"),
            (5, handle),
            (2, name),
            (3, "__ __"),
            (73, "2"),
            (49, "0.5"),
            (74, "0"),
            (49, "-0.25"),
            (74, "0"),
        ]
    };
    let tables = [
        vec![(0, "TABLE"), (2, "LTYPE")],
        dashes("14", "DASH1"),
        dashes("15", "DASH2"),
        vec![(0, "ENDTAB")],
    ]
    .concat();
    let entities = [
        model_line("20", "0"),
        vec![(6, "DASH1"), (62, "1"), (420, "16711680")],
        model_line("21", "0"),
        vec![(6, "DASH2"), (420, "16711680")],
    ]
    .concat();
    let src = sections(&[("HEADER", MM), ("TABLES", &tables), ("ENTITIES", &entities)]);
    let (drawing, diag) = lift(&src, &mut Budget::default());
    assert_eq!(found(&diag), []);
    let drawing = drawing.unwrap();
    // 同じ形の線種も、名前ごとに別の定義のまま持つ
    let [first, second] = drawing.line_types.as_slice() else {
        panic!("線種が 2 つでない");
    };
    assert_eq!(
        (first.name.as_str(), second.name.as_str()),
        ("DASH1", "DASH2")
    );
    assert_eq!(first.definition, second.definition);
    assert_ne!(first.id, second.id);
    let styles: Vec<_> = drawing.spaces[0]
        .elements
        .iter()
        .map(|e| (e.style.color, e.style.line_type))
        .collect();
    // 同じ RGB でも、元の符号を持つ色と RGB だけの色を分ける
    assert_eq!(
        styles,
        [
            (
                Inherit::Own(Color {
                    code: Some(ColorCode {
                        system: ColorSystem::Aci,
                        value: 1
                    }),
                    rgb: Some([255, 0, 0]),
                }),
                Inherit::Own(first.id),
            ),
            (
                Inherit::Own(Color {
                    code: None,
                    rgb: Some([255, 0, 0]),
                }),
                Inherit::Own(second.id),
            ),
        ]
    );
}

#[test]
fn e8_a_fill_keeps_its_hole() {
    let square = |flags: &'static str, [a, b]: [&'static str; 2]| {
        vec![
            (92, flags),
            (72, "0"),
            (73, "1"),
            (93, "4"),
            (10, a),
            (20, a),
            (10, b),
            (20, a),
            (10, b),
            (20, b),
            (10, a),
            (20, b),
            (97, "0"),
        ]
    };
    let hatch = [
        entity(
            "HATCH",
            &[
                (100, "AcDbHatch"),
                (10, "0.0"),
                (20, "0.0"),
                (30, "0.0"),
                (2, "SOLID"),
                (70, "1"),
                (71, "0"),
                (91, "2"),
            ],
        ),
        // 外側でいちばん外の輪と、穴の輪
        square("19", ["0.0", "10.0"]),
        square("2", ["4.0", "6.0"]),
        vec![(75, "0"), (76, "1"), (98, "0")],
    ]
    .concat();
    let (drawing, diag) = lift(&dxf(MM, &hatch), &mut Budget::default());
    assert_eq!(found(&diag), []);
    let drawing = drawing.unwrap();
    let Shape::Hatch(hatch) = &drawing.spaces[0].elements[0].shape else {
        panic!("ハッチングでない");
    };
    assert_eq!(hatch.fill, Fill::Solid);
    assert_eq!(hatch.islands, Islands::Nested);
    let loops: Vec<_> = hatch
        .loops
        .iter()
        .map(|l| (l.external, l.outermost, l.boundary.clone()))
        .collect();
    let square = |a: f64, b: f64| Boundary::Polyline {
        vertices: [(a, a), (b, a), (b, b), (a, b)]
            .map(|(x, y)| Vertex {
                point: at(x, y),
                bulge: 0.0,
            })
            .to_vec(),
        closed: true,
    };
    // 穴の輪を外側の輪と混ぜず、塗りつぶさない
    assert_eq!(
        loops,
        [
            (true, true, square(0.0, 10.0)),
            (false, false, square(4.0, 6.0)),
        ]
    );
}

#[test]
fn e9_cyclic_and_missing_definitions_are_reported_and_placements_are_not_expanded() {
    let block = |name: &'static str, inner: &'static str| {
        [
            entity("BLOCK", &[(2, name), (70, "0"), (10, "0.0"), (20, "0.0")]),
            entity("INSERT", &[(2, inner), (10, "0.0"), (20, "0.0")]),
            entity("ENDBLK", &[]),
        ]
        .concat()
    };
    let blocks = [
        // A と B は互いを、C は自分を配置する
        block("A", "B"),
        block("B", "A"),
        block("C", "C"),
        block("D", "A"),
    ]
    .concat();
    let entities = [
        // 定義のない配置
        entity("INSERT", &[(2, "MISSING"), (10, "0.0"), (20, "0.0")]),
        // 6 万 × 6 万の格子。展開しない
        entity(
            "INSERT",
            &[
                (2, "D"),
                (10, "0.0"),
                (20, "0.0"),
                (70, "60000"),
                (71, "60000"),
                (44, "1.0"),
                (45, "1.0"),
            ],
        ),
        // 有限でない尺度
        entity("INSERT", &[(2, "D"), (10, "0.0"), (20, "0.0"), (41, "inf")]),
    ]
    .concat();
    let src = sections(&[("HEADER", MM), ("BLOCKS", &blocks), ("ENTITIES", &entities)]);
    let mut budget = Budget::new(Limits::default().with(Kind::Work, Limit::Finite(100)));
    let (drawing, diag) = lift(&src, &mut budget);
    let drawing = drawing.unwrap();
    let elements = &drawing.spaces[0].elements;
    assert_eq!(elements.len(), 2);
    let Shape::Symbol(grid) = &elements[1].shape else {
        panic!("記号でない");
    };
    assert_eq!(
        grid.grid.as_ref().map(|g| (g.columns, g.rows)),
        Some((60000, 60000))
    );
    // 定義は輪があっても移す。D は輪の中にないので診断しない
    let names: Vec<&str> = drawing
        .symbols
        .iter()
        .map(|s| s.key.name.as_str())
        .collect();
    assert_eq!(names, ["A", "B", "C", "D"]);
    let at = |name: &str| {
        drawing
            .symbols
            .iter()
            .find(|s| s.key.name == name)
            .unwrap()
            .source
            .clone()
    };
    let named = |code, name: &str, place: Ref| (code, format!("Text({name:?})"), 1, vec![place]);
    let mut found = found(&diag);
    let invalid = found.remove(0);
    assert_eq!(
        (invalid.0, invalid.1),
        (
            codes::DXF_LIFT_INVALID,
            "Text(\"INSERT\") Int(41)".to_owned()
        )
    );
    assert_eq!(
        found,
        [
            named(
                codes::DXF_BLOCK_MISSING,
                "MISSING",
                elements[0].source.clone()
            ),
            named(codes::DXF_BLOCK_CYCLE_INVALID, "A", at("A")),
            named(codes::DXF_BLOCK_CYCLE_INVALID, "B", at("B")),
            named(codes::DXF_BLOCK_CYCLE_INVALID, "C", at("C")),
        ]
    );
}
