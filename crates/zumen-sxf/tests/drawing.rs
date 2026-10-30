//! SFC と P21 から図面モデルへの変換（決定 0121・0132）。入力は、どれも試験の中で作る。

use zumen_base::{Budget, CancelToken, Code, Cx, Diagnostic, Diagnostics, Key, Ref, codes};
use zumen_drawing::{
    Arrowhead, Boundary, CalloutContent, ColorCode, ColorSystem, DimensionKind, Drawing2D, Edge,
    FieldValue, Fill, Flow, HeightMeaning, Horizontal, Inherit, Label, Length, LineWeight,
    ModelLength, ModelPoint, PaperLength, PaperPoint, PatternOrigin, Position, Rotation, Scaled,
    Shape, SheetItem, SpaceId, SymbolKey, Target, UnderlayFrame, Vertical,
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
    // 出所は、レコードの番号と、変わらない鍵のインスタンスの番号を持つ（決定 0156）
    assert_eq!(line.source, Ref::at_record(11).with_key(Key::Instance(60)));

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
    assert!(drawing.sheets[0].elements.is_empty());
    find(&diag, codes::SXF_P21_SHEET_MISSING);
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
        .find(|e| {
            e.namespace == "sxf.text"
                && matches!(position(&drawing, &e.target), Some(Position::Model { .. }))
        })
        .unwrap();
    assert_eq!(
        position(&drawing, &extension.target),
        Some(Position::Model {
            space: SpaceId(0),
            index: 0
        })
    );
    assert_eq!(extension.fields[0].value, FieldValue::Real(30.0));
    assert_eq!(extension.fields[1].value, FieldValue::Real(0.5));
    assert!(extension.is_current(&drawing));
    // 付く先の文字を変えると失効する
    let mut edited = drawing.clone();
    edited.spaces[0].elements[0].layer = zumen_drawing::LayerId(99);
    assert!(!extension.is_current(&edited));

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
    assert_eq!(
        drawing.title_blocks[0].source,
        Ref::at_record(6).with_key(Key::Instance(10))
    );
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
    assert_eq!(
        underlay.resource.definition,
        Ref::at_record(9).with_key(Key::Instance(40))
    );
    assert_eq!(
        underlay.frame,
        UnderlayFrame::Corners {
            origin: paper(0.0, 0.0),
            right: paper(20.0, 0.0),
            top: paper(0.0, 10.0),
        }
    );
    assert_eq!(
        elements[0].source,
        Ref::at_record(7).with_key(Key::Instance(20))
    );
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
        .map(|e| match position(&drawing, &e.target) {
            Some(Position::Paper { index, .. }) => index,
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

#[test]
fn keeps_the_kind_of_a_free_sheet_of_a_standard_size() {
    // E6: 自由サイズの用紙は、定型（A3）と同じ大きさでも種別を保つ
    let src = sfc(&[
        b"layer_feature(\\'L\\','1')",
        b"drawing_sheet_feature(\\'S\\','9','1','420.0','297.0')",
    ]);
    let (drawing, _) = lift(&src);
    assert_eq!(
        drawing.sheets[0].size,
        Some([PaperLength(420.0), PaperLength(297.0)])
    );
    let extension = drawing
        .extensions
        .iter()
        .find(|e| e.namespace == "sxf.sheet")
        .unwrap();
    assert_eq!(extension.fields[0].key, "size");
    assert_eq!(extension.fields[0].value, FieldValue::Integer(9));
}

#[cfg(test)]
fn p21(instances: &str) -> Vec<u8> {
    format!("ISO-10303-21;\nHEADER;\nFILE_DESCRIPTION(('SCADEC level2 AP202_mode'),'2;1');\nFILE_SCHEMA(('ASSOCIATIVE_DRAUGHTING'));\nENDSEC;\nDATA;\n{instances}ENDSEC;\nEND-ISO-10303-21;\n").into_bytes()
}

/// 要素を指す付く先の、空間か用紙の中の位置
#[cfg(test)]
fn position(drawing: &Drawing2D, target: &Target) -> Option<Position> {
    let Target::Element(wanted) = target else {
        return None;
    };
    let mut found = None;
    let _ = zumen_drawing::id::each_element(drawing, |at, id, _| {
        if id == *wanted {
            found = Some(at);
        }
        Ok::<(), ()>(())
    });
    found
}

#[cfg(test)]
fn unsource<P>(elements: &mut [zumen_drawing::Element<P>]) {
    for element in elements {
        element.source = Ref::default();
        if let Shape::Hatch(hatch) = &mut element.shape {
            for hatch_loop in &mut hatch.loops {
                hatch_loop.sources.clear();
            }
        }
    }
}

/// 出どころを除いた図面モデル。SFC と P21 の比較に使う
#[cfg(test)]
fn unsourced(mut drawing: Drawing2D) -> Drawing2D {
    // 要素の ID は出所から作るので、拡張の付く先を位置に控え、出所を除いた後の ID とハッシュで指し直す
    let mut positions = std::collections::BTreeMap::new();
    let _ = zumen_drawing::id::each_element(&drawing, |at, id, _| {
        positions.insert(id, at);
        Ok::<(), ()>(())
    });
    for layer in &mut drawing.layers {
        layer.source = Ref::default();
    }
    for line_type in &mut drawing.line_types {
        line_type.source = Ref::default();
    }
    for extension in &mut drawing.extensions {
        extension.source = Ref::default();
    }
    for sheet in &mut drawing.sheets {
        unsource(&mut sheet.elements);
        for view in &mut sheet.views {
            view.source = Ref::default();
        }
    }
    for space in &mut drawing.spaces {
        unsource(&mut space.elements);
    }
    for symbol in &mut drawing.symbols {
        symbol.source = Ref::default();
        unsource(&mut symbol.space.elements);
    }
    let mut found = std::collections::BTreeMap::new();
    let _ = zumen_drawing::id::each_element(&drawing, |at, id, element| {
        found.insert(at, (id, element.hash()));
        Ok::<(), ()>(())
    });
    for extension in &mut drawing.extensions {
        let Target::Element(id) = &extension.target else {
            continue;
        };
        if let Some((id, hash)) = positions.get(id).and_then(|at| found.get(at)) {
            extension.target = Target::Element(id.clone());
            extension.basis = zumen_drawing::Basis::Hash(*hash);
        }
    }
    drawing
}

#[test]
fn moves_p21_curves_like_the_same_sfc() {
    let sfc = sfc(&[
        b"layer_feature(\\'\x95\xc7\\','1')",
        b"layer_feature(\\'hidden\\','0')",
        b"user_defined_colour_feature('10','20','30')",
        b"width_feature('0.3')",
        b"user_defined_font_feature(\\'mine\\','2','(2.0,1.0)')",
        b"line_feature('1','17','17','11','0','0','10','0')",
        b"circle_feature('2','3','2','3','5','5','3')",
        b"arc_feature('1','1','1','1','0','0','5','1','0','180')",
        b"ellipse_feature('1','1','1','1','0','0','2','4','90')",
        b"polyline_feature('1','1','1','1','3',(0,1,2),(0,1,0))",
        b"spline_feature('1','1','1','1','1','4',(0,1,2,3),(0,1,1,0))",
        b"clothoid_feature('1','1','1','1','0','0','2','0','0','0','1.5')",
        b"drawing_sheet_feature(\\'S\\','3','1','0','0')",
    ]);
    // 色・線種・線幅は CURVE_STYLE に、レイヤは PRESENTATION_LAYER_ASSIGNMENT に書く。用紙の枠（#91）は図形にしない
    let p21 = p21(concat!(
        "#1=COLOUR_RGB(' ',0.0392156862745098,0.0784313725490196,0.117647058823529);\n",
        "#2=DRAUGHTING_PRE_DEFINED_COLOUR('green');\n",
        "#3=DRAUGHTING_PRE_DEFINED_COLOUR('black');\n",
        "#4=DRAUGHTING_PRE_DEFINED_CURVE_FONT('continuous');\n",
        "#5=DRAUGHTING_PRE_DEFINED_CURVE_FONT('dashed');\n",
        "#6=CURVE_STYLE_FONT('mine',(#7));\n",
        "#7=CURVE_STYLE_FONT_PATTERN(2.,1.);\n",
        "#8=(LENGTH_UNIT() NAMED_UNIT(*) SI_UNIT(.MILLI.,.METRE.));\n",
        "#9=LENGTH_MEASURE_WITH_UNIT(POSITIVE_LENGTH_MEASURE(0.3),#8);\n",
        "#10=LENGTH_MEASURE_WITH_UNIT(POSITIVE_LENGTH_MEASURE(0.25),#8);\n",
        "#11=LENGTH_MEASURE_WITH_UNIT(POSITIVE_LENGTH_MEASURE(0.13),#8);\n",
        "#12=CURVE_STYLE(' ',#6,#9,#1);\n",
        "#13=PRESENTATION_STYLE_ASSIGNMENT((#12));\n",
        "#14=CURVE_STYLE(' ',#5,#10,#2);\n",
        "#15=PRESENTATION_STYLE_ASSIGNMENT((#14));\n",
        "#16=CURVE_STYLE(' ',#4,#11,#3);\n",
        "#17=PRESENTATION_STYLE_ASSIGNMENT((#16));\n",
        "#20=CARTESIAN_POINT(' ',(0.,0.));\n",
        "#21=CARTESIAN_POINT(' ',(10.,0.));\n",
        "#22=DIRECTION(' ',(1.,0.));\n",
        "#23=VECTOR(' ',#22,1.);\n",
        "#24=LINE(' ',#20,#23);\n",
        "#25=TRIMMED_CURVE(' ',#24,(#20),(#21),.T.,.CARTESIAN.);\n",
        "#26=(ANNOTATION_CURVE_OCCURRENCE() ANNOTATION_OCCURRENCE() DRAUGHTING_ANNOTATION_OCCURRENCE() GEOMETRIC_REPRESENTATION_ITEM() REPRESENTATION_ITEM(' ') STYLED_ITEM((#13),#25));\n",
        "#30=CARTESIAN_POINT(' ',(5.,5.));\n",
        "#31=AXIS2_PLACEMENT_2D(' ',#30,#22);\n",
        "#32=CIRCLE(' ',#31,3.);\n",
        "#33=ANNOTATION_CURVE_OCCURRENCE(' ',(#15),#32);\n",
        "#35=AXIS2_PLACEMENT_2D(' ',#20,#22);\n",
        "#36=CIRCLE(' ',#35,5.);\n",
        "#37=TRIMMED_CURVE(' ',#36,(PARAMETER_VALUE(0.)),(PARAMETER_VALUE(3.141592653589793)),.F.,.PARAMETER.);\n",
        "#38=ANNOTATION_CURVE_OCCURRENCE(' ',(#17),#37);\n",
        "#40=DIRECTION(' ',(0.,1.));\n",
        "#41=AXIS2_PLACEMENT_2D(' ',#20,#40);\n",
        "#42=ELLIPSE(' ',#41,2.,4.);\n",
        "#43=ANNOTATION_CURVE_OCCURRENCE(' ',(#17),#42);\n",
        "#50=CARTESIAN_POINT(' ',(1.,1.));\n",
        "#51=CARTESIAN_POINT(' ',(2.,0.));\n",
        "#52=POLYLINE(' ',(#20,#50,#51));\n",
        "#53=ANNOTATION_CURVE_OCCURRENCE(' ',(#17),#52);\n",
        "#54=CARTESIAN_POINT(' ',(1.,1.));\n",
        "#55=CARTESIAN_POINT(' ',(2.,1.));\n",
        "#56=CARTESIAN_POINT(' ',(3.,0.));\n",
        "#57=BEZIER_CURVE(' ',3,(#20,#54,#55,#56),.UNSPECIFIED.,.F.,.F.);\n",
        "#58=ANNOTATION_CURVE_OCCURRENCE(' ',(#17),#57);\n",
        "#60=CLOTHOID('0',#35,2.);\n",
        "#61=TRIMMED_CURVE(' ',#60,(PARAMETER_VALUE(0.)),(PARAMETER_VALUE(1.5)),.T.,.PARAMETER.);\n",
        "#62=ANNOTATION_CURVE_OCCURRENCE(' ',(#17),#61);\n",
        "#70=PRESENTATION_LAYER_ASSIGNMENT('\\X2\\58C1\\X0\\',' ',(#26,#38,#43,#53,#58,#62));\n",
        "#71=PRESENTATION_LAYER_ASSIGNMENT('hidden',' ',(#33));\n",
        "#72=INVISIBILITY((#71));\n",
        "#80=DRAUGHTING_TITLE((#81),'JAPANESE','S');\n",
        "#90=DRAWING_SHEET_REVISION('A3_horizontal',(#26,#33,#38,#43,#53,#58,#62,#91),#99,'01');\n",
        "#91=PLANAR_BOX(' ',420.,297.,#35);\n",
        "#92=PRESENTATION_SIZE(#90,#91);\n",
    ));
    let (from_sfc, sfc_diag) = lift(&sfc);
    let (from_p21, diag) = lift(&p21);
    assert!(sfc_diag.is_empty(), "{sfc_diag:?}");
    // 用紙の文脈（#99）と表題の版（#81）は、この試験では書かない
    assert_eq!(
        diag.iter().map(Diagnostic::code).collect::<Vec<_>>(),
        [codes::SXF_P21_REFERENCE_MISSING]
    );
    assert_eq!(from_p21.sheets[0].elements.len(), 7);
    assert_eq!(unsourced(from_p21.clone()), unsourced(from_sfc));
    // 出どころは、表記の実体のレコード。レコードは始まりの行から数える
    let records = |id: &str| {
        let text = String::from_utf8(p21.clone()).unwrap();
        let at = text.find(&format!("\n{id}=")).unwrap();
        text.get(..=at).unwrap().matches(";\n").count() as u64
    };
    assert_eq!(
        from_p21.sheets[0].elements[0].source,
        Ref::at_record(records("#26")).with_key(Key::Instance(26))
    );
}

#[test]
fn reports_p21_elements_it_does_not_move_yet() {
    let src = p21(concat!(
        "#1=PRESENTATION_STYLE_ASSIGNMENT(());\n",
        "#2=ANNOTATION_FILL_AREA_OCCURRENCE(' ',(#1),#3,$);\n",
        "#3=TEXT_LITERAL(' ','A',#4,.LEFT.,#5);\n",
        "#4=AXIS2_PLACEMENT_2D(' ',#6,$);\n",
        "#5=CARTESIAN_POINT(' ',(0.,0.));\n",
        "#6=CARTESIAN_POINT(' ',(0.,0.));\n",
        "#7=B_SPLINE_CURVE(' ',2,(#5,#6),.UNSPECIFIED.,.F.,.F.);\n",
        "#8=ANNOTATION_CURVE_OCCURRENCE(' ',(#1),#7);\n",
        "#9=DRAWING_SHEET_REVISION('FREE',(#2,#8),$,'01');\n",
    ));
    let (drawing, diag) = lift(&src);
    assert!(drawing.sheets[0].elements.is_empty());
    let kinds: Vec<String> = diag
        .iter()
        .filter(|d| d.code() == codes::SXF_LIFT_DROPPED)
        .filter_map(|d| d.param("kind").map(|v| format!("{v:?}")))
        .collect();
    assert_eq!(
        kinds,
        [
            format!(
                "{:?}",
                zumen_base::Value::from("annotation_fill_area_occurrence")
            ),
            format!("{:?}", zumen_base::Value::from("b_spline_curve")),
        ]
    );
}

#[test]
fn moves_p21_text_and_symbols_like_the_same_sfc() {
    // 「表」は Shift_JIS で 0x95 0x5C、Unicode で U+8868
    let sfc = sfc(&[
        b"layer_feature(\\'L\\','1')",
        b"text_font_feature(\\'MS\\')",
        b"text_string_feature('1','2','1',\\'\x95\x5c a\\','10','20','3.5','30','0.5','90','10','6','2')",
        b"text_string_feature('1','2','1',\\'b\\','0','0','1','1','0','0','0','1','1')",
        b"point_marker_feature('1','3','5','6','2','45','1.5')",
        b"externally_defined_symbol_feature('1','1','3',\\'arrow\\','5','6','0','1.5')",
        b"externally_defined_symbol_feature('1','0','0',\\'tree\\','1','2','0','1')",
        b"drawing_sheet_feature(\\'S\\','3','1','0','0')",
    ]);
    // 文字の配置基点は、行ごとにずらして書く。中の行は高さの -1/2、下の行は 1/2 だけ文字の上の向きへずれる
    let p21 = p21(concat!(
        "#1=DRAUGHTING_PRE_DEFINED_COLOUR('red');\n",
        "#2=DRAUGHTING_PRE_DEFINED_COLOUR('green');\n",
        "#3=EXTERNALLY_DEFINED_TEXT_FONT('MS',$);\n",
        "#5=TEXT_STYLE_FOR_DEFINED_FONT(#1);\n",
        "#6=(TEXT_STYLE(' ',#5) TEXT_STYLE_WITH_BOX_CHARACTERISTICS((BOX_HEIGHT(3.5),BOX_WIDTH(2.8),BOX_SLANT_ANGLE(0.17453292519943295),BOX_ROTATE_ANGLE(1.5707963267948966))) TEXT_STYLE_WITH_SPACING(LENGTH_MEASURE(0.5)));\n",
        "#7=PRESENTATION_STYLE_ASSIGNMENT((#6));\n",
        "#10=CARTESIAN_POINT(' ',(11.75,20.));\n",
        "#11=DIRECTION(' ',(0.,1.));\n",
        "#12=AXIS2_PLACEMENT_2D(' ',#10,#11);\n",
        "#13=PLANAR_EXTENT(' ',30.,3.5);\n",
        "#14=TEXT_LITERAL_WITH_EXTENT('$$SXF_middleline right','\\X2\\8868\\X0\\ a',#12,'baseline right',.DOWN.,#3,#13);\n",
        "#15=(ANNOTATION_OCCURRENCE() ANNOTATION_TEXT_OCCURRENCE() DRAUGHTING_ANNOTATION_OCCURRENCE() GEOMETRIC_REPRESENTATION_ITEM() REPRESENTATION_ITEM(' ') STYLED_ITEM((#7),#14));\n",
        "#16=(TEXT_STYLE(' ',#5) TEXT_STYLE_WITH_BOX_CHARACTERISTICS((BOX_SLANT_ANGLE(0.))) TEXT_STYLE_WITH_SPACING(LENGTH_MEASURE(0.)));\n",
        "#17=PRESENTATION_STYLE_ASSIGNMENT((#16));\n",
        "#20=CARTESIAN_POINT(' ',(0.,0.5));\n",
        "#21=DIRECTION(' ',(1.,0.));\n",
        "#22=AXIS2_PLACEMENT_2D(' ',#20,#21);\n",
        "#23=PLANAR_EXTENT(' ',1.,1.);\n",
        "#24=TEXT_LITERAL_WITH_EXTENT('$$SXF_baseline left','b',#22,'baseline left',.RIGHT.,#3,#23);\n",
        "#25=ANNOTATION_TEXT_OCCURRENCE(' ',(#17),#24);\n",
        "#30=SYMBOL_COLOUR(#2);\n",
        "#31=SYMBOL_STYLE(' ',#30);\n",
        "#32=PRESENTATION_STYLE_ASSIGNMENT((#31));\n",
        "#33=CARTESIAN_POINT(' ',(5.,6.));\n",
        "#34=DIRECTION(' ',(1.,1.));\n",
        "#35=AXIS2_PLACEMENT_2D(' ',#33,#34);\n",
        "#36=SYMBOL_TARGET(' ',#35,1.5,1.5);\n",
        "#37=PRE_DEFINED_POINT_MARKER_SYMBOL('circle');\n",
        "#38=DEFINED_SYMBOL(' ',#37,#36);\n",
        "#39=ANNOTATION_SYMBOL_OCCURRENCE(' ',(#32),#38);\n",
        "#40=AXIS2_PLACEMENT_2D(' ',#33,#21);\n",
        "#41=SYMBOL_TARGET(' ',#40,1.5,1.5);\n",
        "#42=EXTERNALLY_DEFINED_SYMBOL(IDENTIFIER('arrow'),$);\n",
        "#43=DEFINED_SYMBOL(' ',#42,#41);\n",
        "#44=ANNOTATION_SYMBOL_OCCURRENCE(' ',(#32),#43);\n",
        "#45=CARTESIAN_POINT(' ',(1.,2.));\n",
        "#46=AXIS2_PLACEMENT_2D(' ',#45,#21);\n",
        "#47=SYMBOL_TARGET(' ',#46,1.,1.);\n",
        "#48=EXTERNALLY_DEFINED_SYMBOL(IDENTIFIER('tree'),$);\n",
        "#49=DEFINED_SYMBOL(' ',#48,#47);\n",
        "#50=PRESENTATION_STYLE_ASSIGNMENT((NULL_STYLE(.NULL.)));\n",
        "#52=ANNOTATION_SYMBOL_OCCURRENCE(' ',(#50),#49);\n",
        "#60=PRESENTATION_LAYER_ASSIGNMENT('L',' ',(#15,#25,#39,#44,#52));\n",
        "#70=DRAWING_SHEET_REVISION('A3_horizontal',(#15,#25,#39,#44,#52),$,'01');\n",
        "#71=DRAUGHTING_TITLE((),'JAPANESE','S');\n",
    ));
    let (from_sfc, sfc_diag) = lift(&sfc);
    let (from_p21, diag) = lift(&p21);
    assert!(sfc_diag.is_empty(), "{sfc_diag:?}");
    assert!(diag.is_empty(), "{diag:?}");
    assert_eq!(from_p21.sheets[0].elements.len(), 5);
    assert_eq!(unsourced(from_p21), unsourced(from_sfc));
}

#[test]
fn moves_p21_dimensions_and_callouts_like_the_same_sfc() {
    let sfc = sfc(&[
        b"layer_feature(\\'L\\','1')",
        b"text_font_feature(\\'MS\\')",
        b"linear_dim_feature('1','1','1','1','0','10','100','10','1','0','0','0','2','0','12','1','100','0','100','2','100','12','1','1','0','10','1','1','2','100','10','1','1','1',\\'100\\','50','11','3.5','9','0.5','0','0','2','1')",
        b"curve_dim_feature('1','1','1','1','0','0','10','0','90','0','0','0','0','0','0','0','0','0','0','0','0','0','0','0','0','0','0','0','0','0','0','0','0','0')",
        b"radius_dim_feature('1','1','1','1','5','0','0','0','1','1','5','0','1','0')",
        b"label_feature('1','1','1','1','3','(0,10,20)','(0,5,5)','1','2.0','1','1',\\'A\\','21','5','2','4','0','0','0','1','1')",
        b"balloon_feature('1','1','1','1','2','(0,10)','(0,10)','15','15','5','0','1.0','1','1',\\'3\\','15','15','4','2','0','0','0','5','1')",
        b"drawing_sheet_feature(\\'S\\','3','1','0','0')",
    ]);
    // 寸法補助線は名前の順に、矢印の向きは端末記号の X 軸と寸法線の外向きから読む
    let p21 = p21(concat!(
        "#1=DRAUGHTING_PRE_DEFINED_COLOUR('black');\n",
        "#2=DRAUGHTING_PRE_DEFINED_CURVE_FONT('continuous');\n",
        "#3=(LENGTH_UNIT() NAMED_UNIT(*) SI_UNIT(.MILLI.,.METRE.));\n",
        "#4=LENGTH_MEASURE_WITH_UNIT(POSITIVE_LENGTH_MEASURE(0.13),#3);\n",
        "#5=CURVE_STYLE(' ',#2,#4,#1);\n",
        "#6=PRESENTATION_STYLE_ASSIGNMENT((#5));\n",
        "#7=EXTERNALLY_DEFINED_TEXT_FONT('MS',$);\n",
        "#8=TEXT_STYLE_FOR_DEFINED_FONT(#1);\n",
        "#9=SYMBOL_COLOUR(#1);\n",
        "#10=SYMBOL_STYLE(' ',#9);\n",
        "#11=PRESENTATION_STYLE_ASSIGNMENT((#10));\n",
        "#12=DIRECTION(' ',(1.,0.));\n",
        "#13=DIRECTION(' ',(-1.,0.));\n",
        "#20=CARTESIAN_POINT(' ',(0.,10.));\n",
        "#21=CARTESIAN_POINT(' ',(100.,10.));\n",
        "#22=VECTOR(' ',#12,1.);\n",
        "#23=LINE(' ',#20,#22);\n",
        "#24=TRIMMED_CURVE(' ',#23,(#20),(#21),.T.,.CARTESIAN.);\n",
        "#25=(ANNOTATION_CURVE_OCCURRENCE() ANNOTATION_OCCURRENCE() DIMENSION_CURVE() DRAUGHTING_ANNOTATION_OCCURRENCE() GEOMETRIC_REPRESENTATION_ITEM() REPRESENTATION_ITEM(' ') STYLED_ITEM((#6),#24));\n",
        "#26=CARTESIAN_POINT(' ',(0.,0.));\n",
        "#27=CARTESIAN_POINT(' ',(0.,2.));\n",
        "#28=CARTESIAN_POINT(' ',(0.,12.));\n",
        "#29=DIRECTION(' ',(0.,1.));\n",
        "#30=VECTOR(' ',#29,1.);\n",
        "#31=LINE(' ',#26,#30);\n",
        "#32=TRIMMED_CURVE(' ',#31,(#27),(#28),.T.,.CARTESIAN.);\n",
        "#33=(ANNOTATION_CURVE_OCCURRENCE() ANNOTATION_OCCURRENCE() DRAUGHTING_ANNOTATION_OCCURRENCE() GEOMETRIC_REPRESENTATION_ITEM() PROJECTION_CURVE() REPRESENTATION_ITEM('$$SXF_prj_1') STYLED_ITEM((#6),#32));\n",
        "#34=CARTESIAN_POINT(' ',(100.,0.));\n",
        "#35=CARTESIAN_POINT(' ',(100.,2.));\n",
        "#36=CARTESIAN_POINT(' ',(100.,12.));\n",
        "#37=LINE(' ',#34,#30);\n",
        "#38=TRIMMED_CURVE(' ',#37,(#35),(#36),.T.,.CARTESIAN.);\n",
        "#39=(ANNOTATION_CURVE_OCCURRENCE() ANNOTATION_OCCURRENCE() DRAUGHTING_ANNOTATION_OCCURRENCE() GEOMETRIC_REPRESENTATION_ITEM() PROJECTION_CURVE() REPRESENTATION_ITEM('$$SXF_prj_2') STYLED_ITEM((#6),#38));\n",
        "#40=(TEXT_STYLE(' ',#8) TEXT_STYLE_WITH_BOX_CHARACTERISTICS((BOX_SLANT_ANGLE(0.))) TEXT_STYLE_WITH_SPACING(LENGTH_MEASURE(0.5)));\n",
        "#41=PRESENTATION_STYLE_ASSIGNMENT((#40));\n",
        "#42=CARTESIAN_POINT(' ',(50.,12.75));\n",
        "#43=AXIS2_PLACEMENT_2D(' ',#42,#12);\n",
        "#44=PLANAR_EXTENT(' ',9.,3.5);\n",
        "#45=TEXT_LITERAL_WITH_EXTENT('$$SXF_baseline centre','100',#43,'baseline centre',.RIGHT.,#7,#44);\n",
        "#46=ANNOTATION_TEXT_OCCURRENCE(' ',(#41),#45);\n",
        "#47=PRE_DEFINED_TERMINATOR_SYMBOL('blanked arrow');\n",
        "#48=AXIS2_PLACEMENT_2D(' ',#20,#13);\n",
        "#49=SYMBOL_TARGET(' ',#48,1.,1.);\n",
        "#50=DEFINED_SYMBOL(' ',#47,#49);\n",
        "#51=(ANNOTATION_OCCURRENCE() ANNOTATION_SYMBOL_OCCURRENCE() DIMENSION_CURVE_TERMINATOR(.ORIGIN.) DRAUGHTING_ANNOTATION_OCCURRENCE() GEOMETRIC_REPRESENTATION_ITEM() REPRESENTATION_ITEM('$$SXF_arw_1') STYLED_ITEM((#11),#50) TERMINATOR_SYMBOL(#25));\n",
        "#52=AXIS2_PLACEMENT_2D(' ',#21,#13);\n",
        "#53=SYMBOL_TARGET(' ',#52,1.,1.);\n",
        "#54=DEFINED_SYMBOL(' ',#47,#53);\n",
        "#55=(ANNOTATION_OCCURRENCE() ANNOTATION_SYMBOL_OCCURRENCE() DIMENSION_CURVE_TERMINATOR(.TARGET.) DRAUGHTING_ANNOTATION_OCCURRENCE() GEOMETRIC_REPRESENTATION_ITEM() REPRESENTATION_ITEM('$$SXF_arw_2') STYLED_ITEM((#11),#54) TERMINATOR_SYMBOL(#25));\n",
        "#56=(DIMENSION_CURVE_DIRECTED_CALLOUT() DRAUGHTING_CALLOUT((#25,#39,#33,#46,#51,#55)) DRAUGHTING_ELEMENTS() GEOMETRIC_REPRESENTATION_ITEM() LINEAR_DIMENSION() REPRESENTATION_ITEM(' '));\n",
        "#57=(DRAUGHTING_CALLOUT((#46)) DRAUGHTING_ELEMENTS() GEOMETRIC_REPRESENTATION_ITEM() REPRESENTATION_ITEM(' ') STRUCTURED_DIMENSION_CALLOUT());\n",
        "#58=DIMENSION_CALLOUT_RELATIONSHIP('primary',' ',#56,#57);\n",
        "#61=AXIS2_PLACEMENT_2D(' ',#26,#12);\n",
        "#62=CIRCLE(' ',#61,10.);\n",
        "#63=TRIMMED_CURVE(' ',#62,(PARAMETER_VALUE(0.)),(PARAMETER_VALUE(1.5707963267948966)),.T.,.PARAMETER.);\n",
        "#64=(ANNOTATION_CURVE_OCCURRENCE() ANNOTATION_OCCURRENCE() DIMENSION_CURVE() DRAUGHTING_ANNOTATION_OCCURRENCE() GEOMETRIC_REPRESENTATION_ITEM() REPRESENTATION_ITEM(' ') STYLED_ITEM((#6),#63));\n",
        "#65=(CURVE_DIMENSION() DIMENSION_CURVE_DIRECTED_CALLOUT() DRAUGHTING_CALLOUT((#64)) DRAUGHTING_ELEMENTS() GEOMETRIC_REPRESENTATION_ITEM() REPRESENTATION_ITEM(' '));\n",
        "#70=CARTESIAN_POINT(' ',(5.,0.));\n",
        "#71=VECTOR(' ',#13,1.);\n",
        "#72=LINE(' ',#70,#71);\n",
        "#73=TRIMMED_CURVE(' ',#72,(#70),(#26),.T.,.CARTESIAN.);\n",
        "#74=(ANNOTATION_CURVE_OCCURRENCE() ANNOTATION_OCCURRENCE() DIMENSION_CURVE() DRAUGHTING_ANNOTATION_OCCURRENCE() GEOMETRIC_REPRESENTATION_ITEM() REPRESENTATION_ITEM(' ') STYLED_ITEM((#6),#73));\n",
        "#75=AXIS2_PLACEMENT_2D(' ',#70,#12);\n",
        "#76=SYMBOL_TARGET(' ',#75,1.,1.);\n",
        "#77=DEFINED_SYMBOL(' ',#47,#76);\n",
        "#78=(ANNOTATION_OCCURRENCE() ANNOTATION_SYMBOL_OCCURRENCE() DIMENSION_CURVE_TERMINATOR(.ORIGIN.) DRAUGHTING_ANNOTATION_OCCURRENCE() GEOMETRIC_REPRESENTATION_ITEM() REPRESENTATION_ITEM('$$SXF_arw_1') STYLED_ITEM((#11),#77) TERMINATOR_SYMBOL(#74));\n",
        "#79=(DIMENSION_CURVE_DIRECTED_CALLOUT() DRAUGHTING_CALLOUT((#74,#78)) DRAUGHTING_ELEMENTS() GEOMETRIC_REPRESENTATION_ITEM() RADIUS_DIMENSION() REPRESENTATION_ITEM(' '));\n",
        "#80=CARTESIAN_POINT(' ',(10.,5.));\n",
        "#81=CARTESIAN_POINT(' ',(20.,5.));\n",
        "#82=POLYLINE(' ',(#26,#80,#81));\n",
        "#83=(ANNOTATION_CURVE_OCCURRENCE() ANNOTATION_OCCURRENCE() DRAUGHTING_ANNOTATION_OCCURRENCE() GEOMETRIC_REPRESENTATION_ITEM() LEADER_CURVE() REPRESENTATION_ITEM(' ') STYLED_ITEM((#6),#82));\n",
        "#84=(TEXT_STYLE(' ',#8) TEXT_STYLE_WITH_BOX_CHARACTERISTICS((BOX_SLANT_ANGLE(0.))) TEXT_STYLE_WITH_SPACING(LENGTH_MEASURE(0.)));\n",
        "#85=PRESENTATION_STYLE_ASSIGNMENT((#84));\n",
        "#86=CARTESIAN_POINT(' ',(21.,6.));\n",
        "#87=AXIS2_PLACEMENT_2D(' ',#86,#12);\n",
        "#88=PLANAR_EXTENT(' ',4.,2.);\n",
        "#89=TEXT_LITERAL_WITH_EXTENT('$$SXF_baseline left','A',#87,'baseline left',.RIGHT.,#7,#88);\n",
        "#90=ANNOTATION_TEXT_OCCURRENCE(' ',(#85),#89);\n",
        "#91=AXIS2_PLACEMENT_2D(' ',#26,#13);\n",
        "#92=SYMBOL_TARGET(' ',#91,2.,2.);\n",
        "#93=DEFINED_SYMBOL(' ',#47,#92);\n",
        "#94=(ANNOTATION_OCCURRENCE() ANNOTATION_SYMBOL_OCCURRENCE() DRAUGHTING_ANNOTATION_OCCURRENCE() GEOMETRIC_REPRESENTATION_ITEM() LEADER_TERMINATOR() REPRESENTATION_ITEM(' ') STYLED_ITEM((#11),#93) TERMINATOR_SYMBOL(#83));\n",
        "#95=(DRAUGHTING_CALLOUT((#83,#90,#94)) DRAUGHTING_ELEMENTS() GEOMETRIC_REPRESENTATION_ITEM() LEADER_DIRECTED_CALLOUT() REPRESENTATION_ITEM(' '));\n",
        "#100=CARTESIAN_POINT(' ',(10.,10.));\n",
        "#101=POLYLINE(' ',(#26,#100));\n",
        "#102=(ANNOTATION_CURVE_OCCURRENCE() ANNOTATION_OCCURRENCE() DRAUGHTING_ANNOTATION_OCCURRENCE() GEOMETRIC_REPRESENTATION_ITEM() LEADER_CURVE() REPRESENTATION_ITEM(' ') STYLED_ITEM((#6),#101));\n",
        "#103=CARTESIAN_POINT(' ',(15.,15.));\n",
        "#104=AXIS2_PLACEMENT_2D(' ',#103,#12);\n",
        "#105=CIRCLE(' ',#104,5.);\n",
        "#106=ANNOTATION_CURVE_OCCURRENCE(' ',(#6),#105);\n",
        "#107=CARTESIAN_POINT(' ',(15.,13.));\n",
        "#108=AXIS2_PLACEMENT_2D(' ',#107,#12);\n",
        "#109=PLANAR_EXTENT(' ',2.,4.);\n",
        "#110=TEXT_LITERAL_WITH_EXTENT('$$SXF_middleline centre','3',#108,'middleline centre',.RIGHT.,#7,#109);\n",
        "#111=ANNOTATION_TEXT_OCCURRENCE(' ',(#85),#110);\n",
        "#112=(DRAUGHTING_CALLOUT((#102,#106,#111)) DRAUGHTING_ELEMENTS() GEOMETRIC_REPRESENTATION_ITEM() LEADER_DIRECTED_CALLOUT() REPRESENTATION_ITEM(' '));\n",
        "#120=PRESENTATION_LAYER_ASSIGNMENT('L',' ',(#25,#64,#74,#83,#102));\n",
        "#121=DRAWING_SHEET_REVISION('A3_horizontal',(#56,#65,#79,#95,#112),$,'01');\n",
        "#122=DRAUGHTING_TITLE((),'JAPANESE','S');\n",
    ));
    let (from_sfc, sfc_diag) = lift(&sfc);
    let (from_p21, diag) = lift(&p21);
    assert!(sfc_diag.is_empty(), "{sfc_diag:?}");
    assert!(diag.is_empty(), "{diag:?}");
    assert_eq!(from_p21.sheets[0].elements.len(), 5);
    assert_eq!(unsourced(from_p21), unsourced(from_sfc));
}

#[test]
fn moves_p21_hatches_and_composite_curves_like_the_same_sfc() {
    let sfc = sfc(&[
        b"layer_feature(\\'L\\','1')",
        b"line_feature('1','2','1','1','0','0','10','0')",
        b"polyline_feature('1','2','1','1','3','(10,10,0)','(0,10,10)')",
        b"composite_curve_org_feature('2','1','1','1')",
        b"circle_feature('1','1','1','1','5','5','1')",
        b"composite_curve_org_feature('1','1','1','0')",
        b"fill_area_style_colour_feature('1','3','1','1','(2)')",
        b"fill_area_style_hatching_feature('1','1','(4,1,1,0,0,2,90)','1','0','()')",
        b"externally_defined_hatch_feature('1',\\'Area_control\\','1','0','()')",
        b"fill_area_style_tiles_feature('1',\\'P\\','2','0','0','1','0','1','90','2','2','90','1','0','()')",
        b"drawing_sheet_feature(\\'S\\','3','1','0','0')",
    ]);
    // 複合曲線は用紙の要素として並べず、ハッチングの外形と中抜きから引く
    let p21 = p21(concat!(
        "#1=DRAUGHTING_PRE_DEFINED_COLOUR('black');\n",
        "#2=DRAUGHTING_PRE_DEFINED_COLOUR('red');\n",
        "#3=DRAUGHTING_PRE_DEFINED_COLOUR('green');\n",
        "#4=DRAUGHTING_PRE_DEFINED_COLOUR('blue');\n",
        "#5=DRAUGHTING_PRE_DEFINED_CURVE_FONT('continuous');\n",
        "#6=(LENGTH_UNIT() NAMED_UNIT(*) SI_UNIT(.MILLI.,.METRE.));\n",
        "#7=LENGTH_MEASURE_WITH_UNIT(POSITIVE_LENGTH_MEASURE(0.13),#6);\n",
        "#8=CURVE_STYLE(' ',#5,#7,#2);\n",
        "#9=PRESENTATION_STYLE_ASSIGNMENT((#8));\n",
        "#10=CURVE_STYLE(' ',#5,#7,#1);\n",
        "#11=PRESENTATION_STYLE_ASSIGNMENT((#10));\n",
        "#12=DIRECTION(' ',(1.,0.));\n",
        "#13=DIRECTION(' ',(0.,1.));\n",
        "#20=CARTESIAN_POINT(' ',(0.,0.));\n",
        "#21=CARTESIAN_POINT(' ',(10.,0.));\n",
        "#22=VECTOR(' ',#12,1.);\n",
        "#23=LINE(' ',#20,#22);\n",
        "#24=TRIMMED_CURVE(' ',#23,(#20),(#21),.T.,.CARTESIAN.);\n",
        "#25=CARTESIAN_POINT(' ',(10.,10.));\n",
        "#26=CARTESIAN_POINT(' ',(0.,10.));\n",
        "#27=POLYLINE(' ',(#21,#25,#26));\n",
        "#28=COMPOSITE_CURVE_SEGMENT(.CONTINUOUS.,.T.,#24);\n",
        "#29=COMPOSITE_CURVE_SEGMENT(.CONTINUOUS.,.T.,#27);\n",
        "#30=COMPOSITE_CURVE(' ',(#28,#29),.F.);\n",
        "#31=ANNOTATION_CURVE_OCCURRENCE(' ',(#9),#30);\n",
        "#32=CARTESIAN_POINT(' ',(5.,5.));\n",
        "#33=AXIS2_PLACEMENT_2D(' ',#32,#12);\n",
        "#34=CIRCLE(' ',#33,1.);\n",
        "#35=COMPOSITE_CURVE_SEGMENT(.CONTINUOUS.,.T.,#34);\n",
        "#36=COMPOSITE_CURVE(' ',(#35),.F.);\n",
        "#37=ANNOTATION_CURVE_OCCURRENCE(' ',(#11),#36);\n",
        "#38=INVISIBILITY((#37));\n",
        "#40=FILL_AREA_STYLE_COLOUR(' ',#3);\n",
        "#41=FILL_AREA_STYLE(' ',(#40));\n",
        "#42=PRESENTATION_STYLE_ASSIGNMENT((#41));\n",
        "#43=ANNOTATION_FILL_AREA(' ',(#30,#36));\n",
        "#44=ANNOTATION_FILL_AREA_OCCURRENCE(' ',(#42),#43,#20);\n",
        "#50=CURVE_STYLE(' ',#5,#7,#4);\n",
        "#51=DIRECTION(' ',(-1.,0.));\n",
        "#52=VECTOR(' ',#51,2.);\n",
        "#53=ONE_DIRECTION_REPEAT_FACTOR(' ',#52);\n",
        "#54=FILL_AREA_STYLE_HATCHING(' ',#50,#53,#20,#20,1.5707963267948966);\n",
        "#55=FILL_AREA_STYLE(' ',(#54));\n",
        "#56=PRESENTATION_STYLE_ASSIGNMENT((#55));\n",
        "#57=ANNOTATION_FILL_AREA(' ',(#30));\n",
        "#58=ANNOTATION_FILL_AREA_OCCURRENCE(' ',(#56),#57,#20);\n",
        "#60=EXTERNALLY_DEFINED_HATCH_STYLE(IDENTIFIER('Area_control'),$,' ');\n",
        "#61=FILL_AREA_STYLE(' ',(#60));\n",
        "#62=PRESENTATION_STYLE_ASSIGNMENT((#61));\n",
        "#63=ANNOTATION_FILL_AREA_OCCURRENCE(' ',(#62),#57,#20);\n",
        "#70=SYMBOL_COLOUR(#2);\n",
        "#71=SYMBOL_STYLE(' ',#70);\n",
        "#72=PRESENTATION_STYLE_ASSIGNMENT((#71));\n",
        "#73=AXIS2_PLACEMENT_2D(' ',#20,#13);\n",
        "#74=SYMBOL_TARGET(' ',#73,2.,2.);\n",
        "#75=EXTERNALLY_DEFINED_SYMBOL(IDENTIFIER('P'),$);\n",
        "#76=DEFINED_SYMBOL(' ',#75,#74);\n",
        "#77=ANNOTATION_SYMBOL_OCCURRENCE(' ',(#72),#76);\n",
        "#78=FILL_AREA_STYLE_TILE_SYMBOL_WITH_STYLE(' ',#77);\n",
        "#79=VECTOR(' ',#12,1.);\n",
        "#80=VECTOR(' ',#13,1.);\n",
        "#81=TWO_DIRECTION_REPEAT_FACTOR(' ',#79,#80);\n",
        "#82=FILL_AREA_STYLE_TILES(' ',#81,(#78),1.);\n",
        "#83=FILL_AREA_STYLE(' ',(#82));\n",
        "#84=PRESENTATION_STYLE_ASSIGNMENT((#83));\n",
        "#85=ANNOTATION_FILL_AREA_OCCURRENCE(' ',(#84),#57,#20);\n",
        "#90=PRESENTATION_LAYER_ASSIGNMENT('L',' ',(#31,#37,#44,#58,#63,#85));\n",
        "#91=DRAWING_SHEET_REVISION('A3_horizontal',(#44,#58,#63,#85),$,'01');\n",
        "#92=DRAUGHTING_TITLE((),'JAPANESE','S');\n",
    ));
    let (from_sfc, sfc_diag) = lift(&sfc);
    let (from_p21, diag) = lift(&p21);
    assert!(sfc_diag.is_empty(), "{sfc_diag:?}");
    assert!(diag.is_empty(), "{diag:?}");
    assert_eq!(from_p21.sheets[0].elements.len(), 6);
    assert_eq!(unsourced(from_p21), unsourced(from_sfc));
}

#[test]
fn moves_p21_subfigures_and_attributes_like_the_same_sfc() {
    // 複合図形の定義は、中の定義から先に書く
    let sfc = sfc(&[
        b"layer_feature(\\'L\\','1')",
        b"line_feature('1','1','1','1','0','0','5','5')",
        b"sfig_org_feature(\\'B\\','1')",
        b"line_feature('1','1','1','1','0','0','1','0')",
        b"sfig_org_feature(\\'P\\','4')",
        b"sfig_locate_feature('1',\\'B\\','10','0','0','1','1')",
        b"sfig_locate_feature('1',\\'P\\','2','3','90','2','2')",
        b"sfig_org_feature(\\'A\\','1')",
        b"line_feature('1','1','1','1','0','0','0','1')",
        b"sfig_org_feature(\\'$$ATRU$$1$$contour$$height$$12.5\\','3')",
        b"sfig_locate_feature('1',\\'A\\','100','50','0','2','2')",
        b"sfig_locate_feature('1',\\'$$ATRU$$1$$contour$$height$$12.5\\','0','0','0','1','1')",
        b"drawing_sheet_feature(\\'S\\','3','1','0','0')",
    ]);
    let p21 = p21(concat!(
        "#1=DRAUGHTING_PRE_DEFINED_COLOUR('black');\n",
        "#2=DRAUGHTING_PRE_DEFINED_CURVE_FONT('continuous');\n",
        "#3=(LENGTH_UNIT() NAMED_UNIT(*) SI_UNIT(.MILLI.,.METRE.));\n",
        "#4=LENGTH_MEASURE_WITH_UNIT(POSITIVE_LENGTH_MEASURE(0.13),#3);\n",
        "#5=CURVE_STYLE(' ',#2,#4,#1);\n",
        "#6=PRESENTATION_STYLE_ASSIGNMENT((#5));\n",
        "#7=PRESENTATION_STYLE_ASSIGNMENT((NULL_STYLE(.NULL.)));\n",
        "#8=DIRECTION(' ',(1.,0.));\n",
        "#9=DIRECTION(' ',(0.,1.));\n",
        "#10=CARTESIAN_POINT(' ',(0.,0.));\n",
        "#11=AXIS2_PLACEMENT_2D(' ',#10,#8);\n",
        "#12=GEOMETRIC_REPRESENTATION_CONTEXT(' ',' ',2);\n",
        "#20=CARTESIAN_POINT(' ',(5.,5.));\n",
        "#21=POLYLINE(' ',(#10,#20));\n",
        "#22=CARTESIAN_POINT(' ',(1.,0.));\n",
        "#23=VECTOR(' ',#8,1.);\n",
        "#24=LINE(' ',#10,#23);\n",
        "#25=TRIMMED_CURVE(' ',#24,(#10),(#22),.T.,.CARTESIAN.);\n",
        "#26=ANNOTATION_CURVE_OCCURRENCE(' ',(#6),#25);\n",
        "#27=CARTESIAN_POINT(' ',(0.,1.));\n",
        "#28=VECTOR(' ',#9,1.);\n",
        "#29=LINE(' ',#10,#28);\n",
        "#30=TRIMMED_CURVE(' ',#29,(#10),(#27),.T.,.CARTESIAN.);\n",
        "#31=ANNOTATION_CURVE_OCCURRENCE(' ',(#6),#30);\n",
        "#32=VECTOR(' ',#8,7.0710678118654755);\n",
        "#33=DIRECTION(' ',(0.7071067811865475,0.7071067811865475));\n",
        "#34=VECTOR(' ',#33,1.);\n",
        "#35=LINE(' ',#10,#34);\n",
        "#36=TRIMMED_CURVE(' ',#35,(#10),(#20),.T.,.CARTESIAN.);\n",
        "#37=ANNOTATION_CURVE_OCCURRENCE(' ',(#6),#36);\n",
        "#40=DRAUGHTING_SUBFIGURE_REPRESENTATION('$$SXF_FM_B',(#37,#11),#12);\n",
        "#41=SYMBOL_REPRESENTATION_MAP(#11,#40);\n",
        "#42=DRAUGHTING_SUBFIGURE_REPRESENTATION('$$SXF_P_P',(#26,#11),#12);\n",
        "#43=SYMBOL_REPRESENTATION_MAP(#11,#42);\n",
        "#44=CARTESIAN_POINT(' ',(10.,0.));\n",
        "#45=AXIS2_PLACEMENT_2D(' ',#44,#8);\n",
        "#46=SYMBOL_TARGET(' ',#45,1.,1.);\n",
        "#47=(ANNOTATION_SYMBOL() GEOMETRIC_REPRESENTATION_ITEM() MAPPED_ITEM(#41,#46) REPRESENTATION_ITEM(' '));\n",
        "#48=(ANNOTATION_OCCURRENCE() ANNOTATION_SUBFIGURE_OCCURRENCE() ANNOTATION_SYMBOL_OCCURRENCE() DRAUGHTING_ANNOTATION_OCCURRENCE() GEOMETRIC_REPRESENTATION_ITEM() REPRESENTATION_ITEM('$$SXF_FM_B') STYLED_ITEM((#7),#47));\n",
        "#49=CARTESIAN_POINT(' ',(2.,3.));\n",
        "#50=AXIS2_PLACEMENT_2D(' ',#49,#9);\n",
        "#51=SYMBOL_TARGET(' ',#50,2.,2.);\n",
        "#52=(ANNOTATION_SYMBOL() GEOMETRIC_REPRESENTATION_ITEM() MAPPED_ITEM(#43,#51) REPRESENTATION_ITEM(' '));\n",
        "#53=(ANNOTATION_OCCURRENCE() ANNOTATION_SUBFIGURE_OCCURRENCE() ANNOTATION_SYMBOL_OCCURRENCE() DRAUGHTING_ANNOTATION_OCCURRENCE() GEOMETRIC_REPRESENTATION_ITEM() REPRESENTATION_ITEM('$$SXF_P_P') STYLED_ITEM((#7),#52));\n",
        "#54=DRAUGHTING_SUBFIGURE_REPRESENTATION('$$SXF_FM_A',(#48,#53,#11),#12);\n",
        "#55=SYMBOL_REPRESENTATION_MAP(#11,#54);\n",
        "#56=CARTESIAN_POINT(' ',(100.,50.));\n",
        "#57=AXIS2_PLACEMENT_2D(' ',#56,#8);\n",
        "#58=SYMBOL_TARGET(' ',#57,2.,2.);\n",
        "#59=(ANNOTATION_SYMBOL() GEOMETRIC_REPRESENTATION_ITEM() MAPPED_ITEM(#55,#58) REPRESENTATION_ITEM(' '));\n",
        "#60=(ANNOTATION_OCCURRENCE() ANNOTATION_SUBFIGURE_OCCURRENCE() ANNOTATION_SYMBOL_OCCURRENCE() DRAUGHTING_ANNOTATION_OCCURRENCE() GEOMETRIC_REPRESENTATION_ITEM() REPRESENTATION_ITEM('$$SXF_FM_A') STYLED_ITEM((#7),#59));\n",
        "#61=DRAUGHTING_SUBFIGURE_REPRESENTATION('$$SXF_G_$$ATRU$$1$$contour$$height$$12.5',(#31,#11),#12);\n",
        "#62=SYMBOL_REPRESENTATION_MAP(#11,#61);\n",
        "#63=SYMBOL_TARGET(' ',#11,1.,1.);\n",
        "#64=(ANNOTATION_SYMBOL() GEOMETRIC_REPRESENTATION_ITEM() MAPPED_ITEM(#62,#63) REPRESENTATION_ITEM(' '));\n",
        "#65=(ANNOTATION_OCCURRENCE() ANNOTATION_SUBFIGURE_OCCURRENCE() ANNOTATION_SYMBOL_OCCURRENCE() DRAUGHTING_ANNOTATION_OCCURRENCE() GEOMETRIC_REPRESENTATION_ITEM() REPRESENTATION_ITEM(' ') STYLED_ITEM((#7),#64));\n",
        "#70=PRESENTATION_LAYER_ASSIGNMENT('L',' ',(#26,#31,#37,#48,#53,#60,#65));\n",
        "#71=DRAWING_SHEET_REVISION('A3_horizontal',(#60,#65),$,'01');\n",
        "#72=DRAUGHTING_TITLE((),'JAPANESE','S');\n",
    ));
    let (from_sfc, sfc_diag) = lift(&sfc);
    let (from_p21, diag) = lift(&p21);
    assert!(sfc_diag.is_empty(), "{sfc_diag:?}");
    assert!(diag.is_empty(), "{diag:?}");
    assert_eq!(unsourced(from_p21), unsourced(from_sfc));
}

/// 番号つきのフィーチャを並べた SFC
#[cfg(test)]
fn numbered(features: &[(u64, &str)]) -> Vec<u8> {
    let mut src = String::from(
        "ISO-10303-21;\nHEADER;\nFILE_DESCRIPTION(('SCADEC level2 feature_mode'),'2;1');\nFILE_SCHEMA(('ASSOCIATIVE_DRAUGHTING'));\nENDSEC;\nDATA;\n",
    );
    for (number, feature) in features {
        src.push_str(&format!("/*SXF3\n#{number} = {feature}\nSXF3*/\n"));
    }
    src.push_str("ENDSEC;\nEND-ISO-10303-21;\n");
    src.into_bytes()
}

#[test]
fn element_ids_keep_instance_numbers_when_a_feature_is_added_before() {
    use zumen_drawing::{Hash, id};
    let ids = |src: &[u8]| {
        let (drawing, _) = lift(src);
        let sheet = &drawing.sheets[0];
        let place = id::sheet(sheet);
        let mut namer = id::Namer::new(&place);
        sheet
            .elements
            .iter()
            .enumerate()
            .map(|(index, element)| (namer.next(index, element), Hash::element(element)))
            .collect::<Vec<_>>()
    };
    let layer = r"layer_feature(\'L\','1')";
    let line = "line_feature('1','1','1','1','0.0','0.0','10.0','0.0')";
    let circle = "circle_feature('1','1','1','1','5.0','5.0','3.0')";
    let sheet = r"drawing_sheet_feature(\'S\','3','1','0','0')";
    let before = ids(&numbered(&[
        (10, layer),
        (20, line),
        (30, circle),
        (90, sheet),
    ]));
    let after = ids(&numbered(&[
        (10, layer),
        (15, "line_feature('1','1','1','1','0.0','0.0','0.0','10.0')"),
        (20, line),
        (30, circle),
        (90, sheet),
    ]));
    // 前に線を足しても、ほかの要素の ID とハッシュは変わらない（決定 0156）
    assert_eq!(
        before.iter().map(|(id, _)| id.as_str()).collect::<Vec<_>>(),
        ["sheet/0/i20", "sheet/0/i30"]
    );
    assert_eq!(after[0].0, "sheet/0/i15");
    assert_eq!(after[1..], before);
}

#[test]
fn repeated_sfc_instance_numbers_do_not_produce_duplicate_ids() {
    use zumen_drawing::id;
    let src = numbered(&[
        (10, r"layer_feature(\'L\','1')"),
        (20, "line_feature('1','1','1','1','0.0','0.0','10.0','0.0')"),
        (20, "circle_feature('1','1','1','1','5.0','5.0','3.0')"),
        (90, r"drawing_sheet_feature(\'S\','3','1','0','0')"),
    ]);
    let (drawing, _) = lift(&src);
    let sheet = &drawing.sheets[0];
    let place = id::sheet(sheet);
    let mut namer = id::Namer::new(&place);
    let ids: Vec<_> = sheet
        .elements
        .iter()
        .enumerate()
        .map(|(index, element)| namer.next(index, element))
        .collect();
    assert_eq!(ids, ["sheet/0/i20", "sheet/0/i20/1"]);
}
