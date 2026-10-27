//! JWW から図面モデルへの変換（決定 0134）。入力は、どれも試験の中で作る。

mod common;

use core::f64::consts::PI;

use common::{Archive, Spec};
use zumen_base::{Budget, CancelToken, Code, Cx, Diagnostic, Diagnostics, codes};
use zumen_drawing::{
    Boundary, Color, ColorCode, ColorSystem, Drawing2D, Edge, Element, Fill, Inherit, LayerId,
    LayerState, Length, LineWeight, ModelLength, ModelPoint, PaperLength, Placement, Shape,
    SpaceId, Unit,
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

#[cfg(test)]
fn find(diag: &[Diagnostic], code: Code) -> &Diagnostic {
    diag.iter()
        .find(|d| d.code() == code)
        .unwrap_or_else(|| panic!("{code:?} がない: {diag:?}"))
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
    // ブロックの参照は、まだ移さない
    archive.tag("CDataBlock");
    archive.base(1, 1);
    archive.reals(&[5.0, 5.0, 1.0, 1.0, 0.0]);
    archive.out.extend(0u32.to_le_bytes());
    13
}

#[cfg(test)]
fn drawing() -> (Drawing2D, Vec<Diagnostic>) {
    let mut archive = Archive::new(700);
    archive.count(13);
    entities(&mut archive);
    archive.count(1);
    archive.block("部品");
    archive.out.extend(0u32.to_le_bytes());
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
            editable: true
        })
    );
    assert_eq!(drawing.layer(LayerId(2)).unwrap().name, "0-1");
    assert!(!drawing.layer(LayerId(2)).unwrap().state.unwrap().visible);
    let shown = drawing.layer(LayerId(17)).unwrap().state.unwrap();
    assert!(shown.visible && !shown.editable);
    assert_eq!(drawing.layer(LayerId(256)).unwrap().name, "F-F");
    // ブロックの参照と定義は、まだ移さない
    let dropped: Vec<_> = diag
        .iter()
        .filter(|d| d.code() == codes::JWW_LIFT_DROPPED)
        .collect();
    assert_eq!(dropped.len(), 2, "{diag:?}");
    assert_eq!(
        find(&diag, codes::JWW_LIFT_DROPPED).param("kind"),
        Some(&"CDataBlock".into())
    );
    assert_eq!(diag.len(), 2, "{diag:?}");
    // 設定の文字は、図面の設定にする
    let setting = &drawing.extensions[0];
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
    // 仮点は描かない
    let Shape::Symbol(point) = &shapes[5].shape else {
        panic!("{:?}", shapes[5].shape);
    };
    assert_eq!(point.key.namespace, "jww.point");
    assert_eq!(point.key.name, "point");
    let Shape::Symbol(symbol) = &shapes[6].shape else {
        panic!("{:?}", shapes[6].shape);
    };
    assert_eq!(symbol.key.name, "3");
    assert_eq!(symbol.position, model(0, 300.0, 300.0));
    assert_eq!((symbol.rotation, symbol.scale), (45.0, [2.0, 2.0]));

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

    let Shape::Hatch(solid) = &shapes[7].shape else {
        panic!("{:?}", shapes[7].shape);
    };
    assert_eq!(solid.fill, Fill::Solid);
    let Boundary::Polyline { vertices, closed } = &solid.loops[0].boundary else {
        panic!();
    };
    assert!(*closed);
    assert_eq!(vertices[2].point, model(0, 100.0, 100.0));
    assert_eq!(
        shapes[7].style.color,
        Inherit::Own(Color {
            code: None,
            rgb: Some([0, 0, 255]),
        })
    );
    let Shape::Hatch(sector) = &shapes[8].shape else {
        panic!("{:?}", shapes[8].shape);
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
    assert_eq!(shapes.len(), 9);
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
