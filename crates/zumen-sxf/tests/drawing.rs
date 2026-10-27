//! SFC から図面モデルへの変換（決定 0121）。入力は、どれも試験の中で作る。

use zumen_base::{Budget, CancelToken, Code, Cx, Diagnostic, Diagnostics, Ref, codes};
use zumen_drawing::{
    ColorCode, ColorSystem, Drawing2D, Edge, FieldValue, Length, LineWeight, ModelLength,
    ModelPoint, PaperLength, PaperPoint, Shape, SheetItem, SpaceId,
};
use zumen_sdk::{Lift, testkit};
use zumen_sxf::{Sxf, SxfFile};

#[cfg(test)]
fn sfc(features: &[&[u8]]) -> Vec<u8> {
    let mut src = b"ISO-10303-21;\nHEADER;\nFILE_DESCRIPTION(('SCADEC level2 feature_mode'),'2;1');\nFILE_SCHEMA(('ASSOCIATIVE_DRAUGHTING'));\nENDSEC;\nDATA;\n".to_vec();
    for (at, feature) in features.iter().enumerate() {
        src.extend_from_slice(format!("/*SXF3\n#{} = ", (at + 1) * 10).as_bytes());
        src.extend_from_slice(feature);
        src.extend_from_slice(b"\nSXF3*/\n");
    }
    src.extend_from_slice(b"ENDSEC;\nEND-ISO-10303-21;\n");
    src
}

#[cfg(test)]
fn lift(src: &[u8]) -> (Drawing2D, Vec<Diagnostic>) {
    let file: SxfFile = testkit::read::<Sxf>(src).0.unwrap();
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

#[cfg(test)]
fn paper(x: f64, y: f64) -> PaperPoint {
    PaperPoint {
        sheet: zumen_drawing::SheetId(0),
        x,
        y,
    }
}

#[test]
fn moves_tables_and_shapes_on_the_sheet() {
    // 「壁」は Shift_JIS で 0x95 0xC7
    let src = sfc(&[
        b"layer_feature(\\'\x95\xc7\\','1')",
        b"layer_feature(\\'hidden\\','0')",
        b"user_defined_colour_feature('10','20','30')",
        b"width_feature('0.3')",
        b"user_defined_font_feature(\\'mine\\','2','(2.0,1.0)')",
        b"line_feature('1','17','17','11','0.0','0.0','10.0','0.0')",
        b"circle_feature('2','2','2','3','5.0','5.0','3.0')",
        b"arc_feature('1','1','1','1','0.0','0.0','5.0','1','0.0','90.0')",
        b"ellipse_feature('1','1','1','1','0.0','0.0','2.0','4.0','30.0')",
        b"polyline_feature('1','1','1','1','3',(0,1,2),'(0,1,0)')",
        b"text_string_feature('1','1','1',\\'a\\','0','0','1','1','0','0','0','1','1')",
        b"drawing_sheet_feature(\\'S\\','3','1','0','0')",
    ]);
    let (drawing, diag) = lift(&src);
    let layer = &drawing.layers[0];
    assert_eq!(layer.name, "壁");
    assert!(layer.state.unwrap().visible);
    assert!(!drawing.layers[1].state.unwrap().visible);

    let sheet = &drawing.sheets[0];
    assert_eq!(sheet.size, Some([PaperLength(420.0), PaperLength(297.0)]));
    let extension = &drawing.extensions[0];
    assert_eq!(extension.namespace, "sxf.sheet");
    assert_eq!(extension.name, "S");
    assert_eq!(extension.fields[0].value, FieldValue::Integer(3));

    let elements = &sheet.elements;
    assert_eq!(elements.len(), 5);
    let line = &elements[0];
    assert_eq!(
        line.shape,
        Shape::Line {
            start: paper(0.0, 0.0),
            end: paper(10.0, 0.0)
        }
    );
    let zumen_drawing::Inherit::Own(color) = line.style.color else {
        panic!();
    };
    assert_eq!(
        color.code,
        Some(ColorCode {
            system: ColorSystem::Sxf,
            value: 17
        })
    );
    assert_eq!(color.rgb, Some([10, 20, 30]));
    assert_eq!(
        line.style.line_weight,
        Some(zumen_drawing::Inherit::Own(LineWeight::Width(PaperLength(
            0.3
        ))))
    );
    let mine = drawing
        .line_types
        .iter()
        .find(|t| t.name == "mine")
        .unwrap();
    assert_eq!(mine.definition.as_ref().unwrap().pattern, [2.0, -1.0]);
    assert_eq!(line.source, Ref::at_record(11));

    // 既定義の色は RGB を持たず、既定義線種は名前だけを持つ
    let circle = &elements[1];
    let zumen_drawing::Inherit::Own(color) = circle.style.color else {
        panic!();
    };
    assert_eq!(color.rgb, None);
    let zumen_drawing::Inherit::Own(dashed) = circle.style.line_type else {
        panic!();
    };
    let dashed = drawing.line_type(dashed).unwrap();
    assert_eq!(dashed.name, "dashed");
    assert!(dashed.definition.is_none());

    let Shape::Curve(Edge::Arc {
        counterclockwise, ..
    }) = &elements[2].shape
    else {
        panic!();
    };
    assert!(!counterclockwise);
    // Y の半径の方が大きいので、長軸は 30 度から 90 度回した 120 度の向き。角度も 90 度ずらす
    let Shape::Curve(Edge::Elliptic {
        major,
        ratio,
        start,
        ..
    }) = &elements[3].shape
    else {
        panic!();
    };
    assert!((major.x + 2.0).abs() < 1e-12);
    assert!((major.y - 2.0 * 3f64.sqrt()).abs() < 1e-12);
    assert_eq!(*ratio, 0.5);
    assert_eq!(*start, -90.0);
    let Shape::Polyline(polyline) = &elements[4].shape else {
        panic!();
    };
    assert_eq!(polyline.vertices[2].point, paper(2.0, 0.0));

    let dropped = find(&diag, codes::SXF_LIFT_DROPPED);
    assert_eq!(
        dropped.param("kind").map(|v| format!("{v:?}")),
        Some(format!(
            "{:?}",
            zumen_base::Value::from("text_string_feature")
        ))
    );
}

#[test]
fn sub_figures_become_spaces_placed_by_views() {
    let src = sfc(&[
        b"layer_feature(\\'L\\','1')",
        b"line_feature('1','1','1','1','0.0','0.0','1.0','0.0')",
        b"sfig_org_feature(\\'A\\','1')",
        b"line_feature('1','1','1','1','1.0','0.0','2.0','0.0')",
        b"sfig_org_feature(\\'N\\','2')",
        b"circle_feature('1','1','1','1','0.0','0.0','1.0')",
        b"sfig_org_feature(\\'G\\','3')",
        b"sfig_locate_feature('1',\\'A\\','100.0','50.0','90.0','0.5','0.5')",
        b"sfig_locate_feature('1',\\'N\\','0.0','0.0','0.0','1.0','1.0')",
        b"sfig_locate_feature('1',\\'G\\','7.0','7.0','45.0','2.0','2.0')",
        b"sfig_locate_feature('1',\\'Z\\','0.0','0.0','0.0','1.0','1.0')",
        b"line_feature('9','1','1','1','0.0','0.0','1.0','1.0')",
        b"drawing_sheet_feature(\\'S\\','9','1','300.0','200.0')",
    ]);
    let (drawing, diag) = lift(&src);
    assert_eq!(drawing.spaces.len(), 2);
    let space = &drawing.spaces[0];
    assert_eq!(space.id, SpaceId(0));
    assert_eq!(
        space.elements[0].shape,
        Shape::Line {
            start: ModelPoint {
                space: SpaceId(0),
                x: 0.0,
                y: 0.0
            },
            end: ModelPoint {
                space: SpaceId(0),
                x: 1.0,
                y: 0.0
            },
        }
    );
    let sheet = &drawing.sheets[0];
    assert_eq!(sheet.size, Some([PaperLength(300.0), PaperLength(200.0)]));
    let view = &sheet.views[0];
    let placed = view
        .place(ModelPoint {
            space: SpaceId(0),
            x: 2.0,
            y: 0.0,
        })
        .unwrap();
    assert!((placed.x - 100.0).abs() < 1e-12 && (placed.y - 51.0).abs() < 1e-12);
    assert_eq!(
        view.paper_length(
            ModelLength {
                space: SpaceId(0),
                value: 2.0
            },
            [1.0, 0.0]
        ),
        Some(PaperLength(1.0))
    );
    // 測地座標系は X が上（北）
    let north = sheet.views[1]
        .place(ModelPoint {
            space: SpaceId(1),
            x: 1.0,
            y: 0.0,
        })
        .unwrap();
    assert!(north.x.abs() < 1e-12 && (north.y - 1.0).abs() < 1e-12);

    // 作図グループは、配置の値によらず、そのまま展開する
    assert_eq!(
        sheet.elements[0].shape,
        Shape::Circle {
            center: paper(0.0, 0.0),
            radius: Length::Paper(PaperLength(1.0))
        }
    );
    assert_eq!(
        sheet.order[..3],
        [
            SheetItem::View(0),
            SheetItem::View(1),
            SheetItem::Element(0)
        ]
    );
    assert_eq!(
        find(&diag, codes::SXF_SFIG_MISSING).param("name"),
        Some(&zumen_base::Value::from("Z"))
    );
    let missing = find(&diag, codes::SXF_CODE_MISSING);
    assert_eq!(missing.param("code"), Some(&zumen_base::Value::from(9i64)));
    assert!(
        drawing
            .layers
            .iter()
            .any(|l| l.name == "9" && l.state.is_none())
    );
}

#[test]
fn reports_what_it_cannot_place() {
    let src = sfc(&[
        b"sfig_locate_feature('1',\\'G\\','0','0','0','1','1')",
        b"sfig_org_feature(\\'G\\','3')",
        b"sfig_locate_feature('1',\\'G\\','0','0','0','1','1')",
        b"line_feature('1','1','1','1','x','0','1','0')",
        b"drawing_sheet_feature(\\'S\\','3','1','0','0')",
        b"line_feature('1','1','1','1','0','0','1','0')",
    ]);
    let (drawing, diag) = lift(&src);
    find(&diag, codes::SXF_SFIG_CYCLE_INVALID);
    find(&diag, codes::SXF_LIFT_INVALID);
    find(&diag, codes::SXF_MEMBER_ASSUMED);
    assert_eq!(drawing.sheets[0].elements.len(), 1);

    let p21 = sfc(&[]);
    let p21 = String::from_utf8(p21)
        .unwrap()
        .replace("feature_mode", "AP202_mode");
    let (drawing, diag) = lift(p21.as_bytes());
    assert_eq!(drawing, Drawing2D::default());
    find(&diag, codes::SXF_P21_UNSUPPORTED);
}
