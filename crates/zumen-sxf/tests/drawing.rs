//! SFC から図面モデルへの変換（決定 0121）。入力は、どれも試験の中で作る。

use zumen_base::{Budget, CancelToken, Code, Cx, Diagnostic, Diagnostics, Ref, codes};
use zumen_drawing::{
    Arrowhead, Boundary, CalloutContent, ColorCode, ColorSystem, DimensionKind, Drawing2D, Edge,
    FieldValue, Fill, Flow, HeightMeaning, Horizontal, Inherit, Label, Length, LineWeight,
    ModelLength, ModelPoint, PaperLength, PaperPoint, PatternOrigin, Rotation, Scaled, Shape,
    SheetItem, SpaceId, SymbolKey, Target, UnderlayFrame, Vertical,
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
        b"future_feature('1','1','1','1')",
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
        Some(format!("{:?}", zumen_base::Value::from("future_feature")))
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

#[test]
fn moves_text_with_its_box() {
    let src = sfc(&[
        b"layer_feature(\\'L\\','1')",
        b"text_font_feature(\\'\x82\x6c\x82\x72 \x83\x53\x83\x56\x83\x62\x83\x4e\\')",
        b"text_string_feature('1','2','1',\\'\x95\x5c a\\','10.0','20.0','3.5','30.0','0.5','15.0','10.0','6','2')",
        b"text_string_feature('1','2','2',\\'b\\','0','0','1','1','0','0','0','1')",
        b"text_string_feature('1','2','1',\\'c\\','0','0','1','1','0','0','0','10','1')",
        b"sfig_org_feature(\\'A\\','1')",
        b"sfig_locate_feature('1',\\'A\\','0','0','0','1','1')",
        b"text_string_feature('1','2','1',\\'d\\','0','0','2','4','0','0','0','1','1')",
        b"drawing_sheet_feature(\\'S\\','3','1','0','0')",
    ]);
    let (drawing, diag) = lift(&src);
    let text = &drawing.spaces[0].elements[0];
    let Shape::Formatted(text) = &text.shape else {
        panic!("{:?}", text.shape);
    };
    // 「表」の 2 バイト目の `\` は、エスケープとして戻さない
    assert_eq!(text.content, "表 a");
    assert_eq!(text.height.meaning, HeightMeaning::Frame);
    assert_eq!(text.horizontal, Horizontal::Right);
    assert_eq!(text.vertical, Vertical::Middle);
    assert_eq!(text.rotation, Rotation::Angle(15.0));
    assert_eq!(text.flow, Flow::TopToBottom);
    assert_eq!(text.width, None);
    let format = &text.spans[0].format;
    assert_eq!(format.oblique, Some(10.0));
    assert_eq!(format.font.as_ref().unwrap().name, "ＭＳ ゴシック");
    let extension = drawing
        .extensions
        .iter()
        .find(|e| e.namespace == "sxf.text" && matches!(e.target, Target::Model { .. }))
        .unwrap();
    assert_eq!(
        extension.target,
        Target::Model {
            space: SpaceId(0),
            index: 0
        }
    );
    assert_eq!(extension.fields[0].value, FieldValue::Real(30.0));
    assert_eq!(extension.fields[1].value, FieldValue::Real(0.5));
    assert!(extension.is_current(&drawing));

    // 書く向きのない文字は横書き。フォントの定義がなければ、コードを報告する
    let space = &drawing.spaces[0];
    let Shape::Formatted(b) = &space.elements[1].shape else {
        panic!();
    };
    assert_eq!(
        (b.flow, b.horizontal, b.vertical),
        (Flow::LeftToRight, Horizontal::Left, Vertical::Bottom)
    );
    assert!(b.spans[0].format.font.is_none());
    assert_eq!(space.elements[1].style.line_type, Inherit::Layer);
    let missing = find(&diag, codes::SXF_CODE_MISSING);
    assert_eq!(
        missing.param("kind"),
        Some(&zumen_base::Value::from("font"))
    );
    // 9 点の外の配置は読めない
    find(&diag, codes::SXF_LIFT_INVALID);
    assert_eq!(space.elements.len(), 2);
    assert_eq!(drawing.sheets[0].elements.len(), 1);
    assert!(
        drawing
            .extensions
            .iter()
            .filter(|e| e.namespace == "sxf.text")
            .all(|e| e.is_current(&drawing))
    );
}

#[test]
fn moves_clothoids_as_they_are_defined() {
    let src = sfc(&[
        b"layer_feature(\\'L\\','1')",
        b"clothoid_feature('1','1','1','1','10.0','20.0','50.0','1','30.0','-5.0','40.0')",
        b"clothoid_feature('1','1','1','1','0','0','0','0','0','0','1')",
        b"drawing_sheet_feature(\\'S\\','3','1','0','0')",
    ]);
    let (drawing, diag) = lift(&src);
    let elements = &drawing.sheets[0].elements;
    assert_eq!(
        elements[0].shape,
        Shape::Curve(Edge::Clothoid {
            origin: paper(10.0, 20.0),
            parameter: Length::Paper(PaperLength(50.0)),
            angle: 30.0,
            counterclockwise: false,
            start: Length::Paper(PaperLength(-5.0)),
            end: Length::Paper(PaperLength(40.0)),
        })
    );
    // パラメーターが 0 のものは読めない
    assert_eq!(elements.len(), 1);
    find(&diag, codes::SXF_LIFT_INVALID);
}

#[test]
fn moves_the_title_block_items() {
    // 「橋」は Shift_JIS で 0x8B 0xB4
    let src = sfc(&[
        b"drawing_attribute_feature(\\'A\\',\\'\x8b\xb4 \\','x','y','1')",
        b"drawing_attribute_feature(('bad'))",
        b"drawing_sheet_feature(\\'S\\','3','1','0','0')",
    ]);
    let (drawing, diag) = lift(&src);
    assert_eq!(drawing.title_blocks.len(), 1);
    let items = &drawing.title_blocks[0].items;
    assert_eq!(items.len(), 12);
    assert_eq!(items[1].key, Some("construction_name"));
    assert_eq!(items[1].name, "工事名");
    assert_eq!(items[1].value, "橋");
    assert_eq!(items[4].value, "1");
    assert_eq!(items[11].value, "");
    assert!(items[0].display.is_empty());
    assert_eq!(drawing.title_blocks[0].source, Ref::at_record(6));
    find(&diag, codes::SXF_LIFT_INVALID);
}

#[test]
fn moves_raster_groups_as_underlays() {
    let src = sfc(&[
        b"layer_feature(\\'L\\','1')",
        // 2 番目の点が上なので、4 番目の点が右下の角
        b"polyline_feature('1','1','1','1','5',(0,0,20,20,0),(0,10,10,0,0))",
        b"line_feature('1','1','1','1','0','0','1','1')",
        b"sfig_org_feature(\\'$$RASTER$$a.tif\\','3')",
        b"line_feature('1','1','1','1','0','0','1','1')",
        b"sfig_org_feature(\\'$$ATRU$$1$$x$$\x89\xe6\x91\x9c$$b$$c.jpg\\','3')",
        b"sfig_locate_feature('1',\\'$$RASTER$$a.tif\\','0','0','0','1','1')",
        b"sfig_locate_feature('1',\\'$$ATRU$$1$$x$$\x89\xe6\x91\x9c$$b$$c.jpg\\','0','0','0','1','1')",
        b"drawing_sheet_feature(\\'S\\','3','1','0','0')",
    ]);
    let (drawing, diag) = lift(&src);
    let elements = &drawing.sheets[0].elements;
    assert_eq!(elements.len(), 3);
    let Shape::Underlay(underlay) = &elements[0].shape else {
        panic!("{:?}", elements[0].shape);
    };
    assert_eq!(underlay.resource.path, "a.tif");
    assert_eq!(underlay.resource.definition, Ref::at_record(9));
    assert_eq!(
        underlay.frame,
        UnderlayFrame::Corners {
            origin: paper(0.0, 0.0),
            right: paper(20.0, 0.0),
            top: paper(0.0, 10.0),
        }
    );
    assert_eq!(elements[0].source, Ref::at_record(7));
    assert!(matches!(elements[1].shape, Shape::Line { .. }));
    // 「画像」の属性の名前のグループに、範囲を示す折れ線がなければ、画像を移さない
    let missing = find(&diag, codes::SXF_RASTER_FRAME_MISSING);
    assert_eq!(
        missing.param("name"),
        Some(&zumen_base::Value::from("$$ATRU$$1$$x$$画像$$b$$c.jpg"))
    );
}

#[test]
fn moves_splines_as_joined_bezier_curves() {
    let src = sfc(&[
        b"layer_feature(\\'L\\','1')",
        b"spline_feature('1','1','1','1','1','7',(0,1,2,3,4,5,6),(0,1,1,0,-1,-1,0))",
        b"spline_feature('1','1','1','1','0','5',(0,1,2,3,4),(0,1,1,0,0))",
        b"drawing_sheet_feature(\\'S\\','3','1','0','0')",
    ]);
    let (drawing, diag) = lift(&src);
    let elements = &drawing.sheets[0].elements;
    let Shape::Curve(Edge::Spline(spline)) = &elements[0].shape else {
        panic!("{:?}", elements[0].shape);
    };
    assert_eq!(spline.degree, 3);
    assert_eq!(
        spline.knots,
        [0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 2.0, 2.0, 2.0, 2.0]
    );
    assert_eq!(spline.control[3], paper(3.0, 0.0));
    assert!(!spline.closed);
    // 制御点が 3n + 1 個でないものは読めない
    assert_eq!(elements.len(), 1);
    find(&diag, codes::SXF_LIFT_INVALID);
}

#[test]
fn moves_point_markers_as_predefined_symbols() {
    let src = sfc(&[
        b"layer_feature(\\'L\\','1')",
        b"point_marker_feature('1','3','5.0','6.0','2','45.0','1.5')",
        b"drawing_sheet_feature(\\'S\\','3','1','0','0')",
    ]);
    let (drawing, _) = lift(&src);
    let element = &drawing.sheets[0].elements[0];
    let Shape::Symbol(symbol) = &element.shape else {
        panic!("{:?}", element.shape);
    };
    assert_eq!(symbol.key.namespace, "sxf.marker");
    assert_eq!(symbol.key.name, "2");
    assert_eq!(symbol.position, paper(5.0, 6.0));
    assert_eq!((symbol.scale, symbol.rotation), ([1.5, 1.5], 45.0));
    assert!(drawing.symbol(&symbol.key).is_none());
}

#[test]
fn moves_dimensions_with_their_definition_points() {
    let src = sfc(&[
        b"layer_feature(\\'L\\','1')",
        b"linear_dim_feature('1','1','1','1','0','10','100','10','1','0','0','0','2','0','12','1','100','0','100','2','100','12','1','1','0','10','1','1','2','100','10','1','1','1',\\'100\\','50','11','3.5','9','0.5','0','0','2','1')",
        b"curve_dim_feature('1','1','1','1','0','0','10','0','90','0','0','0','0','0','0','0','0','0','0','0','0','0','0','0','0','0','0','0','0','0','0','0','0','0')",
        b"angular_dim_feature('1','1','1','1','0','0','10','350','10','0','0','0','0','0','0','0','0','0','0','0','0','0','0','0','0','0','0','0','0','0','0','0','0','0')",
        b"radius_dim_feature('1','1','1','1','5','0','0','0','1','1','5','0','1','0')",
        b"diameter_dim_feature('1','1','1','1','-5','0','5','0','0','0','0','0','0','0','0','0','0','0','0')",
        b"radius_dim_feature('1','1','1','1','5','0','0','0','1','1','5','0','1','2')",
        b"drawing_sheet_feature(\\'S\\','3','1','0','0')",
    ]);
    let (drawing, diag) = lift(&src);
    let elements = &drawing.sheets[0].elements;
    let dimensions: Vec<_> = elements
        .iter()
        .map(|e| match &e.shape {
            Shape::Dimension(d) => d,
            other => panic!("{other:?}"),
        })
        .collect();
    // 補助線の基点が定義点で、寸法線は補助線に垂直
    assert_eq!(
        dimensions[0].kind,
        DimensionKind::Linear {
            first: paper(0.0, 0.0),
            second: paper(100.0, 0.0),
            line: paper(0.0, 10.0),
            rotation: 0.0,
            oblique: 0.0,
        }
    );
    assert_eq!(dimensions[0].measurement, None);
    let Label::Override { content, spans } = &dimensions[0].label else {
        panic!("{:?}", dimensions[0].label);
    };
    assert_eq!(content, "100");
    assert_eq!(
        spans[0].format.height,
        Some(Scaled::Value(Length::Paper(PaperLength(3.5))))
    );
    assert_eq!(dimensions[0].text.position, Some(paper(50.0, 11.0)));
    assert_eq!(
        dimensions[0].text.attachment,
        Some((Horizontal::Center, Vertical::Bottom))
    );
    let extension = drawing
        .extensions
        .iter()
        .find(|e| e.namespace == "sxf.dimension")
        .unwrap();
    let value = |key: &str| {
        &extension
            .fields
            .iter()
            .find(|f| f.key == key)
            .unwrap_or_else(|| panic!("{key} がない"))
            .value
    };
    assert_eq!(
        value("extension1_start"),
        &FieldValue::Point([0.0, 2.0, 0.0])
    );
    assert_eq!(value("line_end"), &FieldValue::Point([100.0, 10.0, 0.0]));
    assert_eq!(value("arrow2_direction"), &FieldValue::Integer(2));
    assert_eq!(value("text_width"), &FieldValue::Real(9.0));
    assert!(
        drawing
            .extensions
            .iter()
            .filter(|e| e.namespace == "sxf.dimension")
            .all(|e| e.is_current(&drawing))
    );

    // 弧長は角度に変えない。補助線がなければ、寸法の弧の両端を定義点にする
    let near = |p: PaperPoint, x: f64, y: f64| (p.x - x).abs() < 1e-9 && (p.y - y).abs() < 1e-9;
    let DimensionKind::ArcLength {
        center,
        first,
        second,
        arc,
    } = dimensions[1].kind
    else {
        panic!("{:?}", dimensions[1].kind);
    };
    assert!(near(center, 0.0, 0.0) && near(first, 10.0, 0.0) && near(second, 0.0, 10.0));
    let half = 10.0 * core::f64::consts::FRAC_1_SQRT_2;
    assert!(near(arc, half, half), "{arc:?}");
    assert_eq!(dimensions[1].label, Label::Hidden);
    // 角度の寸法の弧は、反時計回りに 0 度をまたぐ
    let DimensionKind::Angular { arc, .. } = dimensions[2].kind else {
        panic!("{:?}", dimensions[2].kind);
    };
    assert!(near(arc, 10.0, 0.0), "{arc:?}");
    // 半径は、矢印に近い端が円の上の点
    assert_eq!(
        dimensions[3].kind,
        DimensionKind::Radius {
            center: paper(0.0, 0.0),
            chord: paper(5.0, 0.0),
            leader: Length::Paper(PaperLength(0.0)),
        }
    );
    assert_eq!(
        dimensions[4].kind,
        DimensionKind::Diameter {
            chord: paper(-5.0, 0.0),
            opposite: paper(5.0, 0.0),
            leader: Length::Paper(PaperLength(0.0)),
        }
    );
    // 文字の有無が 0 と 1 でなければ読めない
    assert_eq!(dimensions.len(), 5);
    find(&diag, codes::SXF_LIFT_INVALID);
}

#[test]
fn moves_labels_and_balloons_as_callouts() {
    let src = sfc(&[
        b"layer_feature(\\'L\\','1')",
        b"label_feature('1','1','1','1','3','(0,10,20)','(0,5,5)','1','2.0','1','1',\\'A\\','21','5','2','4','0','0','0','1','1')",
        b"balloon_feature('1','1','1','1','2','(0,10)','(0,10)','15','15','5','0','1.0','1','1',\\'3\\','15','15','4','2','0','0','0','5','1')",
        b"label_feature('1','1','1','1','1','(0)','(0)','1','1.0','0')",
        b"drawing_sheet_feature(\\'S\\','3','1','0','0')",
    ]);
    let (drawing, diag) = lift(&src);
    let elements = &drawing.sheets[0].elements;
    assert_eq!(elements.len(), 2);
    let Shape::Callout(label) = &elements[0].shape else {
        panic!("{:?}", elements[0].shape);
    };
    // 頂点は矢印の側から並ぶ
    assert_eq!(
        label.branches[0].lines,
        [vec![paper(0.0, 0.0), paper(10.0, 5.0), paper(20.0, 5.0)]]
    );
    assert_eq!(
        label.arrowhead,
        Arrowhead::Symbol(SymbolKey {
            namespace: "sxf.arrow",
            name: "1".into(),
        })
    );
    assert_eq!(label.scale, 2.0);
    let CalloutContent::Text(text) = &label.content else {
        panic!("{:?}", label.content);
    };
    assert_eq!((text.content.as_str(), text.point), ("A", paper(21.0, 5.0)));

    let Shape::Callout(balloon) = &elements[1].shape else {
        panic!("{:?}", elements[1].shape);
    };
    let CalloutContent::Text(text) = &balloon.content else {
        panic!("{:?}", balloon.content);
    };
    assert_eq!(
        (text.horizontal, text.vertical),
        (Horizontal::Center, Vertical::Middle)
    );
    let extension = drawing
        .extensions
        .iter()
        .find(|e| e.namespace == "sxf.callout" && e.fields[0].key == "center")
        .unwrap();
    assert_eq!(
        extension.fields[0].value,
        FieldValue::Point([15.0, 15.0, 0.0])
    );
    assert_eq!(extension.fields[1].value, FieldValue::Real(5.0));
    assert!(
        drawing
            .extensions
            .iter()
            .filter(|e| e.namespace == "sxf.callout")
            .all(|e| e.is_current(&drawing))
    );
    // 頂点が 1 つの引出線は読めない
    find(&diag, codes::SXF_LIFT_INVALID);
}

#[test]
fn moves_hatches_with_composite_curve_boundaries() {
    let src = sfc(&[
        b"layer_feature(\\'L\\','1')",
        b"line_feature('1','1','1','1','0','0','10','0')",
        b"polyline_feature('1','1','1','1','3','(10,10,0)','(0,10,10)')",
        b"line_feature('1','1','1','1','0','10','0','0')",
        b"composite_curve_org_feature('2','1','1','1')",
        b"circle_feature('1','1','1','1','5','5','1')",
        b"composite_curve_org_feature('1','1','1','0')",
        b"fill_area_style_colour_feature('1','3','1','1','(2)')",
        b"fill_area_style_hatching_feature('1','1','(4,1,1,0,0,2,90)','1','0','()')",
        b"externally_defined_hatch_feature('1',\\'Area_control\\','1','0','()')",
        b"fill_area_style_tiles_feature('1',\\'P\\','2','0','0','1','0','1','90','2','2','30','1','0','()')",
        b"fill_area_style_colour_feature('1','3','9','0','()')",
        b"drawing_sheet_feature(\\'S\\','3','1','0','0')",
    ]);
    let (drawing, diag) = lift(&src);
    let elements = &drawing.sheets[0].elements;
    // 表示する複合曲線の曲線は、複合曲線の色で置く。表示しない複合曲線は置かない
    assert_eq!(elements.len(), 7, "{diag:?}");
    assert!(matches!(elements[1].shape, Shape::Polyline(_)));
    assert_eq!(elements[0].style.color, elements[2].style.color);
    let hatches: Vec<_> = elements[3..]
        .iter()
        .map(|e| match &e.shape {
            Shape::Hatch(hatch) => hatch,
            other => panic!("{other:?}"),
        })
        .collect();
    let solid = hatches[0];
    assert_eq!(solid.fill, Fill::Solid);
    assert_eq!(solid.loops.len(), 2);
    let Boundary::Edges(outer) = &solid.loops[0].boundary else {
        panic!();
    };
    assert_eq!(outer.len(), 4);
    assert_eq!(
        outer[1],
        Edge::Line {
            start: paper(10.0, 0.0),
            end: paper(10.0, 10.0),
        }
    );
    assert!(solid.loops[0].external && !solid.loops[1].external);
    assert_eq!(solid.loops[0].sources.len(), 3);
    let Boundary::Edges(hole) = &solid.loops[1].boundary else {
        panic!();
    };
    assert!(matches!(hole[0], Edge::Arc { end: 360.0, .. }));

    // 線のハッチングは、間隔を線に垂直なずれにする
    let Fill::Pattern(pattern) = &hatches[1].fill else {
        panic!("{:?}", hatches[1].fill);
    };
    assert_eq!(pattern.origin, PatternOrigin::UserDefined);
    let line = &pattern.lines[0];
    assert_eq!((line.angle, line.base), (90.0, [0.0, 0.0]));
    assert!((line.offset[0] + 2.0).abs() < 1e-12 && line.offset[1].abs() < 1e-12);
    let extension = drawing
        .extensions
        .iter()
        .find(|e| e.namespace == "sxf.hatch")
        .unwrap();
    assert_eq!(extension.fields[0].value, FieldValue::Integer(4));

    let Fill::Pattern(external) = &hatches[2].fill else {
        panic!();
    };
    assert_eq!(
        (external.name.as_str(), external.origin),
        ("Area_control", PatternOrigin::Predefined)
    );
    assert_eq!(elements[5].style.color, Inherit::Layer);
    let Fill::Pattern(tiles) = &hatches[3].fill else {
        panic!();
    };
    assert_eq!(
        (tiles.name.as_str(), tiles.angle, tiles.scale),
        ("P", 30.0, 2.0)
    );
    assert!(
        drawing
            .extensions
            .iter()
            .filter(|e| e.namespace == "sxf.hatch")
            .all(|e| e.is_current(&drawing))
    );
    // 複合曲線のない番号は読めない
    find(&diag, codes::SXF_LIFT_INVALID);
}

#[test]
fn moves_parts_predefined_symbols_and_nested_views() {
    let src = sfc(&[
        b"layer_feature(\\'L\\','1')",
        b"line_feature('1','1','1','1','0','0','1','0')",
        b"sfig_org_feature(\\'P\\','4')",
        b"line_feature('1','1','1','1','0','0','5','5')",
        b"sfig_org_feature(\\'B\\','1')",
        b"sfig_locate_feature('1',\\'B\\','10','0','0','1','1')",
        b"sfig_locate_feature('1',\\'P\\','2','3','90','2','2')",
        b"sfig_org_feature(\\'A\\','1')",
        b"sfig_locate_feature('1',\\'A\\','100','50','0','2','2')",
        b"externally_defined_symbol_feature('1','1','3',\\'arrow\\','5','6','0','1.5')",
        b"drawing_sheet_feature(\\'S\\','3','1','0','0')",
    ]);
    let (drawing, diag) = lift(&src);
    assert!(diag.is_empty(), "{diag:?}");
    // 作図部品は、原点を基点にした記号の定義
    let key = SymbolKey {
        namespace: "sxf.sfig",
        name: "P".into(),
    };
    let part = drawing.symbol(&key).unwrap();
    assert_eq!(part.space.elements.len(), 1);
    assert_eq!(part.base.map(|b| (b.x, b.y)), Some((0.0, 0.0)));
    assert!(drawing.spaces.iter().all(|s| s.id != part.space.id));
    let a = drawing.spaces.iter().find(|s| s.id == SpaceId(2)).unwrap();
    let Shape::Symbol(placed) = &a.elements[0].shape else {
        panic!("{:?}", a.elements[0].shape);
    };
    assert_eq!(placed.key, key);
    assert_eq!(
        (
            placed.position.x,
            placed.position.y,
            placed.scale,
            placed.rotation
        ),
        (2.0, 3.0, [2.0, 2.0], 90.0)
    );

    // 部分図の中の部分図は、外の部分図のビューに重ねて用紙に置く
    let views = &drawing.sheets[0].views;
    assert_eq!(views.len(), 2);
    assert_eq!(views[0].space, SpaceId(2));
    assert_eq!(views[1].space, SpaceId(1));
    assert_eq!(views[1].transform.m, [2.0, 0.0, 120.0, 0.0, 2.0, 50.0]);

    let element = &drawing.sheets[0].elements[0];
    let Shape::Symbol(symbol) = &element.shape else {
        panic!("{:?}", element.shape);
    };
    assert_eq!(
        (symbol.key.namespace, symbol.key.name.as_str()),
        ("sxf.symbol", "arrow")
    );
    assert_eq!(symbol.scale, [1.5, 1.5]);
    assert!(matches!(element.style.color, Inherit::Own(_)));
}

#[test]
fn keeps_attribute_mechanism_names_on_the_elements() {
    let src = sfc(&[
        b"layer_feature(\\'L\\','1')",
        b"line_feature('1','1','1','1','0','0','1','0')",
        b"line_feature('1','1','1','1','0','0','0','1')",
        b"sfig_org_feature(\\'$$ATRU$$1$$contour$$height$$12.5\\','3')",
        b"line_feature('1','1','1','1','0','0','2','2')",
        b"sfig_locate_feature('1',\\'$$ATRU$$1$$contour$$height$$12.5\\','0','0','0','1','1')",
        b"drawing_sheet_feature(\\'S\\','3','1','0','0')",
    ]);
    let (drawing, diag) = lift(&src);
    assert!(diag.is_empty(), "{diag:?}");
    let attributes: Vec<_> = drawing
        .extensions
        .iter()
        .filter(|e| e.namespace == "sxf.attribute")
        .collect();
    // 属性のグループの 2 本の線（用紙の 2 本目と 3 本目）に付け、グループの外の線には付けない
    let indices: Vec<_> = attributes
        .iter()
        .map(|e| match e.target {
            Target::Paper { index, .. } => index,
            _ => panic!("{:?}", e.target),
        })
        .collect();
    assert_eq!(indices, [1, 2]);
    let parts: Vec<_> = attributes[0]
        .fields
        .iter()
        .filter(|f| f.key == "part")
        .map(|f| f.value.clone())
        .collect();
    assert_eq!(
        parts,
        ["ATRU", "1", "contour", "height", "12.5"].map(|p| FieldValue::Text(p.into()))
    );
    assert!(attributes.iter().all(|e| e.is_current(&drawing)));
}
