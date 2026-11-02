//! JWW から図面モデルへの変換（決定 0134）。入力は、どれも試験の中で作る。

mod common;

use core::f64::consts::PI;

use common::{Archive, Spec};
use zumen_base::{Budget, CancelToken, Cx, Diagnostic, Diagnostics};
use zumen_drawing::{
    Boundary, Color, ColorCode, ColorSystem, DimensionKind, Drawing2D, Edge, Element, Extension,
    FieldValue, Fill, Inherit, Label, LayerId, LayerState, Length, LineWeight, MarkerKey,
    ModelLength, ModelPoint, PaperLength, Placement, Point, PointLook, Position, Shape, SpaceId,
    Target, UnderlayFrame, UnderlayKind, Unit,
};
use zumen_jww::Jww;
use zumen_sdk::{Lift, testkit};

#[cfg(test)]
fn lift(src: &[u8]) -> (Drawing2D, Vec<Diagnostic>) {
    let file = testkit::read::<Jww>(src).0.unwrap();
    let mut budget = Budget::default();
    let mut diag = Diagnostics::new();
    let cancel = CancelToken::new();
    let drawing = file
        .lift(&mut Cx::new(&mut budget, &mut diag, &cancel))
        .unwrap();
    (drawing, diag.into_vec())
}

fn model(space: u32, x: f64, y: f64) -> ModelPoint {
    ModelPoint {
        space: SpaceId(space),
        x,
        y,
    }
}

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-9
}

/// A3 の用紙。グループ 0 は縮尺 1/100 で書き込み、グループ 1 は 1/1 で表示だけ
fn spec() -> Spec {
    let mut spec = Spec::new(700);
    spec.paper = 3;
    let mut layers = [0; 16];
    layers[0] = 2;
    layers[2] = 3;
    spec.groups[0] = (3, 100.0, layers);
    let mut layers = [0; 16];
    layers[0] = 1;
    spec.groups[1] = (1, 0.0, layers);
    spec.layer_names = vec![(0, "通り芯")];
    spec.write_group = 0;
    spec.group_names = vec![(0, "平面")];
    spec.protects = vec![(2, 1)];
    spec.max_draw_width = -300;
    spec.pens[2] = 0x0000_00FF;
    spec.printer_widths[2] = 35;
    // 線種 2 は、4 ビットの線と 4 ビットの間。1 ビットは 1 mm
    spec.patterns[0] = [0x0F0F, 16, 1, 32];
    spec.sxf_pens = vec![(2, 0x0000_FF00, 50)];
    spec.sxf_line_types = vec![(3, "dashed", vec![6.0, 6.0])];
    spec
}

fn entities(archive: &mut Archive) -> u16 {
    archive.layer = 0;
    archive.group = 0;
    archive.pen_width = 0;
    archive.tag("CDataSen");
    archive.base(2, 2);
    archive.reals(&[0.0, 0.0, 10.0, 0.0]);
    archive.pen_width = 50;
    archive.tag("CDataSen");
    archive.base(33, 102);
    archive.reals(&[0.0, 1.0, 10.0, 1.0]);
    archive.pen_width = 0;
    // 円・時計回りの円弧・楕円
    archive.tag("CDataEnko");
    archive.base(1, 1);
    archive.reals(&[1.0, 2.0, 5.0, 0.0, 0.0, 0.0, 1.0]);
    archive.out.extend(1u32.to_le_bytes());
    archive.tag("CDataEnko");
    archive.base(1, 1);
    archive.reals(&[0.0, 0.0, 5.0, 0.0, -PI / 2.0, PI / 2.0, 1.0]);
    archive.out.extend(0u32.to_le_bytes());
    archive.tag("CDataEnko");
    archive.base(1, 1);
    archive.reals(&[0.0, 0.0, 4.0, 0.0, 0.0, 0.0, 0.5]);
    archive.out.extend(1u32.to_le_bytes());
    // 点・仮点・記号の点
    archive.tag("CDataTen");
    archive.base(1, 2);
    archive.reals(&[1.0, 1.0]);
    archive.out.extend(0u32.to_le_bytes());
    archive.tag("CDataTen");
    archive.base(1, 2);
    archive.reals(&[2.0, 2.0]);
    archive.out.extend(1u32.to_le_bytes());
    archive.tag("CDataTen");
    archive.base(100, 2);
    archive.reals(&[3.0, 3.0]);
    archive.out.extend([0, 0, 0, 0, 3, 0, 0, 0]);
    archive.reals(&[45.0, 2.0]);
    // グループ 1 の文字と、設定の文字
    archive.group = 1;
    archive.tag("CDataMoji");
    archive.base(2, 1);
    archive.reals(&[10.0, 20.0, 16.0, 20.0]);
    archive.out.extend(0u32.to_le_bytes());
    archive.reals(&[3.0, 5.0, 0.0, 15.0]);
    archive.cstring("ＭＳ ゴシック");
    archive.cstring("図面");
    archive.tag("CDataMoji");
    archive.base(2, 1);
    archive.reals(&[0.0, -1000.0, 0.0, -1000.0]);
    archive.out.extend(0u32.to_le_bytes());
    archive.reals(&[3.0, 3.0, 0.0, 0.0]);
    archive.cstring("");
    archive.cstring("Printer_Orientation = 0");
    // 色を持つソリッドと、扇形の円のソリッド
    archive.group = 0;
    archive.tag("CDataSolid");
    archive.base(1, 10);
    archive.reals(&[0.0, 0.0, 1.0, 0.0, 1.0, 1.0, 0.0, 1.0]);
    archive.out.extend(0x00FF_0000u32.to_le_bytes());
    archive.tag("CDataSolid");
    archive.base(101, 1);
    archive.reals(&[0.0, 0.0, 1.0, 1.0, 0.0, 0.0, PI / 2.0, 0.0]);
    // ブロックの参照と、SXF の形を持つ寸法
    archive.tag("CDataBlock");
    archive.base(1, 1);
    archive.reals(&[5.0, 5.0, 2.0, 3.0, PI / 2.0]);
    archive.out.extend(1u32.to_le_bytes());
    archive.tag("CDataSunpou");
    archive.base(1, 1);
    archive.base(1, 2);
    archive.reals(&[0.0, 0.0, 10.0, 0.0]);
    archive.base(1, 2);
    archive.text_body("1000");
    archive.out.extend(1u16.to_le_bytes());
    for x in [0.0, 10.0] {
        archive.base(1, 2);
        archive.reals(&[x, -2.0, x, 1.0]);
    }
    for [x, y] in [[0.0, 0.0], [10.0, 0.0], [0.0, -2.0], [10.0, -2.0]] {
        archive.base(1, 2);
        archive.reals(&[x, y]);
        archive.out.extend(0u32.to_le_bytes());
    }
    // 同梱画像を指す文字・ほかの制御文字・属性を持つ線
    archive.tag("CDataMoji");
    archive.base(1, 1);
    archive.reals(&[10.0, 10.0, 10.0, 10.0]);
    archive.out.extend(0u32.to_le_bytes());
    archive.reals(&[3.0, 3.0, 0.0, 90.0]);
    archive.cstring("");
    archive.cstring("^@BM%temp%a.jpg,40,30,0,0,1,0,255,255,255");
    archive.tag("CDataMoji");
    archive.base(1, 1);
    archive.reals(&[1.0, 2.0, 1.0, 2.0]);
    archive.out.extend(0u32.to_le_bytes());
    archive.reals(&[3.0, 3.0, 0.0, 0.0]);
    archive.cstring("");
    archive.cstring("^@XX");
    archive.curve = 5;
    archive.flag = 0x20;
    archive.line();
    archive.curve = 0;
    archive.flag = 0;
    17
}

#[cfg(test)]
fn drawing() -> (Drawing2D, Vec<Diagnostic>) {
    let mut archive = Archive::new(700);
    archive.count(17);
    entities(&mut archive);
    archive.count(1);
    archive.block("部品");
    archive.out.extend(1u32.to_le_bytes());
    archive.cstring("a.jpg.gz");
    archive.out.extend(3u32.to_le_bytes());
    archive.out.extend([1, 2, 3]);
    let mut src = spec().build();
    src.extend(&archive.out);
    lift(&src)
}

#[cfg(test)]
fn elements(drawing: &Drawing2D, space: u32) -> &[Element<ModelPoint>] {
    &drawing
        .spaces
        .iter()
        .find(|s| s.id == SpaceId(space))
        .unwrap()
        .elements
}

#[test]
fn sheet_layers_and_groups() {
    let (drawing, diag) = drawing();
    let sheet = &drawing.sheets[0];
    assert_eq!(sheet.size, Some([PaperLength(420.0), PaperLength(297.0)]));
    // グループ 0 は 1/100 で、グループ 1 は縮尺が 0 なので 1/1。原点は用紙の中央
    assert_eq!(drawing.spaces.len(), 2);
    assert_eq!(drawing.spaces[0].unit, Unit::Millimeter);
    assert_eq!(
        sheet.views[0].transform.m,
        [0.01, 0.0, 210.0, 0.0, 0.01, 148.5]
    );
    assert_eq!(
        sheet.views[1].transform.m,
        [1.0, 0.0, 210.0, 0.0, 1.0, 148.5]
    );
    assert_eq!(drawing.layers.len(), 256);
    let layer = drawing.layer(LayerId(1)).unwrap();
    assert_eq!(layer.name, "通り芯");
    assert_eq!(
        layer.state,
        Some(LayerState {
            visible: true,
            printable: true,
            editable: true,
            frozen: false,
        })
    );
    assert_eq!(drawing.layer(LayerId(2)).unwrap().name, "0-1");
    assert!(!drawing.layer(LayerId(2)).unwrap().state.unwrap().visible);
    let shown = drawing.layer(LayerId(17)).unwrap().state.unwrap();
    assert!(shown.visible && !shown.editable);
    assert_eq!(drawing.layer(LayerId(256)).unwrap().name, "F-F");
    assert!(diag.is_empty(), "{diag:?}");
    // 設定の文字は、図面の設定にする
    let setting = drawing
        .extensions
        .iter()
        .find(|e| e.namespace == "jww.setting")
        .unwrap();
    assert_eq!(setting.namespace, "jww.setting");
    assert_eq!(setting.name, "Printer_Orientation");
}

#[test]
fn lines_take_colors_line_types_and_widths() {
    let (drawing, _) = drawing();
    let lines = elements(&drawing, 0);
    let Shape::Line { start, end } = lines[0].shape else {
        panic!("{:?}", lines[0].shape);
    };
    assert_eq!([start, end], [model(0, 0.0, 0.0), model(0, 1000.0, 0.0)]);
    assert_eq!(lines[0].layer, LayerId(1));
    assert_eq!(
        lines[0].style.color,
        Inherit::Own(Color {
            code: Some(ColorCode {
                system: ColorSystem::Jww,
                value: 2
            }),
            rgb: Some([255, 0, 0]),
        })
    );
    // 図形の線幅がなければ、ペンの印刷の幅（1/100 mm）
    assert_eq!(
        lines[0].style.line_weight,
        Some(Inherit::Own(LineWeight::Width(PaperLength(0.35))))
    );
    let Inherit::Own(dashed) = lines[0].style.line_type else {
        panic!();
    };
    let dashed = drawing.line_type(dashed).unwrap();
    assert_eq!(dashed.name, "2");
    assert_eq!(
        dashed.definition.as_ref().unwrap().pattern,
        [4.0, -4.0, 4.0, -4.0]
    );
    assert_eq!(
        lines[1].style.line_weight,
        Some(Inherit::Own(LineWeight::Width(PaperLength(0.5))))
    );
    let Inherit::Own(color) = &lines[1].style.color else {
        panic!();
    };
    assert_eq!(color.rgb, Some([0, 255, 0]));
    let Inherit::Own(sxf) = lines[1].style.line_type else {
        panic!();
    };
    let sxf = drawing.line_type(sxf).unwrap();
    assert_eq!(sxf.name, "dashed");
    assert_eq!(sxf.definition.as_ref().unwrap().pattern, [6.0, -6.0]);
}

#[test]
fn arcs_and_ellipses() {
    let (drawing, _) = drawing();
    let shapes = elements(&drawing, 0);
    assert_eq!(
        shapes[2].shape,
        Shape::Circle {
            center: model(0, 100.0, 200.0),
            radius: Length::Model(ModelLength {
                space: SpaceId(0),
                value: 500.0
            }),
        }
    );
    let Shape::Curve(Edge::Arc {
        start,
        end,
        counterclockwise,
        ..
    }) = shapes[3].shape
    else {
        panic!("{:?}", shapes[3].shape);
    };
    assert!(close(start, 90.0) && close(end, 0.0) && !counterclockwise);
    let Shape::Curve(Edge::Elliptic {
        major,
        ratio,
        start,
        end,
        ..
    }) = shapes[4].shape
    else {
        panic!("{:?}", shapes[4].shape);
    };
    assert_eq!(major, model(0, 400.0, 0.0));
    assert!(close(ratio, 0.5) && close(start, 0.0) && close(end, 360.0));
}

#[test]
fn points_texts_and_solids() {
    let (drawing, _) = drawing();
    let shapes = elements(&drawing, 0);
    // 点は文書の点の表し方に従う点に、仮点は作図の補助の点に、記号の点は既定義の印にし、記号にしない（決定 0160）
    let point = |shape: &Shape<ModelPoint>| match shape {
        Shape::Point(point) => point.clone(),
        other => panic!("{other:?}"),
    };
    assert_eq!(
        point(&shapes[5].shape),
        Point {
            position: model(0, 100.0, 100.0),
            rotation: 0.0,
            look: PointLook::Document,
            construction: false,
        }
    );
    let construction = point(&shapes[6].shape);
    assert!(construction.construction);
    assert_eq!(construction.look, PointLook::Document);
    assert_eq!(
        point(&shapes[7].shape),
        Point {
            position: model(0, 300.0, 300.0),
            rotation: 45.0,
            look: PointLook::Marker {
                key: MarkerKey {
                    namespace: "jww.point".into(),
                    code: "3".into(),
                },
                scale: 2.0,
            },
            construction: false,
        }
    );
    assert!(
        drawing
            .symbols
            .iter()
            .all(|s| s.key.namespace != "jww.point")
    );
    assert_eq!(drawing.point_style, None);

    let texts = elements(&drawing, 1);
    let Shape::Text(text) = &texts[0].shape else {
        panic!("{:?}", texts[0].shape);
    };
    assert_eq!(text.content, "図面");
    assert_eq!(text.style, "ＭＳ ゴシック");
    assert_eq!(text.height.value, Length::Paper(PaperLength(5.0)));
    assert!(close(text.width_factor, 0.6));
    assert_eq!(text.rotation, 15.0);
    let Placement::Anchor { point, .. } = text.placement else {
        panic!();
    };
    assert_eq!(point, model(1, 10.0, 20.0));
    assert_eq!(texts.len(), 1);

    let Shape::Hatch(solid) = &shapes[8].shape else {
        panic!("{:?}", shapes[8].shape);
    };
    assert_eq!(solid.fill, Fill::Solid);
    let Boundary::Polyline { vertices, closed } = &solid.loops[0].boundary else {
        panic!();
    };
    assert!(*closed);
    assert_eq!(vertices[2].point, model(0, 100.0, 100.0));
    assert_eq!(
        shapes[8].style.color,
        Inherit::Own(Color {
            code: None,
            rgb: Some([0, 0, 255]),
        })
    );
    let Shape::Hatch(sector) = &shapes[9].shape else {
        panic!("{:?}", shapes[9].shape);
    };
    let Boundary::Edges(edges) = &sector.loops[0].boundary else {
        panic!();
    };
    assert_eq!(edges.len(), 3);
    let Edge::Line { start, end } = edges[0] else {
        panic!();
    };
    assert_eq!(start, model(0, 0.0, 0.0));
    assert!(close(end.x, 100.0) && close(end.y, 0.0));
    assert_eq!(shapes.len(), 14);
}

#[test]
fn zero_headers_have_hidden_layers_and_no_widths() {
    // 版 600 は SXF の欄を持ち、版 300 は持たない
    for version in [300, 600] {
        let mut archive = Archive::new(version);
        archive.count(1);
        archive.layer = 1;
        archive.group = 2;
        archive.line();
        archive.count(0);
        let mut src = Spec::new(version).build();
        src.extend(&archive.out);
        let (drawing, diag) = lift(&src);
        assert!(diag.is_empty(), "{diag:?}");
        let line = &elements(&drawing, 2)[0];
        assert_eq!(line.layer, LayerId(2 * 16 + 1 + 1));
        // 線幅の単位が分からないので、線幅を持たない
        assert_eq!(line.style.line_weight, None);
        assert!(!drawing.layer(line.layer).unwrap().state.unwrap().visible);
    }
}

#[test]
fn blocks_and_dimensions() {
    let (drawing, _) = drawing();
    // 定義の中身は、定義の空間に用紙の mm のまま置く
    let block = &drawing.symbols[0];
    assert_eq!(block.key.namespace, "jww.block");
    assert_eq!(block.key.name, "1");
    assert_eq!(block.space.id, SpaceId(16));
    let Shape::Line { end, .. } = block.space.elements[0].shape else {
        panic!("{:?}", block.space.elements[0].shape);
    };
    assert_eq!(end, model(16, 10.0, 0.0));
    let named = drawing
        .extensions
        .iter()
        .find(|e| e.namespace == "jww.block")
        .unwrap();
    assert_eq!(named.name, "1");
    assert_eq!(named.fields[0].value, FieldValue::Text("部品".into()));

    // 参照は、尺度にレイヤグループの縮尺を掛ける
    let shapes = elements(&drawing, 0);
    let Shape::Symbol(insert) = &shapes[10].shape else {
        panic!("{:?}", shapes[10].shape);
    };
    assert_eq!(insert.key, block.key);
    assert_eq!(insert.position, model(0, 500.0, 500.0));
    assert_eq!(insert.scale, [200.0, 300.0]);
    assert!(close(insert.rotation, 90.0));
    assert_eq!(shapes[10].style.color, Inherit::Block);

    // 寸法は、基準点を寸法線の向きに測る
    let Shape::Dimension(dimension) = &shapes[11].shape else {
        panic!("{:?}", shapes[11].shape);
    };
    let DimensionKind::Linear {
        first,
        second,
        line,
        rotation,
        ..
    } = dimension.kind
    else {
        panic!("{:?}", dimension.kind);
    };
    assert_eq!(first, model(0, 0.0, -200.0));
    assert_eq!(second, model(0, 1000.0, -200.0));
    assert_eq!(line, model(0, 0.0, 0.0));
    assert_eq!(rotation, 0.0);
    let Label::Override { content, .. } = &dimension.label else {
        panic!();
    };
    assert_eq!(content, "1000");
    let extension = drawing
        .extensions
        .iter()
        .find(|e| e.namespace == "jww.dimension")
        .unwrap();
    assert!(extension.is_current(&drawing));
    assert_eq!(extension.target, element_at(&drawing, SpaceId(0), 11));
    // 付く先の要素を変えると失効する
    let mut edited = drawing.clone();
    let space = edited
        .spaces
        .iter_mut()
        .find(|s| s.id == SpaceId(0))
        .unwrap();
    space.elements[11].layer = LayerId(99);
    assert!(!extension.is_current(&edited));
    assert_eq!(edited.stale_extensions().count(), 1);
}

/// 空間の添字の要素を指す付く先。要素の ID で指す（決定 0156）
#[cfg(test)]
fn element_at(drawing: &Drawing2D, space: SpaceId, index: usize) -> Target {
    let mut found = None;
    let _ = zumen_drawing::id::each_element(drawing, |at, id, _| {
        if at == (Position::Model { space, index }) {
            found = Some(id);
        }
        Ok::<(), ()>(())
    });
    Target::Element(found.unwrap())
}

#[test]
fn images_and_edit_state() {
    let (drawing, diag) = drawing();
    assert!(diag.is_empty(), "{diag:?}");
    let find = |namespace: &str| -> Vec<&Extension> {
        drawing
            .extensions
            .iter()
            .filter(|e| e.namespace == namespace)
            .collect()
    };

    // 画像は、文字の始点を左下にした下敷きにし、同梱画像のレコードを定義にする
    let shapes = elements(&drawing, 0);
    let Shape::Underlay(image) = &shapes[12].shape else {
        panic!("{:?}", shapes[12].shape);
    };
    assert_eq!(image.kind, UnderlayKind::Image);
    assert_eq!(image.resource.path, "%temp%a.jpg");
    // 同梱画像のレコードは、文字のレコードより後ろにある
    assert!(image.resource.definition.record() > shapes[12].source.record());
    let UnderlayFrame::Corners { origin, right, top } = image.frame else {
        panic!("{:?}", image.frame);
    };
    assert_eq!(origin, model(0, 1000.0, 1000.0));
    assert!(close(right.x, 1000.0) && close(right.y, 5000.0));
    assert!(close(top.x, -2000.0) && close(top.y, 1000.0));
    let extra = &find("jww.image")[0];
    assert_eq!(
        extra.fields[0].value,
        FieldValue::Text("0,0,1,0,255,255,255".into())
    );
    assert!(extra.is_current(&drawing));

    // ほかの制御文字は、描かずに元のまま持つ
    let control = &find("jww.control")[0];
    assert_eq!(control.name, "^@XX");
    assert_eq!(control.target, Target::Document);

    // 曲線属性と属性のフラグ
    let entity = &find("jww.entity")[0];
    assert_eq!(entity.target, element_at(&drawing, SpaceId(0), 13));
    assert_eq!(entity.fields[1].value, FieldValue::Integer(0x20));

    // 書き込むグループ・グループの名前・レイヤの保護
    assert_eq!(find("jww.edit")[0].fields[0].value, FieldValue::Integer(0));
    let groups = find("jww.group");
    assert_eq!(groups.len(), 16);
    assert_eq!(groups[0].fields[0].value, FieldValue::Text("平面".into()));
    let layers = find("jww.layer");
    assert_eq!(layers.len(), 1);
    assert_eq!(layers[0].name, "0-2");
}
