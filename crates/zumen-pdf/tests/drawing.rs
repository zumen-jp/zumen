//! PDF から図面モデル（L2）とシーンへの移し（決定 0256）。入力は、どれも試験の中で作る。

use zumen_base::{Budget, CancelToken, Cx, Diagnostics, codes};
use zumen_drawing::{
    Boundary, Color, Drawing2D, Edge, Element, FieldValue, Fill, HeightMeaning, Horizontal,
    Inherit, Islands, LayerId, Length, LineTypeId, LineWeight, Origin, PaperLength, PaperPoint,
    Placement, Shape, SheetId, Transparency, UnderlayFrame, UnderlayKind, Vertical,
};
use zumen_pdf::Pdf;
use zumen_scene::{Chunking, Options, Palette};
use zumen_sdk::{Error, Lift, testkit};

mod common;

use common::{Document, Page, codes as codes_of, lift, mm, near, one_page};

const HELVETICA: &str = "/Font << /F1 << /Type /Font /Subtype /Type1 /BaseFont /Helvetica >> >>";

#[cfg(test)]
fn elements(drawing: &Drawing2D) -> &[Element<PaperPoint>] {
    &drawing.sheets[0].elements
}

fn point(point: &PaperPoint) -> (f64, f64) {
    (point.x, point.y)
}

fn assert_point(found: &PaperPoint, x: f64, y: f64) {
    assert!(
        near(found.x, x) && near(found.y, y),
        "{:?} と ({x}, {y}) が違う",
        point(found)
    );
}

/// 用紙の大きさ（mm）が、pt の幅と高さと等しい
#[cfg(test)]
fn assert_size(size: Option<(f64, f64)>, width: f64, height: f64) {
    let (w, h) = size.unwrap();
    assert!(near(w, mm(width)) && near(h, mm(height)), "{size:?}");
}

fn rgb(element: &Element<PaperPoint>) -> Option<[u8; 3]> {
    match element.style.color {
        Inherit::Own(Color { rgb, .. }) => rgb,
        _ => None,
    }
}

#[cfg(test)]
fn width(element: &Element<PaperPoint>) -> f64 {
    match element.style.line_weight {
        Some(Inherit::Own(LineWeight::Width(PaperLength(width)))) => width,
        other => panic!("線幅がない: {other:?}"),
    }
}

#[test]
fn moves_pages_to_sheets() {
    let src = Document::new(vec![
        Page::new(""),
        Page::new("").extra("/MediaBox [0 0 595 842]"),
    ])
    .bytes();
    let (drawing, diag) = lift(&src);
    assert!(diag.is_empty(), "{diag:?}");
    assert_eq!(drawing.sheets.len(), 2);
    let sizes: Vec<_> = drawing
        .sheets
        .iter()
        .map(|sheet| sheet.size.map(|[w, h]| (w.0, h.0)))
        .collect();
    assert_size(sizes[0], 612.0, 792.0);
    assert_size(sizes[1], 595.0, 842.0);
    assert_eq!(drawing.sheets[1].id, SheetId(1));
    // ページラベルがなければ、用紙の名前はない
    assert_eq!(drawing.sheets[0].name, None);
    // ページの出所は、ページのオブジェクトのレコード
    assert!(drawing.sheets[0].source.record().is_some());
    // 文書の情報と版は、図面の拡張に持つ
    let document = drawing
        .extensions
        .iter()
        .find(|extension| extension.namespace == "pdf.document")
        .unwrap();
    assert_eq!(document.version.as_deref(), Some("1.7"));
    assert!(
        document
            .fields
            .iter()
            .any(|field| field.key == "pages" && field.value == FieldValue::Integer(2))
    );
}

#[test]
fn moves_lines_rectangles_and_curves() {
    let src = one_page(Page::new(
        "2 w 0 0 m 72 0 l S \
         10 20 30 40 re S \
         0 0 m 0 72 l 72 72 l S \
         0 0 m 10 20 30 20 40 0 c S \
         0 0 m 72 0 l h S",
    ));
    let (drawing, diag) = lift(&src);
    assert!(diag.is_empty(), "{diag:?}");
    let elements = elements(&drawing);
    assert_eq!(elements.len(), 5);
    // 線分。線幅はユーザー空間の 2 を mm にしたもの
    let Shape::Line { start, end } = &elements[0].shape else {
        panic!("線分でない: {:?}", elements[0].shape);
    };
    assert_point(start, 0.0, 0.0);
    assert_point(end, 25.4, 0.0);
    assert!(near(width(&elements[0]), mm(2.0)));
    assert_eq!(rgb(&elements[0]), Some([0, 0, 0]));
    assert_eq!(elements[0].layer, LayerId(0));
    // 長方形は、閉じた 4 頂点の折れ線
    let Shape::Polyline(rectangle) = &elements[1].shape else {
        panic!("折れ線でない");
    };
    assert!(rectangle.closed);
    let corners: Vec<_> = rectangle.vertices.iter().map(|v| point(&v.point)).collect();
    assert_eq!(corners.len(), 4);
    assert!(near(corners[2].0, mm(40.0)) && near(corners[2].1, mm(60.0)));
    // 開いた折れ線
    let Shape::Polyline(open) = &elements[2].shape else {
        panic!("折れ線でない");
    };
    assert!(!open.closed);
    assert_eq!(open.vertices.len(), 3);
    // 3 次のベジェ曲線は、制御点をそのまま持つスプライン
    let Shape::Curve(Edge::Spline(spline)) = &elements[3].shape else {
        panic!("曲線でない");
    };
    assert_eq!(spline.degree, 3);
    assert_eq!(spline.knots, [0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0]);
    assert_point(&spline.control[1], mm(10.0), mm(20.0));
    assert_point(&spline.control[3], mm(40.0), 0.0);
    // 閉じた 2 点の経路は、閉じた折れ線
    let Shape::Polyline(closed) = &elements[4].shape else {
        panic!("折れ線でない");
    };
    assert!(closed.closed);
}

#[test]
fn moves_fills_to_hatches() {
    let src = one_page(Page::new(
        "0 0 1 rg 0 0 m 100 0 l 100 100 l 0 100 l h 25 25 m 75 25 l 75 75 l 25 75 l h f \
         1 0 0 rg 0 0 m 100 0 l 100 100 l 0 100 l h 25 25 m 75 25 l 75 75 l 25 75 l h f* \
         0 0 m 50 100 100 0 v f \
         0 1 0 rg 0 0 50 50 re B",
    ));
    let (drawing, diag) = lift(&src);
    let elements = elements(&drawing);
    assert_eq!(elements.len(), 5, "{:?}", elements);
    let Shape::Hatch(hatch) = &elements[0].shape else {
        panic!("ハッチングでない");
    };
    assert_eq!(hatch.loops.len(), 2);
    assert_eq!(hatch.islands, Islands::Nested);
    assert_eq!(hatch.fill, Fill::Solid);
    let Boundary::Polyline { vertices, closed } = &hatch.loops[0].boundary else {
        panic!("折れ線の輪でない");
    };
    assert!(closed);
    assert_eq!(vertices.len(), 4);
    assert_eq!(rgb(&elements[0]), Some([0, 0, 255]));
    assert_eq!(elements[0].style.line_weight, None);
    // 曲線の輪は辺の並び。閉じる線分を足す
    let Shape::Hatch(curved) = &elements[2].shape else {
        panic!("ハッチングでない");
    };
    let Boundary::Edges(edges) = &curved.loops[0].boundary else {
        panic!("辺の輪でない");
    };
    assert!(matches!(edges[..], [Edge::Spline(_), Edge::Line { .. }]));
    // 塗りと線（B）は、ハッチングと折れ線の 2 つ
    assert!(matches!(elements[3].shape, Shape::Hatch(_)));
    assert!(matches!(elements[4].shape, Shape::Polyline(_)));
    assert_eq!(rgb(&elements[3]), Some([0, 255, 0]));
    // 非ゼロ回転数の規則で塗った 2 つの輪だけを診断する
    assert_eq!(
        codes_of(&diag),
        [(codes::PDF_FILL_RULE_APPROXIMATED.name(), 1)]
    );
}

#[test]
fn moves_dashes_to_line_types() {
    let src = one_page(Page::new(
        "[6 3] 0 d 0 0 m 72 0 l S [2 1 3] 2 d 0 10 m 72 10 l S [6 3] 0 d 0 20 m 72 20 l S \
         [0 4] 0 d 0 30 m 72 30 l S [] 0 d 0 40 m 72 40 l S",
    ));
    let (drawing, diag) = lift(&src);
    let elements = elements(&drawing);
    let line_types: Vec<LineTypeId> = elements
        .iter()
        .map(|element| match element.style.line_type {
            Inherit::Own(id) => id,
            other => panic!("線種がない: {other:?}"),
        })
        .collect();
    // 同じ破線は同じ線種
    assert_eq!(line_types[0], line_types[2]);
    let pattern = |id: LineTypeId| {
        drawing.line_types[id.0 as usize]
            .definition
            .as_ref()
            .unwrap()
            .pattern
            .clone()
    };
    let close =
        |a: &[f64], b: &[f64]| a.len() == b.len() && a.iter().zip(b).all(|(a, b)| near(*a, *b));
    assert!(close(&pattern(line_types[0]), &[mm(6.0), -mm(3.0)]));
    // 数が奇数の列は 2 回並べ、線と間を交互にする
    assert!(close(
        &pattern(line_types[1]),
        &[mm(2.0), -mm(1.0), mm(3.0), -mm(2.0), mm(1.0), -mm(3.0)]
    ));
    // 0 の長さは点
    assert!(close(&pattern(line_types[3]), &[0.0, -mm(4.0)]));
    assert!(pattern(line_types[4]).is_empty());
    assert_eq!(
        drawing.line_types[line_types[0].0 as usize].name,
        "[2.117 1.058]"
    );
    assert_eq!(drawing.line_types[line_types[4].0 as usize].name, "[]");
    // 位相のある破線だけを診断する
    assert_eq!(codes_of(&diag), [(codes::PDF_DASH_APPROXIMATED.name(), 1)]);
}

#[test]
fn maps_colours_by_the_space_family() {
    let resources = "/ColorSpace << \
        /Cal [/CalRGB << /WhitePoint [0.9505 1 1.089] >>] \
        /Lab [/Lab << /WhitePoint [0.9505 1 1.089] >>] \
        /Spot [/Separation /Spot /DeviceCMYK << /FunctionType 2 /Domain [0 1] /C0 [0 0 0 0] /C1 [1 0 0 0] /N 1 >>] \
        /Ix [/Indexed /DeviceRGB 1 <FF000000FF00>] >>";
    let src = one_page(
        Page::new(
            "0.5 g 0 0 10 10 re f \
             0 0 0 1 k 0 0 10 10 re f \
             0.5 0.25 0 0.25 k 0 0 10 10 re f \
             /Cal cs 1 0.5 0 sc 0 0 10 10 re f \
             /Lab cs 50 0 0 sc 0 0 10 10 re f \
             /Spot cs 1 sc 0 0 10 10 re f \
             /Ix cs 1 sc 0 0 10 10 re f",
        )
        .resources(resources),
    );
    let (drawing, diag) = lift(&src);
    let colours: Vec<_> = elements(&drawing).iter().map(rgb).collect();
    assert_eq!(
        colours,
        [
            Some([128, 128, 128]),
            Some([0, 0, 0]),
            Some([64, 128, 191]),
            Some([255, 128, 0]),
            None,
            None,
            None,
        ]
    );
    let mut found: Vec<_> = diag
        .iter()
        .map(|d| {
            let space = match d.param("space") {
                Some(zumen_base::Value::Text(space)) => space.clone(),
                _ => String::new(),
            };
            (d.code().name(), space, d.count())
        })
        .collect();
    found.sort();
    assert_eq!(
        found,
        [
            ("pdf.color.approximated", "CalRGB".to_owned(), 1),
            ("pdf.color.approximated", "DeviceCMYK".to_owned(), 2),
            ("pdf.color.dropped", "Indexed".to_owned(), 1),
            ("pdf.color.dropped", "Lab".to_owned(), 1),
            ("pdf.color.dropped", "Separation".to_owned(), 1),
        ]
    );
}

#[test]
fn moves_opacity_to_transparency() {
    let src = one_page(
        Page::new("/Half gs 0 0 10 10 re f /Clear gs 0 0 m 10 0 l S")
            .resources("/ExtGState << /Half << /ca 0.5 >> /Clear << /CA 0 >> >>"),
    );
    let (drawing, diag) = lift(&src);
    assert!(diag.is_empty(), "{diag:?}");
    let elements = elements(&drawing);
    assert_eq!(
        elements[0].style.transparency,
        Inherit::Own(Transparency { alpha: 128 })
    );
    assert_eq!(
        elements[1].style.transparency,
        Inherit::Own(Transparency { alpha: 0 })
    );
}

#[test]
fn moves_text_runs() {
    let src = one_page(
        Page::new(
            "BT /F1 12 Tf 72 720 Td (Plan A) Tj ET \
             BT /F1 10 Tf 0 1 -1 0 300 300 Tm (Up) Tj ET \
             BT /F1 10 Tf 50 Tz 100 100 Td (Half) Tj ET \
             BT /F1 10 Tf 2 Tr 100 200 Td (Both) Tj 3 Tr (Hidden) Tj ET \
             BT /F1 10 Tf 100 300 Td (   ) Tj ET",
        )
        .resources(HELVETICA),
    );
    let (drawing, diag) = lift(&src);
    assert!(diag.is_empty(), "{diag:?}");
    let texts: Vec<_> = elements(&drawing)
        .iter()
        .map(|element| match &element.shape {
            Shape::Text(text) => (text, element),
            other => panic!("文字でない: {other:?}"),
        })
        .collect();
    // 空白だけの並びは移さない。塗りと線で描く文字は 1 つにする
    let contents: Vec<_> = texts
        .iter()
        .map(|(text, _)| text.content.as_str())
        .collect();
    assert_eq!(contents, ["Plan A", "Up", "Half", "Both", "Hidden"]);
    let (plan, element) = texts[0];
    assert!(near(
        match plan.height.value {
            Length::Paper(PaperLength(height)) => height,
            Length::Model(_) => panic!("用紙の長さでない"),
        },
        mm(12.0)
    ));
    assert_eq!(plan.height.meaning, HeightMeaning::Em);
    let Placement::Anchor {
        point,
        horizontal,
        vertical,
    } = &plan.placement
    else {
        panic!("基点で置いていない");
    };
    assert_point(point, 25.4, 254.0);
    assert_eq!(
        (*horizontal, *vertical),
        (Horizontal::Left, Vertical::Baseline)
    );
    assert!(near(plan.rotation, 0.0) && near(plan.oblique, 0.0));
    assert!(
        (plan.width_factor - 1.0).abs() < 1e-6,
        "{}",
        plan.width_factor
    );
    assert!(!plan.backward);
    assert_eq!(rgb(element), Some([0, 0, 0]));
    // 文字の行列の回転
    assert!(near(texts[1].0.rotation, 90.0));
    // 水平の伸縮（Tz）は幅の倍率
    assert!((texts[2].0.width_factor - 0.5).abs() < 1e-6);
    // 見えない文字は透明にする
    assert_eq!(
        texts[4].1.style.transparency,
        Inherit::Own(Transparency { alpha: 0 })
    );
}

/// 部分集合の名前の前置きを持つ、埋め込みのない Type 0 のフォントと、`<0001>`・`<0002>` を「図」「面」に戻す
/// ToUnicode
fn cid_document(base_font: &str, encoding: &str) -> Vec<u8> {
    let cmap = "/CIDInit /ProcSet findresource begin 12 dict begin begincmap \
        /CIDSystemInfo << /Registry (Adobe) /Ordering (UCS) /Supplement 0 >> def \
        /CMapName /Adobe-Identity-UCS def /CMapType 2 def \
        1 begincodespacerange <0000> <FFFF> endcodespacerange \
        2 beginbfchar <0001> <56F3> <0002> <9762> endbfchar \
        endcmap CMapName currentdict /CMap defineresource pop end end";
    let font = format!(
        "<< /Type /Font /Subtype /Type0 /BaseFont /{base_font} /Encoding /{encoding} \
         /DescendantFonts [<< /Type /Font /Subtype /CIDFontType2 /BaseFont /{base_font} \
         /CIDSystemInfo << /Registry (Adobe) /Ordering (Japan1) /Supplement 6 >> \
         /FontDescriptor << /Type /FontDescriptor /FontName /{base_font} /Flags 4 \
         /FontBBox [0 -141 1000 859] /ItalicAngle 0 /Ascent 859 /Descent -141 /CapHeight 700 \
         /StemV 80 >> /DW 1000 >>] /ToUnicode 4 0 R >>"
    );
    let mut document = Document::new(vec![
        Page::new("BT /F2 10 Tf 100 100 Td <00010002> Tj ET").resources("/Font << /F2 3 0 R >>"),
    ]);
    document.objects = vec![font.into_bytes(), common::stream("", cmap)];
    document.bytes()
}

#[test]
fn moves_font_names_to_text_styles() {
    let (drawing, diag) = lift(&cid_document("ABCDEF+MS-Gothic", "Identity-H"));
    assert!(diag.is_empty(), "{diag:?}");
    let Shape::Text(text) = &elements(&drawing)[0].shape else {
        panic!("文字でない");
    };
    assert_eq!(text.content, "図面");
    assert_eq!(text.style, "MS-Gothic");
    let style = &drawing.text_styles[0];
    assert_eq!(style.name, "MS-Gothic");
    let definition = style.definition.as_ref().unwrap();
    assert_eq!(definition.font.as_ref().unwrap().name, "MS-Gothic");
    assert!(!definition.vertical);

    // 縦書きは、名前の前に `@` を付けた文字スタイル
    let (drawing, _) = lift(&cid_document("MS-Gothic", "Identity-V"));
    let Shape::Text(text) = &elements(&drawing)[0].shape else {
        panic!("文字でない");
    };
    assert_eq!(text.style, "@MS-Gothic");
    assert!(drawing.text_styles[0].definition.as_ref().unwrap().vertical);
    assert!(matches!(
        text.placement,
        Placement::Anchor {
            horizontal: Horizontal::Center,
            vertical: Vertical::Top,
            ..
        }
    ));

    // UTF-8 でない名前（ＭＳ ゴシックの Shift_JIS）は移さない
    let (drawing, diag) = lift(&cid_document("#82l#82r#83S#83V#83b#83N", "Identity-H"));
    let Shape::Text(text) = &elements(&drawing)[0].shape else {
        panic!("文字でない");
    };
    assert_eq!(text.style, "");
    assert!(drawing.text_styles.is_empty());
    assert_eq!(codes_of(&diag), [(codes::PDF_FONT_NAME_DROPPED.name(), 1)]);
}

#[test]
fn moves_optional_content_groups_to_layers() {
    let mut document = Document::new(vec![
        Page::new(
            "/OC /oc1 BDC 0 0 m 10 10 l S EMC /OC /oc2 BDC 0 0 m 20 20 l S EMC \
             /OC /md BDC 0 0 m 40 40 l S EMC 0 0 m 30 30 l S",
        )
        .resources("/Properties << /oc1 3 0 R /oc2 4 0 R /md 5 0 R >>"),
    ]);
    document.objects = vec![
        b"<< /Type /OCG /Name (Walls) >>".to_vec(),
        b"<< /Type /OCG /Name (Hidden) >>".to_vec(),
        b"<< /Type /OCMD /OCGs [3 0 R 4 0 R] /P /AnyOn >>".to_vec(),
    ];
    document.catalog =
        "/OCProperties << /OCGs [3 0 R 4 0 R] /D << /OFF [4 0 R] /Locked [3 0 R] >> >>";
    let (drawing, diag) = lift(&document.bytes());
    let layers: Vec<_> = drawing
        .layers
        .iter()
        .map(|layer| {
            (
                layer.id,
                layer.name.as_str(),
                layer.state.map(|state| (state.visible, state.editable)),
            )
        })
        .collect();
    assert_eq!(
        layers,
        [
            (LayerId(0), "0", None),
            (LayerId(1), "Walls", Some((true, false))),
            (LayerId(2), "Hidden", Some((false, true))),
        ]
    );
    // 隠れた表示グループの内容は、hayro-interpret が塗らない
    let on: Vec<_> = elements(&drawing).iter().map(|e| e.layer).collect();
    assert_eq!(on, [LayerId(1), LayerId(1), LayerId(0)]);
    assert_eq!(
        codes_of(&diag),
        [
            (codes::PDF_OPTIONAL_CONTENT_DROPPED.name(), 1),
            (codes::PDF_OPTIONAL_CONTENT_APPROXIMATED.name(), 1),
        ]
    );
}

#[test]
fn turns_and_crops_pages() {
    // 90 度回したページは、用紙の幅と高さを入れ替え、左上へ回す
    let src = one_page(Page::new("0 0 m 10 0 l S").extra("/MediaBox [0 0 200 100] /Rotate 90"));
    let (drawing, _) = lift(&src);
    let sheet = &drawing.sheets[0];
    assert_size(sheet.size.map(|[w, h]| (w.0, h.0)), 100.0, 200.0);
    let Shape::Line { start, end } = &sheet.elements[0].shape else {
        panic!("線分でない");
    };
    assert_point(start, 0.0, mm(200.0));
    assert_point(end, 0.0, mm(190.0));

    // クロップボックスの左下が原点。UserUnit は単位の大きさ
    let src = one_page(
        Page::new("10 10 m 20 10 l S")
            .extra("/MediaBox [0 0 200 100] /CropBox [10 10 110 60] /UserUnit 2"),
    );
    let (drawing, _) = lift(&src);
    let sheet = &drawing.sheets[0];
    assert_size(sheet.size.map(|[w, h]| (w.0, h.0)), 200.0, 100.0);
    let Shape::Line { start, end } = &sheet.elements[0].shape else {
        panic!("線分でない");
    };
    assert_point(start, 0.0, 0.0);
    assert_point(end, mm(20.0), 0.0);
}

#[test]
fn names_sheets_by_page_labels() {
    let mut document = Document::new(vec![Page::new(""), Page::new(""), Page::new("")]);
    document.catalog = "/PageLabels << /Nums [0 << /S /r >> 2 << /S /D /P (A-) /St 5 >>] >>";
    let (drawing, _) = lift(&document.bytes());
    let names: Vec<_> = drawing
        .sheets
        .iter()
        .map(|sheet| sheet.name.clone())
        .collect();
    assert_eq!(
        names,
        [
            Some("i".to_owned()),
            Some("ii".to_owned()),
            Some("A-5".to_owned())
        ]
    );
}

#[test]
fn moves_images_to_underlays() {
    let mut document = Document::new(vec![
        Page::new("q 72 0 0 36 100 200 cm /Im1 Do Q").resources("/XObject << /Im1 3 0 R >>"),
    ]);
    document.objects = vec![common::stream(
        "/Type /XObject /Subtype /Image /Width 2 /Height 2 /ColorSpace /DeviceRGB \
         /BitsPerComponent 8 /Filter /ASCIIHexDecode",
        "FF000000FF000000FFFFFFFF>",
    )];
    let (drawing, diag) = lift(&document.bytes());
    assert!(diag.is_empty(), "{diag:?}");
    let Shape::Underlay(underlay) = &elements(&drawing)[0].shape else {
        panic!("下敷きでない");
    };
    assert_eq!(underlay.kind, UnderlayKind::Image);
    assert_eq!(underlay.resource.path, "3 0 R");
    assert!(underlay.resource.definition.record().is_some());
    let UnderlayFrame::Pixels {
        size,
        origin,
        right,
        top,
    } = &underlay.frame
    else {
        panic!("画素の格子でない");
    };
    assert_eq!(*size, [2.0, 2.0]);
    assert_point(origin, mm(100.0), mm(200.0));
    assert_point(right, mm(172.0), mm(200.0));
    assert_point(top, mm(100.0), mm(236.0));
}

#[test]
fn reports_type3_glyphs_and_image_masks() {
    // Type 3 のフォントの字形は書体のない文字に、色を塗る画像（ImageMask）はレイヤの色の下敷きにする
    let mut document = Document::new(vec![
        Page::new("BT /T3 12 Tf 100 100 Td (a) Tj ET q 0 0 1 rg 72 0 0 72 200 200 cm /M Do Q")
            .resources("/Font << /T3 3 0 R >> /XObject << /M 5 0 R >>"),
    ]);
    document.objects = vec![
        b"<< /Type /Font /Subtype /Type3 /FontBBox [0 0 1000 1000] /FontMatrix [0.001 0 0 0.001 0 0] \
          /CharProcs << /a 4 0 R >> /Encoding << /Type /Encoding /Differences [97 /a] >> \
          /FirstChar 97 /LastChar 97 /Widths [1000] >>"
            .to_vec(),
        common::stream("", "1000 0 0 0 1000 1000 d1 0 0 1000 1000 re f"),
        common::stream(
            "/Type /XObject /Subtype /Image /Width 1 /Height 1 /ImageMask true \
             /BitsPerComponent 1 /Filter /ASCIIHexDecode",
            "00>",
        ),
    ];
    let (drawing, diag) = lift(&document.bytes());
    let mut found = codes_of(&diag);
    found.sort();
    assert_eq!(
        found,
        [
            (codes::PDF_IMAGE_MASK_APPROXIMATED.name(), 1),
            (codes::PDF_TYPE3_APPROXIMATED.name(), 1),
        ]
    );
    let shapes: Vec<_> = elements(&drawing).iter().map(|e| &e.shape).collect();
    let [Shape::Text(text), Shape::Underlay(underlay)] = shapes.as_slice() else {
        panic!("文字と下敷きでない: {shapes:?}");
    };
    assert_eq!(text.content, "a");
    assert_eq!(text.style, "");
    assert_eq!(underlay.resource.path, "5 0 R");
    assert_eq!(elements(&drawing)[1].style.color, Inherit::Layer);
}

#[test]
fn reports_what_is_not_moved() {
    let shading = "/Shading << /Sh1 << /ShadingType 2 /ColorSpace /DeviceRGB /Coords [0 0 100 0] \
        /Function << /FunctionType 2 /Domain [0 1] /C0 [1 0 0] /C1 [0 0 1] /N 1 >> >> >>";
    let src = one_page(
        Page::new(
            "q 0 0 100 100 re W n 200 200 m 300 300 l S 50 50 m 150 150 l S Q \
             q 0 0 50 50 re W n /Sh1 sh Q",
        )
        .resources(shading)
        .extra(
            "/Annots [<< /Type /Annot /Subtype /Square /Rect [0 0 10 10] >> \
             << /Type /Annot /Subtype /Square /Rect [0 0 20 20] >>] \
             /VP [<< /Type /Viewport /BBox [0 0 100 100] /Measure << /Type /Measure /Subtype /RL \
             /R (1 in = 1 ft) >> >>]",
        ),
    );
    let (drawing, diag) = lift(&src);
    // 切り抜きの外の線は移さず、かかる線は切らずに移す
    assert_eq!(elements(&drawing).len(), 1);
    let mut found = codes_of(&diag);
    found.sort();
    assert_eq!(
        found,
        [
            (codes::PDF_ANNOTATION_UNSUPPORTED.name(), 2),
            (codes::PDF_CLIP_APPROXIMATED.name(), 1),
            (codes::PDF_CLIP_DROPPED.name(), 1),
            (codes::PDF_MEASURE_UNSUPPORTED.name(), 1),
            (codes::PDF_SHADING_DROPPED.name(), 1),
        ]
    );
    let annotation = diag
        .iter()
        .find(|d| d.code() == codes::PDF_ANNOTATION_UNSUPPORTED)
        .unwrap();
    assert_eq!(
        annotation.param("kind"),
        Some(&zumen_base::Value::Text("Square".to_owned()))
    );
}

#[test]
fn moves_document_information() {
    let mut document = Document::new(vec![Page::new("")]);
    document.objects = vec![
        b"<< /Title (Plan) /Author <FEFF56F39762> /Producer (zumen) \
          /CreationDate (D:20261023093000+09'00') >>"
            .to_vec(),
    ];
    document.trailer = "/Info 3 0 R";
    let (drawing, _) = lift(&document.bytes());
    let document = drawing
        .extensions
        .iter()
        .find(|extension| extension.namespace == "pdf.document")
        .unwrap();
    let fields: Vec<_> = document
        .fields
        .iter()
        .map(|field| (field.key.as_ref(), field.value.clone()))
        .collect();
    assert_eq!(
        fields,
        [
            ("title", FieldValue::Text("Plan".to_owned())),
            ("author", FieldValue::Text("図面".to_owned())),
            ("producer", FieldValue::Text("zumen".to_owned())),
            (
                "created",
                FieldValue::Text("2026-10-23T09:30:00+09:00".to_owned())
            ),
            ("pages", FieldValue::Integer(1)),
        ]
    );
}

#[test]
fn rejects_documents_that_cannot_be_read() {
    let mut budget = Budget::default();
    let mut diag = Diagnostics::new();
    let cancel = CancelToken::new();
    // ページの木もオブジェクトもない
    let file = testkit::read::<Pdf>(b"%PDF-1.7\n%%EOF\n").0.unwrap();
    let result = file.lift(&mut Cx::new(&mut budget, &mut diag, &cancel));
    assert!(matches!(result, Err(Error::Invalid)), "{result:?}");
    assert_eq!(
        diag.iter().next().unwrap().code(),
        codes::PDF_DOCUMENT_INVALID
    );

    // 空のパスワードで開けない暗号
    let mut document = Document::new(vec![Page::new("")]);
    document.objects = vec![
        b"<< /Filter /Standard /V 1 /R 2 /Length 40 /P -4 \
          /O <0000000000000000000000000000000000000000000000000000000000000000> \
          /U <0000000000000000000000000000000000000000000000000000000000000000> >>"
            .to_vec(),
    ];
    document.trailer = "/Encrypt 3 0 R /ID [<00112233445566778899AABBCCDDEEFF> <00112233445566778899AABBCCDDEEFF>]";
    let file = testkit::read::<Pdf>(&document.bytes()).0.unwrap();
    let mut diag = Diagnostics::new();
    let result = file.lift(&mut Cx::new(&mut budget, &mut diag, &cancel));
    assert!(matches!(result, Err(Error::Unsupported)), "{result:?}");
    assert_eq!(
        diag.iter().next().unwrap().code(),
        codes::PDF_ENCRYPTION_UNSUPPORTED
    );
}

#[test]
fn builds_a_scene_from_a_page() {
    let src = one_page(
        Page::new(
            "1 w 0 0 m 72 0 l S 0 0 1 rg 10 10 50 30 re f BT /F1 12 Tf 72 720 Td (Plan) Tj ET",
        )
        .resources(HELVETICA),
    );
    let (drawing, _) = lift(&src);
    let options = Options {
        origin: Origin::Sheet(SheetId(0)),
        tolerance: None,
        print: false,
        region: None,
        palette: Palette::default(),
        chunking: Chunking::PROVISIONAL,
    };
    let mut budget = Budget::default();
    let mut diag = Diagnostics::new();
    let cancel = CancelToken::new();
    let scene = zumen_scene::build(
        &drawing,
        &options,
        &mut Cx::new(&mut budget, &mut diag, &cancel),
    )
    .map_err(|(stop, _)| stop)
    .unwrap();
    assert!(scene.counts.lines >= 1, "{:?}", scene.counts);
    assert_eq!(scene.counts.faces, 1, "{:?}", scene.counts);
    assert_eq!(scene.counts.texts, 1, "{:?}", scene.counts);
    assert!(!scene.bytes.is_empty());
}

#[test]
fn stops_within_the_budget() {
    use zumen_base::{Kind, Limit, Limits};
    let content: String = (0..100).map(|i| format!("0 {i} m 72 {i} l S ")).collect();
    let src = one_page(Page::new(&content));
    let file = testkit::read::<Pdf>(&src).0.unwrap();
    let mut budget = Budget::new(Limits::default().with(Kind::Elements, Limit::Finite(10)));
    let mut diag = Diagnostics::new();
    let cancel = CancelToken::new();
    let result = file.lift(&mut Cx::new(&mut budget, &mut diag, &cancel));
    assert!(matches!(result, Err(Error::Stopped(_))), "{result:?}");
    assert_eq!(
        diag.iter().next().unwrap().code(),
        codes::BUDGET_ELEMENTS_EXCEEDED
    );
}

#[test]
fn lifts_cut_files_without_panicking() {
    let src = one_page(
        Page::new("1 w 0 0 m 72 0 l S BT /F1 12 Tf 72 720 Td (Plan) Tj ET").resources(HELVETICA),
    );
    for end in (0..src.len()).step_by(3) {
        let Ok(file) = testkit::read::<Pdf>(&src[..end]).0 else {
            continue;
        };
        let mut budget = Budget::default();
        let mut diag = Diagnostics::new();
        let cancel = CancelToken::new();
        let first = file.lift(&mut Cx::new(&mut budget, &mut diag, &cancel));
        let mut again = Diagnostics::new();
        let mut budget = Budget::default();
        let second = file.lift(&mut Cx::new(&mut budget, &mut again, &cancel));
        assert_eq!(
            first.ok(),
            second.ok(),
            "{end} バイトで切ると、2 回の結果が違う"
        );
        assert_eq!(diag.into_vec(), again.into_vec());
    }
}
