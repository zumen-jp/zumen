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
