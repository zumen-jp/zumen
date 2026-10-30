//! 図面モデルの変更を、JWW の図形の一覧に書き戻す（決定 0168）。入力は、どれも試験の中で作る。

mod common;

use core::f64::consts::PI;

use common::{Archive, Spec};
use zumen_base::{Budget, CancelToken, Code, Cx, Diagnostics, codes};
use zumen_drawing::{
    Affine, Applied, Changes, Color, ColorCode, ColorSystem, Drawing2D, Edge, Edit, Element, Hash,
    Inherit, LayerId, Length, LineTypeId, LineWeight, ModelLength, ModelPoint, PaperLength,
    PaperPoint, Placed, Restyle, Seen, Shape, SpaceId, Style, id,
};
use zumen_jww::{Jww, JwwFile};
use zumen_sdk::{Apply, Codec, Error, Lift, testkit};

/// グループ 0 は縮尺 1/100。線・円・円弧・点・文字と、線を 1 本持つブロックの定義。レコードは、見出し・図形 5 つ・
/// 定義・定義の中の線・画像の件数
#[cfg(test)]
fn src() -> Vec<u8> {
    let mut spec = Spec::new(700);
    let mut layers = [0; 16];
    layers[0] = 3;
    layers[1] = 2;
    spec.groups[0] = (3, 100.0, layers);
    spec.max_draw_width = -300;
    spec.pens[2] = 0x0000_00FF;
    spec.printer_widths[2] = 25;
    let mut archive = Archive::new(700);
    archive.layer = 0;
    archive.group = 0;
    archive.pen_width = 0;
    archive.count(5);
    archive.line();
    archive.tag("CDataEnko");
    archive.base(1, 1);
    archive.reals(&[1.0, 2.0, 5.0, 0.0, 2.0 * PI, 0.0, 1.0]);
    archive.out.extend(1u32.to_le_bytes());
    archive.tag("CDataEnko");
    archive.base(1, 1);
    archive.reals(&[0.0, 0.0, 5.0, 0.0, PI / 2.0, 0.0, 1.0]);
    archive.out.extend(0u32.to_le_bytes());
    archive.tag("CDataTen");
    archive.base(1, 2);
    archive.reals(&[1.0, 1.0]);
    archive.out.extend(0u32.to_le_bytes());
    archive.tag("CDataMoji");
    archive.base(1, 1);
    archive.text_body("図面");
    archive.count(1);
    archive.block("部品");
    archive.out.extend(0u32.to_le_bytes());
    let mut src = spec.build();
    src.extend(&archive.out);
    src
}

#[cfg(test)]
fn read(src: &[u8]) -> JwwFile {
    testkit::read::<Jww>(src).0.unwrap()
}

#[cfg(test)]
fn lift(file: &JwwFile) -> Drawing2D {
    let mut budget = Budget::default();
    let mut diag = Diagnostics::new();
    let cancel = CancelToken::new();
    file.lift(&mut Cx::new(&mut budget, &mut diag, &cancel))
        .unwrap()
}

#[cfg(test)]
fn write(file: &JwwFile) -> Vec<u8> {
    let mut budget = Budget::default();
    let mut diag = Diagnostics::new();
    let cancel = CancelToken::new();
    let mut out = Vec::new();
    Jww::write(
        file,
        &mut out,
        &mut Cx::new(&mut budget, &mut diag, &cancel),
    )
    .unwrap();
    out
}

#[cfg(test)]
#[expect(clippy::type_complexity)]
#[cfg(test)]
fn apply(
    file: &mut JwwFile,
    edits: Vec<Edit>,
    document: Option<Hash>,
) -> (Result<(Vec<Applied>, bool), Error>, Vec<Code>) {
    let mut budget = Budget::default();
    let mut diag = Diagnostics::new();
    let cancel = CancelToken::new();
    let changes = Changes { edits, document };
    let result = file.apply(&changes, &mut Cx::new(&mut budget, &mut diag, &cancel));
    (result, diag.iter().map(|d| d.code()).collect())
}

/// 空間 0 の要素と、その ID
#[cfg(test)]
fn element(drawing: &Drawing2D, at: usize) -> (String, &Element<ModelPoint>) {
    let space = drawing.spaces.iter().find(|s| s.id == SpaceId(0)).unwrap();
    let element = &space.elements[at];
    let mut namer = id::Namer::new("space/0");
    for (index, other) in space.elements.iter().enumerate().take(at) {
        namer.next(index, other);
    }
    (namer.next(at, element), element)
}

#[cfg(test)]
fn seen(drawing: &Drawing2D, at: usize) -> Seen {
    let (id, element) = element(drawing, at);
    Seen {
        id,
        hash: Hash::element(element),
    }
}

#[cfg(test)]
fn model(x: f64, y: f64) -> ModelPoint {
    ModelPoint {
        space: SpaceId(0),
        x,
        y,
    }
}

#[cfg(test)]
fn length(value: f64) -> Length {
    Length::Model(ModelLength {
        space: SpaceId(0),
        value,
    })
}

#[cfg(test)]
fn close(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-9 * a.abs().max(1.0)
}

#[test]
fn rewriting_lists_preserves_each_class_schema() {
    let mut spec = Spec::new(700);
    spec.groups[0] = (3, 100.0, [3; 16]);
    let mut archive = Archive::new(700);
    archive.group = 0;
    archive.layer = 0;
    archive.count(2);
    archive.line();
    // 同じクラス名を別のスキーマで新たに定義する。
    archive.out.extend([0xFF, 0xFF, 2, 0, 8, 0]);
    archive.out.extend(b"CDataSen");
    archive.base(1, 1);
    archive.reals(&[0.0, 1.0, 10.0, 1.0]);
    archive.count(0);
    archive.out.extend(0u32.to_le_bytes());
    let mut input = spec.build();
    input.extend(&archive.out);
    let mut file = read(&input);
    let drawing = lift(&file);
    let schemas = |file: &JwwFile| {
        file.records()
            .iter()
            .filter(|record| file.kind_name(record) == b"CDataSen")
            .map(|record| file.classes()[record.tag().unwrap().1].schema())
            .collect::<Vec<_>>()
    };
    assert_eq!(schemas(&file), [1, 2]);
    let added = element(&drawing, 0).1.clone();
    let (result, diagnostics) = apply(
        &mut file,
        vec![Edit::Add {
            place: "space/0".to_owned(),
            element: Box::new(Placed::Model(added)),
        }],
        None,
    );
    assert!(result.is_ok(), "{result:?} {diagnostics:?}");
    assert_eq!(schemas(&file), [1, 2, 1]);
}

#[test]
fn restyling_layer_preserves_unread_high_bits() {
    let mut input = src();
    let file = read(&input);
    let at = file.records()[1].body().start;
    input[at + 9..at + 11].copy_from_slice(&0xCD00u16.to_le_bytes());
    input[at + 11..at + 13].copy_from_slice(&0xAB00u16.to_le_bytes());
    let mut file = read(&input);
    let drawing = lift(&file);
    let (result, diagnostics) = apply(
        &mut file,
        vec![Edit::Restyle {
            target: seen(&drawing, 0),
            style: Restyle {
                layer: Some(LayerId(2)),
                ..Restyle::default()
            },
        }],
        None,
    );
    assert!(result.is_ok(), "{result:?} {diagnostics:?}");
    let output = write(&file);
    assert_eq!(
        u16::from_le_bytes(output[at + 9..at + 11].try_into().unwrap()),
        0xCD01
    );
    assert_eq!(
        u16::from_le_bytes(output[at + 11..at + 13].try_into().unwrap()),
        0xAB00
    );
}

#[test]
fn added_entity_uses_the_schema_of_its_own_class() {
    let mut spec = Spec::new(700);
    spec.groups[0] = (3, 100.0, [3; 16]);
    let mut archive = Archive::new(700);
    archive.group = 0;
    archive.layer = 0;
    archive.count(2);
    archive.tag("CDataEnko");
    archive.base(1, 1);
    archive.reals(&[0.0, 0.0, 5.0, 0.0, 2.0 * PI, 0.0, 1.0]);
    archive.out.extend(1u32.to_le_bytes());
    archive.out.extend([0xFF, 0xFF, 2, 0, 8, 0]);
    archive.out.extend(b"CDataSen");
    archive.base(1, 1);
    archive.reals(&[0.0, 0.0, 10.0, 0.0]);
    archive.count(0);
    archive.out.extend(0u32.to_le_bytes());
    let mut input = spec.build();
    input.extend(&archive.out);
    let mut file = read(&input);
    let drawing = lift(&file);
    let added = element(&drawing, 1).1.clone();
    let (result, diagnostics) = apply(
        &mut file,
        vec![Edit::Add {
            place: "space/0".to_owned(),
            element: Box::new(Placed::Model(added)),
        }],
        None,
    );
    assert!(result.is_ok(), "{result:?} {diagnostics:?}");
    let schemas: Vec<_> = file
        .records()
        .iter()
        .filter(|record| file.kind_name(record) == b"CDataSen")
        .map(|record| file.classes()[record.tag().unwrap().1].schema())
        .collect();
    assert_eq!(schemas, [2, 2]);
}

#[test]
fn edits_are_written_into_the_entity_lists() {
    let src = src();
    let mut file = read(&src);
    let drawing = lift(&file);
    let definition = &drawing.symbols[0];
    let inner = Seen {
        id: format!("{}/r7", id::DefinitionNamer::default().next(definition)),
        hash: Hash::element(&definition.space.elements[0]),
    };
    let added = Element {
        source: Default::default(),
        layer: LayerId(2),
        style: Style {
            color: Inherit::Own(Color {
                code: None,
                rgb: Some([0xFF, 0, 0]),
            }),
            line_type: Inherit::Own(LineTypeId(1)),
            line_type_scale: 1.0,
            line_weight: None,
            transparency: Inherit::Layer,
        },
        shape: Shape::Line {
            start: model(0.0, 0.0),
            end: model(0.0, 300.0),
        },
    };
    let edits = vec![
        Edit::Restyle {
            target: seen(&drawing, 0),
            style: Restyle {
                color: Some(Inherit::Own(Color {
                    code: Some(ColorCode {
                        system: ColorSystem::Jww,
                        value: 2,
                    }),
                    rgb: None,
                })),
                line_type: Some(Inherit::Own(LineTypeId(2))),
                line_weight: Some(Inherit::Own(LineWeight::Width(PaperLength(0.5)))),
                ..Restyle::default()
            },
        },
        Edit::Transform {
            target: seen(&drawing, 1),
            transform: Affine {
                m: [1.0, 0.0, 10.0, 0.0, 1.0, -20.0],
            },
        },
        Edit::Replace {
            target: seen(&drawing, 2),
            element: Box::new(Placed::Model(Element {
                shape: Shape::Curve(Edge::Arc {
                    center: model(50.0, 50.0),
                    radius: length(30.0),
                    start: 180.0,
                    end: 90.0,
                    counterclockwise: false,
                }),
                ..element(&drawing, 2).1.clone()
            })),
        },
        Edit::Remove {
            target: seen(&drawing, 3),
        },
        Edit::Add {
            place: "space/0".into(),
            element: Box::new(Placed::Model(added)),
        },
        Edit::Transform {
            target: inner,
            transform: Affine {
                m: [1.0, 0.0, 1.0, 0.0, 1.0, 0.0],
            },
        },
    ];
    let (result, diagnostics) = apply(&mut file, edits, Some(Hash::document(&src)));
    let (applied, shifted) = result.unwrap();
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
    // 点を除いたので、後ろの文字の ID がずれる
    assert!(shifted);
    let ids: Vec<_> = applied.iter().map(|a| a.id.as_deref()).collect();
    assert_eq!(
        ids[..5],
        [
            Some("space/0/r1"),
            Some("space/0/r2"),
            Some("space/0/r3"),
            None,
            Some("space/0/r5"),
        ]
    );

    // 読み直すと、求めた値になる
    let out = write(&file);
    let reread = read(&out);
    assert_eq!(reread.version(), 700);
    let drawing = lift(&reread);
    let (_, line) = element(&drawing, 0);
    assert_eq!(
        line.style.color,
        Inherit::Own(Color {
            code: Some(ColorCode {
                system: ColorSystem::Jww,
                value: 2,
            }),
            rgb: Some([0xFF, 0, 0]),
        })
    );
    assert_eq!(line.style.line_type, Inherit::Own(LineTypeId(2)));
    assert_eq!(
        line.style.line_weight,
        Some(Inherit::Own(LineWeight::Width(PaperLength(0.5))))
    );
    let (_, circle) = element(&drawing, 1);
    let Shape::Circle { center, radius } = circle.shape else {
        panic!("{:?}", circle.shape);
    };
    assert!(close(center.x, 110.0) && close(center.y, 180.0));
    assert_eq!(radius, length(500.0));
    let (_, arc) = element(&drawing, 2);
    let Shape::Curve(Edge::Arc {
        center,
        start,
        end,
        counterclockwise,
        ..
    }) = arc.shape
    else {
        panic!("{:?}", arc.shape);
    };
    assert!(close(center.x, 50.0) && close(center.y, 50.0));
    assert!(close(start, 180.0) && close(end, 90.0) && !counterclockwise);
    let (id, text) = element(&drawing, 3);
    assert_eq!(id, "space/0/r4");
    assert!(matches!(text.shape, Shape::Text(_)));
    let (_, line) = element(&drawing, 4);
    assert_eq!(line.layer, LayerId(2));
    assert_eq!(
        line.shape,
        Shape::Line {
            start: model(0.0, 0.0),
            end: model(0.0, 300.0),
        }
    );
    // ペンの印刷の幅に従う
    assert_eq!(
        line.style.line_weight,
        Some(Inherit::Own(LineWeight::Width(PaperLength(0.25))))
    );
    let Shape::Line { start, .. } = drawing.symbols[0].space.elements[0].shape else {
        panic!();
    };
    assert_eq!(start.x, 1.0);
    // 点のクラスはなくなり、ほかのクラスの番号を数え直した
    let names: Vec<_> = reread
        .records()
        .iter()
        .map(|record| String::from_utf8_lossy(reread.kind_name(record)).into_owned())
        .collect();
    assert_eq!(
        names,
        [
            "header",
            "CDataSen",
            "CDataEnko",
            "CDataEnko",
            "CDataMoji",
            "CDataSen",
            "CDataList",
            "CDataSen",
            "image-count",
        ]
    );
    // 見出しと画像の件数は、元のバイトのまま
    let header = reread.records()[0].span();
    assert_eq!(out[header.clone()], src[header]);
    assert!(out.ends_with(&0u32.to_le_bytes()));
}

#[test]
fn edits_that_cannot_be_written_change_nothing() {
    let src = src();
    let mut file = read(&src);
    let drawing = lift(&file);
    let (_, line) = element(&drawing, 0);
    let paper = Element {
        source: Default::default(),
        layer: LayerId(1),
        style: line.style,
        shape: Shape::Line {
            start: PaperPoint {
                sheet: zumen_drawing::SheetId(0),
                x: 0.0,
                y: 0.0,
            },
            end: PaperPoint {
                sheet: zumen_drawing::SheetId(0),
                x: 1.0,
                y: 0.0,
            },
        },
    };
    let edits = vec![
        // 当てられる変更
        Edit::Remove {
            target: seen(&drawing, 0),
        },
        // 文字のスタイル、別のレイヤグループへの移動、楕円にする変換、用紙への追加、レイヤに従う色
        Edit::Restyle {
            target: seen(&drawing, 4),
            style: Restyle {
                line_type: Some(Inherit::Own(LineTypeId(2))),
                ..Restyle::default()
            },
        },
        Edit::Restyle {
            target: seen(&drawing, 1),
            style: Restyle {
                layer: Some(LayerId(17)),
                ..Restyle::default()
            },
        },
        Edit::Transform {
            target: seen(&drawing, 2),
            transform: Affine {
                m: [2.0, 0.0, 0.0, 0.0, 1.0, 0.0],
            },
        },
        Edit::Add {
            place: "sheet/0".into(),
            element: Box::new(Placed::Paper(paper)),
        },
        Edit::Restyle {
            target: seen(&drawing, 2),
            style: Restyle {
                color: Some(Inherit::Layer),
                ..Restyle::default()
            },
        },
    ];
    let mut budget = Budget::default();
    let mut diag = Diagnostics::new();
    let cancel = CancelToken::new();
    let changes = Changes {
        edits,
        document: None,
    };
    let result = file.apply(&changes, &mut Cx::new(&mut budget, &mut diag, &cancel));
    assert!(matches!(result, Err(Error::Rejected)));
    // 同じ符号の診断は 1 つにまとまり、数を持つ
    let diagnostics: Vec<_> = diag.iter().collect();
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].code(), codes::CHANGE_EDIT_UNSUPPORTED);
    assert_eq!(diagnostics[0].count(), 5);
    assert_eq!(write(&file), src);
}

#[test]
fn a_stale_document_or_element_is_refused() {
    let src = src();
    let mut file = read(&src);
    let drawing = lift(&file);
    let remove = || Edit::Remove {
        target: seen(&drawing, 0),
    };
    let (result, diagnostics) = apply(&mut file, vec![remove()], Some(Hash::document(b"x")));
    assert!(matches!(result, Err(Error::Rejected)));
    assert_eq!(diagnostics, [codes::CHANGE_DOCUMENT_STALE]);
    let mut stale = seen(&drawing, 1);
    stale.hash = Hash::document(b"x");
    let (result, diagnostics) = apply(&mut file, vec![Edit::Remove { target: stale }], None);
    assert!(matches!(result, Err(Error::Rejected)));
    assert_eq!(diagnostics, [codes::CHANGE_ELEMENT_STALE]);
    assert_eq!(write(&file), src);
}

#[test]
fn writing_counts_copied_bytes_and_stops_without_changes() {
    use zumen_base::{Kind, Limit, Limits};
    let mut large = src();
    large.extend(vec![0u8; 1024 * 1024]);
    let run = |input: &[u8], remove: bool, limit: Option<u64>| {
        let mut file = read(input);
        let drawing = lift(&file);
        let mut budget = Budget::new(limit.map_or(Limits::default(), |limit| {
            Limits::default().with(Kind::Work, Limit::Finite(limit))
        }));
        let mut diagnostics = Diagnostics::new();
        let changes = Changes {
            edits: if remove {
                vec![Edit::Remove {
                    target: seen(&drawing, 0),
                }]
            } else {
                Vec::new()
            },
            document: None,
        };
        let result = file.apply(
            &changes,
            &mut Cx::new(&mut budget, &mut diagnostics, &CancelToken::new()),
        );
        (result, budget.used(Kind::Work), write(&file))
    };
    for remove in [false, true] {
        let (_, small_work, _) = run(&src(), remove, None);
        let (result, large_work, _) = run(&large, remove, None);
        assert!(result.is_ok());
        assert!(
            large_work >= small_work + 1024,
            "{small_work}, {large_work}"
        );
        let (result, _, written) = run(&large, remove, Some(small_work + 100));
        assert!(matches!(result, Err(Error::Stopped(_))));
        assert_eq!(written, large);
    }
}

/// 点の位置と仮点かを置き換え、変換で動かし、点と記号の点を足す。読み直すと同じ点になる。記号でない点は角度を
/// 持たないので、回す変換では角度を 0 のままにし、角度を置き換えることと、記号の点と入れ替えることは断る（決定 0183）
#[test]
fn points_are_replaced_transformed_and_added() {
    use zumen_drawing::{MarkerKey, Point, PointLook};
    let src = src();
    let drawing = lift(&read(&src));
    let (_, point) = element(&drawing, 3);
    let Shape::Point(dot) = &point.shape else {
        panic!("{:?}", point.shape);
    };
    let with = |dot: Point<ModelPoint>| Element {
        shape: Shape::Point(dot),
        ..point.clone()
    };
    let points = |file: &JwwFile| -> Vec<Point<ModelPoint>> {
        lift(file).spaces[0]
            .elements
            .iter()
            .filter_map(|element| match &element.shape {
                Shape::Point(dot) => Some(dot.clone()),
                _ => None,
            })
            .collect()
    };
    let close = |a: &Point<ModelPoint>, b: &Point<ModelPoint>| {
        (a.position.x - b.position.x).abs() < 1e-9
            && (a.position.y - b.position.y).abs() < 1e-9
            && (a.rotation - b.rotation).abs() < 1e-9
            && a.look == b.look
            && a.construction == b.construction
    };

    let moved = Point {
        position: model(50.0, 60.0),
        construction: true,
        ..dot.clone()
    };
    let mut file = read(&src);
    let (result, diagnostics) = apply(
        &mut file,
        vec![Edit::Replace {
            target: seen(&drawing, 3),
            element: Box::new(Placed::Model(with(moved.clone()))),
        }],
        None,
    );
    assert!(result.is_ok(), "{diagnostics:?}");
    assert!(close(&points(&file)[0], &moved));

    // 回しても、記号でない点の角度は 0 のまま
    let mut file = read(&src);
    let (result, diagnostics) = apply(
        &mut file,
        vec![Edit::Transform {
            target: seen(&drawing, 3),
            transform: Affine::rotation(0.0, 0.0, 90.0),
        }],
        None,
    );
    assert!(result.is_ok(), "{diagnostics:?}");
    assert!(close(
        &points(&file)[0],
        &Point {
            position: model(-100.0, 100.0),
            ..dot.clone()
        }
    ));

    let marker = Point {
        position: model(200.0, 300.0),
        rotation: 30.0,
        look: PointLook::Marker {
            key: MarkerKey {
                namespace: "jww.point",
                code: "3".to_owned(),
            },
            scale: 2.0,
        },
        construction: false,
    };
    let added = Point {
        position: model(10.0, 20.0),
        ..dot.clone()
    };
    let mut file = read(&src);
    let (result, diagnostics) = apply(
        &mut file,
        vec![
            Edit::Add {
                place: "space/0".into(),
                element: Box::new(Placed::Model(with(added.clone()))),
            },
            Edit::Add {
                place: "space/0".into(),
                element: Box::new(Placed::Model(with(marker.clone()))),
            },
        ],
        None,
    );
    assert!(result.is_ok(), "{diagnostics:?}");
    let found = points(&file);
    assert_eq!(found.len(), 3);
    assert!(
        close(&found[1], &added) && close(&found[2], &marker),
        "{found:?}"
    );

    let turned = Point {
        rotation: 10.0,
        ..dot.clone()
    };
    let foreign = Point {
        look: PointLook::Marker {
            key: MarkerKey {
                namespace: "sxf.marker",
                code: "3".to_owned(),
            },
            scale: 1.0,
        },
        ..marker.clone()
    };
    for edit in [
        Edit::Replace {
            target: seen(&drawing, 3),
            element: Box::new(Placed::Model(with(turned))),
        },
        Edit::Replace {
            target: seen(&drawing, 3),
            element: Box::new(Placed::Model(with(marker))),
        },
        Edit::Add {
            place: "space/0".into(),
            element: Box::new(Placed::Model(with(foreign))),
        },
    ] {
        let mut file = read(&src);
        let (result, diagnostics) = apply(&mut file, vec![edit], None);
        assert_eq!(result, Err(Error::Rejected));
        assert_eq!(diagnostics, [codes::CHANGE_EDIT_UNSUPPORTED]);
        assert_eq!(write(&file), src);
    }
}

/// JWW は文書の点の表し方（DXF・DWG の $PDMODE・$PDSIZE）に当たるものを持たないので、その変更を断り、ファイルを
/// 変えない（決定 0190）
#[test]
fn point_styles_are_refused() {
    use zumen_drawing::{PointMark, PointSize, PointStyle};
    let src = src();
    let mut file = read(&src);
    let (result, diagnostics) = apply(
        &mut file,
        vec![Edit::SetPointStyle {
            style: PointStyle {
                mark: PointMark::Plus,
                circle: false,
                square: false,
                size: PointSize::Length(1.0),
            },
        }],
        None,
    );
    assert_eq!(result, Err(Error::Rejected));
    assert_eq!(diagnostics, [codes::CHANGE_EDIT_UNSUPPORTED]);
    assert_eq!(write(&file), src);
}
