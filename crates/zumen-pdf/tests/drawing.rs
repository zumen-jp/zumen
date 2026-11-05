//! PDF から図面モデル（L2）とシーンへの移し（決定 0256）。入力は、どれもテストの中で作る。

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
    // 文書の情報とバージョンは、図面の拡張に持つ
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
fn reports_colours_for_each_element() {
    // 装置は、色を演算子ごとに 1 度だけ RGB にして（多成分の DeviceN の色は、換算のたびに色変換の関数を当てる）、
    // 部分経路と、字形の並びを分けた文字のそれぞれに使う。移さない色の診断は、要素ごとに数える
    let mut document = Document::new(vec![
        Page::new(
            "/N CS 0.5 0.5 SCN 0 0 m 10 0 l 20 0 m 30 0 l 40 0 m 50 0 l S \
             BT /F1 12 Tf /N cs 0.5 0.5 sc 100 Tc 100 100 Td (ABC) Tj ET",
        )
        .resources(
            "/ColorSpace << /N [/DeviceN [/A /B] /DeviceRGB 3 0 R] >> \
             /Font << /F1 << /Type /Font /Subtype /Type1 /BaseFont /Helvetica >> >>",
        ),
    ]);
    document.objects = vec![common::stream(
        "/FunctionType 4 /Domain [0 1 0 1] /Range [0 1 0 1 0 1]",
        "{ pop pop 0 0 0 }",
    )];
    let (drawing, diag) = lift(&document.bytes());
    let count = elements(&drawing).len() as u64;
    assert!(count >= 3 + 2, "{count}");
    let dropped: u64 = diag
        .iter()
        .filter(|d| d.code() == codes::PDF_COLOR_DROPPED)
        .map(|d| d.count())
        .sum();
    assert_eq!(dropped, count, "{diag:?}");
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
    // Helvetica（埋め込みのない Type 1）の名前は、hayro-interpret が渡さない
    assert_eq!(codes_of(&diag), [(codes::PDF_FONT_NAME_DROPPED.name(), 5)]);
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
/// ToUnicode。(100, 100) から「図面」を描く
fn cid_document(base_font: &str, encoding: &str) -> Vec<u8> {
    cid_with(
        base_font,
        encoding,
        "BT /F2 10 Tf 100 100 Td <00010002> Tj ET",
    )
}

/// [`cid_document`] のフォントで、`content` を描く
fn cid_with(base_font: &str, encoding: &str, content: &str) -> Vec<u8> {
    cid_font_with(base_font, encoding, "", content)
}

/// [`cid_document`] のフォントの子孫のフォントに `descendant` を足し、`content` を描く
fn cid_font_with(base_font: &str, encoding: &str, descendant: &str, content: &str) -> Vec<u8> {
    cid_font_document(base_font, encoding, descendant, content).bytes()
}

/// [`cid_font_with`] の文書。フォントは 3 番、ToUnicode は 4 番
fn cid_font_document<'a>(
    base_font: &str,
    encoding: &str,
    descendant: &str,
    content: &'a str,
) -> Document<'a> {
    let font = cid_font(base_font, encoding, descendant);
    let mut document = Document::new(vec![Page::new(content).resources("/Font << /F2 3 0 R >>")]);
    document.objects = vec![font.into_bytes(), cid_to_unicode()];
    document
}

/// [`cid_document`] のフォントの辞書。子孫のフォントに `descendant` を足す。ToUnicode は 4 番
fn cid_font(base_font: &str, encoding: &str, descendant: &str) -> String {
    format!(
        "<< /Type /Font /Subtype /Type0 /BaseFont /{base_font} /Encoding /{encoding} \
         /DescendantFonts [<< /Type /Font /Subtype /CIDFontType2 /BaseFont /{base_font} \
         /CIDSystemInfo << /Registry (Adobe) /Ordering (Japan1) /Supplement 6 >> \
         /FontDescriptor << /Type /FontDescriptor /FontName /{base_font} /Flags 4 \
         /FontBBox [0 -141 1000 859] /ItalicAngle 0 /Ascent 859 /Descent -141 /CapHeight 700 \
         /StemV 80 >> /DW 1000 {descendant} >>] /ToUnicode 4 0 R >>"
    )
}

/// [`cid_document`] のフォントの ToUnicode のストリーム
fn cid_to_unicode() -> Vec<u8> {
    let cmap = "/CIDInit /ProcSet findresource begin 12 dict begin begincmap \
        /CIDSystemInfo << /Registry (Adobe) /Ordering (UCS) /Supplement 0 >> def \
        /CMapName /Adobe-Identity-UCS def /CMapType 2 def \
        1 begincodespacerange <0000> <FFFF> endcodespacerange \
        2 beginbfchar <0001> <56F3> <0002> <9762> endbfchar \
        endcmap CMapName currentdict /CMap defineresource pop end end";
    common::stream("", cmap)
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
    // 隠れた OCG の内容は、hayro-interpret が塗らない
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
    // 数の木の節は間接参照で書く（7.9.7）が、直接書いた節も、根と同じ節として飛ばさずにたどる
    let mut document = Document::new(vec![Page::new(""), Page::new("")]);
    document.catalog =
        "/PageLabels << /Kids [<< /Nums [0 << /S /r >>] >> << /Nums [1 << /S /D /St 5 >>] >>] >>";
    let (drawing, _) = lift(&document.bytes());
    let names: Vec<_> = drawing
        .sheets
        .iter()
        .map(|sheet| sheet.name.clone())
        .collect();
    assert_eq!(names, [Some("i".to_owned()), Some("5".to_owned())]);
}

#[test]
fn counts_memory_of_page_labels() {
    use zumen_base::Kind;
    // ページラベルの範囲は、1 つを 256 バイトと数える
    let tree = format!(
        "/PageLabels << /Kids [{}] >>",
        "<< /Nums [0 << /S /D >>] >> ".repeat(1_000)
    );
    let mut document = Document::new(vec![Page::new("")]);
    document.catalog = &tree;
    let bytes = used(&document.bytes(), Kind::DecodedBytes);
    assert!(bytes >= 1_000 * 256, "{bytes}");
    // ラベルは図面モデルに写すので、前に付ける文字列を、ページごとに読み直す手間とメモリに数える
    let tree = format!(
        "/PageLabels << /Nums [0 << /P ({}) >>] >>",
        "x".repeat(10_000)
    );
    let mut document = Document::new(vec![Page::new(""); 100]);
    document.catalog = &tree;
    let bytes = used(&document.bytes(), Kind::DecodedBytes);
    assert!(bytes >= 100 * 10_000 * 6, "{bytes}");
    let work = used(&document.bytes(), Kind::Work);
    assert!(work >= 100 * 10_000 / 4, "{work}");
}

#[test]
fn counts_repeated_references_in_page_labels() {
    use zumen_base::Kind;
    // `Nums` の値は、参照を解くたびに読み直す
    let range = format!("<< /P ({}) >>", "x".repeat(10_000)).into_bytes();
    let nums: String = (0..1_000).map(|at| format!("{at} 3 0 R ")).collect();
    let tree = format!("/PageLabels << /Nums [{nums}] >>");
    let mut document = Document::new(vec![Page::new("")]);
    document.catalog = &tree;
    document.objects = vec![range];
    let work = used(&document.bytes(), Kind::Work);
    assert!(work >= 1_000 * 10_000 / 4, "{work}");
    // `Kids` の同じ参照は、2 度解かない
    let node = format!("<< /Nums [0 << /P ({}) >>] >>", "x".repeat(10_000)).into_bytes();
    let tree = format!("/PageLabels << /Kids [{}] >>", "3 0 R ".repeat(10_000));
    let mut document = Document::new(vec![Page::new("")]);
    document.catalog = &tree;
    document.objects = vec![node];
    let work = used(&document.bytes(), Kind::Work);
    assert!(work < 1_000_000, "{work}");
    let (drawing, _) = lift(&document.bytes());
    assert_eq!(
        drawing.sheets[0].name.as_deref().map(str::len),
        Some(10_000)
    );
    // 辞書でない項目から後ろは、読まない
    let mut document = Document::new(vec![Page::new(""), Page::new("")]);
    document.catalog =
        "/PageLabels << /Kids [<< /Nums [0 << /S /r >>] >> 5 << /Nums [1 << /S /D >>] >>] >>";
    let (drawing, _) = lift(&document.bytes());
    let names: Vec<_> = drawing
        .sheets
        .iter()
        .map(|sheet| sheet.name.clone())
        .collect();
    assert_eq!(names, [Some("i".to_owned()), Some("ii".to_owned())]);
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
        panic!("アンダーレイでない");
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

    // 資源の辞書に直接書いた画像（7.3.8.1 に反する）は、コンテナーのページの番号を名前にしない
    let image = direct_stream(
        "/Type /XObject /Subtype /Image /Width 1 /Height 1 /ColorSpace /DeviceGray \
         /BitsPerComponent 8",
        &[0],
    );
    let resources = format!("/XObject << /A {image} /B {image} >>");
    let (drawing, _) = lift(&one_page(Page::new("/A Do /B Do").resources(&resources)));
    let paths: Vec<_> = elements(&drawing)
        .iter()
        .map(|element| match &element.shape {
            Shape::Underlay(underlay) => underlay.resource.path.clone(),
            _ => panic!("アンダーレイでない"),
        })
        .collect();
    assert_eq!(paths, ["", ""]);
}

#[test]
fn reports_type3_glyphs_and_image_masks() {
    // Type 3 のフォントの字形は書体のない文字に、色を塗る画像（ImageMask）はレイヤの色のアンダーレイにする
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
        panic!("文字とアンダーレイでない: {shapes:?}");
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

#[test]
fn reports_contents_that_cannot_be_read() {
    // 内容のストリームの配列の、壊れた Flate（途中で切れたものと、Flate でないもの）と、ストリームでない項目
    let good = common::stream("/Filter /FlateDecode", common::zlib(b"0 0 m 72 0 l S"));
    let mut cut = common::zlib(b"0 0 m 72 0 l S 0 10 m 72 10 l S");
    cut.truncate(20);
    let cut = common::stream("/Filter /FlateDecode", cut);
    let garbage = common::stream("/Filter /FlateDecode", "not flate at all");
    let mut document = Document::new(vec![Page::new("").contents("[3 0 R 4 0 R 5 0 R 6 0 R]")]);
    document.objects = vec![good, cut, garbage, b"(not a stream)".to_vec()];
    let src = document.bytes();
    let (drawing, diag) = lift(&src);
    assert_eq!(elements(&drawing).len(), 1);
    assert_eq!(codes_of(&diag), [(codes::PDF_CONTENT_INVALID.name(), 3)]);
    // 場所は、読めない項目のレコード
    let file = testkit::read::<Pdf>(&src).0.unwrap();
    let objects: Vec<_> = diag[0]
        .places()
        .iter()
        .filter_map(|place| place.record())
        .map(|record| file.records()[record as usize].object)
        .collect();
    assert_eq!(objects, [Some((4, 0)), Some((5, 0)), Some((6, 0))]);

    // 引数の足りない演算子から後ろは、hayro-interpret も読まない
    let (drawing, diag) = lift(&one_page(Page::new("0 0 m 72 0 l S 1 m 0 10 m 72 10 l S")));
    assert_eq!(elements(&drawing).len(), 1);
    assert_eq!(codes_of(&diag), [(codes::PDF_CONTENT_INVALID.name(), 1)]);

    // BBox のないフォームと、大きさのない画像は描かれない
    let mut document = Document::new(vec![
        Page::new("/Fm Do q 10 0 0 10 0 0 cm /Im Do Q")
            .resources("/XObject << /Fm 3 0 R /Im 4 0 R >>"),
    ]);
    document.objects = vec![
        common::stream("/Type /XObject /Subtype /Form", "0 0 m 10 10 l S"),
        common::stream(
            "/Type /XObject /Subtype /Image /Height 1 /ColorSpace /DeviceGray /BitsPerComponent 8",
            [0u8],
        ),
    ];
    let (drawing, diag) = lift(&document.bytes());
    assert!(elements(&drawing).is_empty());
    assert_eq!(codes_of(&diag), [(codes::PDF_CONTENT_INVALID.name(), 2)]);
}

/// 文字の中身
#[cfg(test)]
fn contents(drawing: &Drawing2D) -> Vec<&str> {
    drawing
        .sheets
        .iter()
        .flat_map(|sheet| &sheet.elements)
        .map(|element| match &element.shape {
            Shape::Text(text) => text.content.as_str(),
            other => panic!("文字でない: {other:?}"),
        })
        .collect()
}

/// 診断の code の名前と、値 `name` と、件数。並べたもの
fn named(diag: &[zumen_base::Diagnostic]) -> Vec<(&'static str, String, u64)> {
    let mut found: Vec<_> = diag
        .iter()
        .map(|d| {
            let name = match d.param("name") {
                Some(zumen_base::Value::Text(name)) => name.clone(),
                _ => String::new(),
            };
            (d.code().name(), name, d.count())
        })
        .collect();
    found.sort();
    found
}

#[test]
fn reads_missing_and_unreadable_fonts_with_helvetica() {
    let fonts = "/Font << /F1 << /Type /Font /Subtype /Type1 /BaseFont /Helvetica >> \
        /F2 << /Type /Font /Subtype /Type0 /BaseFont /Broken /Encoding /Identity-H >> >>";
    let src = Document::new(vec![
        // フォントを選ばずに ' で描いた文字は描かれず、Tj で描いた文字は Helvetica で読まれる
        Page::new("BT 72 700 Td (pq) ' (xyz) Tj ET").resources(fonts),
        // 資源にないフォント。ASCII でないバイトを含む文字列は描かれない
        Page::new("BT /F9 12 Tf 72 700 Td (abc) Tj <82A0> Tj ET").resources(fonts),
        // 読めないフォント（DescendantFonts のない Type 0）。同じフォントは 1 件
        Page::new("BT /F2 12 Tf 72 700 Td (AB) Tj <8140> Tj /F2 10 Tf (CD) Tj ET").resources(fonts),
    ])
    .bytes();
    let (drawing, diag) = lift(&src);
    assert_eq!(contents(&drawing), ["xyz", "abc", "AB", "CD"]);
    assert_eq!(
        named(&diag),
        [
            ("pdf.font-name.dropped", String::new(), 4),
            ("pdf.font.assumed", String::new(), 1),
            ("pdf.font.missing", "F9".to_owned(), 1),
            ("pdf.font.unsupported", "Broken".to_owned(), 1),
            ("pdf.text.dropped", String::new(), 3),
        ]
    );
    // 見えない所の文字で Helvetica を選んでも、見える所の文字で診断する
    let mut document = Document::new(vec![
        Page::new("BT 72 700 Td /OC /oc BDC (A) Tj EMC (B) Tj ET")
            .resources("/Properties << /oc 3 0 R >>"),
    ]);
    document.objects = vec![b"<< /Type /OCG /Name (Hidden) >>".to_vec()];
    document.catalog = "/OCProperties << /OCGs [3 0 R] /D << /OFF [3 0 R] >> >>";
    let (drawing, diag) = lift(&document.bytes());
    assert_eq!(contents(&drawing), ["B"]);
    assert_eq!(
        named(&diag),
        [
            ("pdf.font-name.dropped", String::new(), 1),
            ("pdf.font.assumed", String::new(), 1),
            ("pdf.optional-content.dropped", "Hidden".to_owned(), 1),
        ]
    );
}

#[test]
fn reports_soft_masks_and_blend_modes() {
    let mut document = Document::new(vec![
        Page::new(
            "/Bm gs 0 0 50 50 re f /Sm gs 0 0 m 10 0 l S /N gs \
             BT /F1 12 Tf /Bm gs 72 700 Td (A) Tj ET /N gs \
             q 10 0 0 10 200 200 cm /Im Do Q 0 0 m 20 0 l S",
        )
        .resources(
            "/Font << /F1 << /Type /Font /Subtype /Type1 /BaseFont /Helvetica >> >> \
             /ExtGState << /Bm << /BM /Multiply >> \
             /Sm << /SMask << /Type /Mask /S /Luminosity /G 3 0 R >> >> \
             /N << /BM /Normal /SMask /None >> >> /XObject << /Im 4 0 R >>",
        ),
    ]);
    document.objects = vec![
        common::stream(
            "/Type /XObject /Subtype /Form /BBox [0 0 10 10] \
             /Group << /S /Transparency /CS /DeviceGray >>",
            "1 g 0 0 10 10 re f",
        ),
        common::stream(
            "/Type /XObject /Subtype /Image /Width 1 /Height 1 /ColorSpace /DeviceGray \
             /BitsPerComponent 8 /SMask 5 0 R",
            [128u8],
        ),
        common::stream(
            "/Type /XObject /Subtype /Image /Width 1 /Height 1 /ColorSpace /DeviceGray \
             /BitsPerComponent 8",
            [255u8],
        ),
    ];
    let (drawing, diag) = lift(&document.bytes());
    // 塗り・線・文字・画像のソフトマスクと合成の方法を診断し、通常に戻した後の線は診断しない
    assert_eq!(elements(&drawing).len(), 5);
    let mut found = codes_of(&diag);
    found.sort();
    assert_eq!(
        found,
        [
            (codes::PDF_FONT_NAME_DROPPED.name(), 1),
            (codes::PDF_TRANSPARENCY_APPROXIMATED.name(), 4),
        ]
    );
}

#[test]
fn reports_patterns_and_glyphs_without_unicode() {
    let mut document = Document::new(vec![
        Page::new("/Pattern cs /P1 scn 0 0 10 10 re f").resources("/Pattern << /P1 3 0 R >>"),
    ]);
    document.objects = vec![common::stream(
        "/PatternType 1 /PaintType 1 /TilingType 1 /BBox [0 0 10 10] /XStep 10 /YStep 10 \
         /Resources << >>",
        "0 0 5 5 re f",
    )];
    let (drawing, diag) = lift(&document.bytes());
    assert!(elements(&drawing).is_empty());
    assert_eq!(codes_of(&diag), [(codes::PDF_PATTERN_DROPPED.name(), 1)]);

    // ToUnicode にない字形は U+FFFD にする
    let (drawing, diag) = lift(&cid_with(
        "MS-Gothic",
        "Identity-H",
        "BT /F2 10 Tf 100 100 Td <00010003> Tj ET",
    ));
    assert_eq!(contents(&drawing), ["図\u{FFFD}"]);
    assert_eq!(codes_of(&diag), [(codes::PDF_GLYPH_DROPPED.name(), 1)]);
}

#[test]
fn places_vertical_and_mirrored_text() {
    // 縦書きの文字の基点は、縦の原点（文字を描き始めた点）
    let (drawing, _) = lift(&cid_document("MS-Gothic", "Identity-V"));
    let Shape::Text(text) = &elements(&drawing)[0].shape else {
        panic!("文字でない");
    };
    assert_eq!(text.content, "図面");
    let Placement::Anchor {
        point,
        horizontal,
        vertical,
    } = &text.placement
    else {
        panic!("基点で置いていない");
    };
    assert_point(point, mm(100.0), mm(100.0));
    assert_eq!(
        (*horizontal, *vertical),
        (Horizontal::Center, Vertical::Top)
    );

    // 裏返しの行列は、基準線の向きのまま、上下を裏返す
    let src = one_page(
        Page::new("BT /F1 10 Tf 1 0 0 -1 100 100 Tm (A) Tj -1 0 0 1 200 100 Tm (B) Tj ET")
            .resources(HELVETICA),
    );
    let (drawing, _) = lift(&src);
    let texts: Vec<_> = elements(&drawing)
        .iter()
        .map(|element| match &element.shape {
            Shape::Text(text) => text,
            other => panic!("文字でない: {other:?}"),
        })
        .collect();
    assert!(near(texts[0].rotation, 0.0), "{}", texts[0].rotation);
    assert!(texts[0].upside_down && !texts[0].backward);
    assert!(
        near(texts[1].rotation.abs(), 180.0),
        "{}",
        texts[1].rotation
    );
    assert!(texts[1].upside_down && !texts[1].backward);
}

#[test]
fn reports_transfer_functions() {
    let states = "/ExtGState << /Id << /TR /Identity >> \
        /Tr << /TR << /FunctionType 2 /Domain [0 1] /C0 [0] /C1 [1] /N 2.2 >> >> >>";
    let src = Document::new(vec![
        Page::new("/Id gs 0.5 g 0 0 10 10 re f").resources(states),
        Page::new("0.5 g 0 0 10 10 re f /Tr gs 0 0 10 10 re f").resources(states),
    ])
    .bytes();
    let (drawing, diag) = lift(&src);
    // 関数を外す名前（Identity）は、色を変えない
    assert_eq!(rgb(&drawing.sheets[0].elements[0]), Some([128, 128, 128]));
    // 転送関数を使うページの色は、すべてレイヤに従う
    assert!(
        drawing.sheets[1]
            .elements
            .iter()
            .all(|element| element.style.color == Inherit::Layer)
    );
    assert_eq!(codes_of(&diag), [(codes::PDF_TRANSFER_DROPPED.name(), 2)]);
}

#[test]
fn drops_shapes_without_finite_coordinates() {
    let big = "9".repeat(39);
    let scale = format!("{big} 0 0 {big} 0 0 cm ").repeat(9);
    let content = format!(
        "q {scale} 0 0 m 10 10 l S 0 0 10 10 re f Q \
         BT /F1 0 Tf 72 700 Td (A) Tj ET BT /F1 10 Tf 0 Tz 72 600 Td (B) Tj ET 0 0 m 10 0 l S"
    );
    let (drawing, diag) = lift(&one_page(Page::new(&content).resources(HELVETICA)));
    assert_eq!(elements(&drawing).len(), 1);
    assert_eq!(codes_of(&diag), [(codes::PDF_GEOMETRY_DROPPED.name(), 4)]);
}

#[test]
fn reports_content_hidden_by_membership_dictionaries() {
    let mut document = Document::new(vec![
        Page::new("/OC /md BDC 0 0 m 10 10 l S EMC 0 0 m 20 20 l S")
            .resources("/Properties << /md 4 0 R >>"),
    ]);
    document.objects = vec![
        b"<< /Type /OCG /Name (Walls) >>".to_vec(),
        b"<< /Type /OCMD /OCGs [3 0 R] /P /AllOff >>".to_vec(),
    ];
    document.catalog = "/OCProperties << /OCGs [3 0 R] /D << >> >>";
    let (drawing, diag) = lift(&document.bytes());
    assert_eq!(elements(&drawing).len(), 1);
    assert_eq!(codes_of(&diag), [(codes::PDF_MEMBERSHIP_DROPPED.name(), 1)]);
}

/// 段ごとに次の段のフォームを 2 回呼ぶ、深さ `depth` のフォームの鎖。最後の段は `leaf` を描く
fn doubling_forms(depth: usize, leaf: &str) -> Vec<u8> {
    let mut document = Document::new(vec![
        Page::new("/X Do").resources("/XObject << /X 3 0 R >>"),
    ]);
    document.objects = (0..=depth)
        .map(|level| {
            if level == depth {
                common::stream("/Type /XObject /Subtype /Form /BBox [0 0 100 100]", leaf)
            } else {
                let dict = format!(
                    "/Type /XObject /Subtype /Form /BBox [0 0 100 100] \
                     /Resources << /XObject << /X {} 0 R >> >>",
                    4 + level
                );
                common::stream(&dict, "/X Do /X Do")
            }
        })
        .collect();
    document.bytes()
}

#[test]
fn counts_nested_forms_before_interpreting() {
    use zumen_base::{Kind, Limit, Limits};
    // 2 の 30 乗回の呼び出しを、解釈する前に数えて止める。末端が何も描かなくても止める
    for leaf in ["0 0 m 1 1 l S", ""] {
        let file = testkit::read::<Pdf>(&doubling_forms(30, leaf)).0.unwrap();
        let mut budget = Budget::new(Limits::default().with(Kind::Work, Limit::Finite(100_000)));
        let mut diag = Diagnostics::new();
        let cancel = CancelToken::new();
        let result = file.lift(&mut Cx::new(&mut budget, &mut diag, &cancel));
        assert!(matches!(result, Err(Error::Stopped(_))), "{result:?}");
        assert_eq!(
            diag.iter().next().unwrap().code(),
            codes::BUDGET_WORK_EXCEEDED
        );
    }
    // 中断の合図も、解釈する前に見る
    let file = testkit::read::<Pdf>(&doubling_forms(30, "")).0.unwrap();
    let mut budget = Budget::default();
    let mut diag = Diagnostics::new();
    let cancel = CancelToken::new();
    cancel.cancel();
    let result = file.lift(&mut Cx::new(&mut budget, &mut diag, &cancel));
    assert!(matches!(result, Err(Error::Stopped(_))), "{result:?}");
    // 予算に収まる深さは、呼んだ数の要素を移す
    let (drawing, _) = lift(&doubling_forms(4, "0 0 m 1 1 l S"));
    assert_eq!(elements(&drawing).len(), 16);
    // フォームの呼び出しは、hayro-interpret の手間に合わせて、簡単な演算子の 50 倍に数える
    let file = testkit::read::<Pdf>(&doubling_forms(10, "")).0.unwrap();
    let mut budget = Budget::default();
    let mut diag = Diagnostics::new();
    let cancel = CancelToken::new();
    assert!(
        file.lift(&mut Cx::new(&mut budget, &mut diag, &cancel))
            .is_ok()
    );
    let calls = (1 << 11) - 1;
    assert!(
        budget.used(Kind::Work) >= 50 * calls,
        "{}",
        budget.used(Kind::Work)
    );
    // 既定の `Work` の上限でも、2 の 29 乗回の呼び出しは、解釈する前に止める
    let file = testkit::read::<Pdf>(&doubling_forms(28, "")).0.unwrap();
    let mut budget = Budget::new(Limits::default().with(Kind::DecodedBytes, Limit::Unlimited));
    let mut diag = Diagnostics::new();
    let result = file.lift(&mut Cx::new(&mut budget, &mut diag, &cancel));
    assert!(matches!(result, Err(Error::Stopped(_))), "{result:?}");
    assert_eq!(
        diag.iter().next().unwrap().code(),
        codes::BUDGET_WORK_EXCEEDED
    );
}

#[test]
fn splits_text_at_large_gaps() {
    let src = one_page(
        Page::new(
            "BT /F1 12 Tf 72 700 Td [(A) -5000 (B) -9000 (C)] TJ ET \
             BT /F1 12 Tf 72 600 Td [(A) -100 (B)] TJ ET \
             BT /F1 12 Tf 72 500 Td [(A) -20 (B)] TJ ET \
             BT /F1 12 Tf 72 400 Td [(A) 600 (BCDE)] TJ ET",
        )
        .resources(HELVETICA),
    );
    let (drawing, diag) = lift(&src);
    // 大きな隙間で分け、小さなずれは 1 つの文字にする。詰めた所が 1 つあっても、ほかの字形の間は分けない
    assert_eq!(contents(&drawing), ["A", "B", "C", "AB", "AB", "A", "BCDE"]);
    // B は、A の送り（Helvetica の 667）と、TJ の 5,000 の後ろ
    let Shape::Text(text) = &elements(&drawing)[1].shape else {
        panic!("文字でない");
    };
    let Placement::Anchor { point, .. } = &text.placement else {
        panic!("基点で置いていない");
    };
    let x = mm(72.0 + 12.0 * 0.667 + 60.0);
    assert!((point.x - x).abs() < 1e-3, "{} と {x} が違う", point.x);
    let mut found = codes_of(&diag);
    found.sort();
    assert_eq!(
        found,
        [
            (codes::PDF_FONT_NAME_DROPPED.name(), 7),
            (codes::PDF_GLYPH_POSITION_APPROXIMATED.name(), 1),
        ]
    );
}

#[test]
fn names_the_newer_version() {
    for (catalog, version) in [
        ("", "1.7"),
        ("/Version /1.4", "1.7"),
        ("/Version /2.0", "2.0"),
    ] {
        let mut document = Document::new(vec![Page::new("")]);
        document.catalog = catalog;
        let (drawing, _) = lift(&document.bytes());
        let found = drawing
            .extensions
            .iter()
            .find(|extension| extension.namespace == "pdf.document")
            .unwrap();
        assert_eq!(found.version.as_deref(), Some(version), "{catalog}");
    }
}

#[test]
fn stops_at_the_same_place_every_time() {
    use zumen_base::{Kind, Limit, Limits};
    let content: String = (0..50).map(|i| format!("0 {i} m 72 {i} l S ")).collect();
    let src = one_page(Page::new(&content));
    let file = testkit::read::<Pdf>(&src).0.unwrap();
    for (kind, limit) in [
        (Kind::Work, 20),
        (Kind::Work, 200),
        (Kind::Elements, 10),
        (Kind::DecodedBytes, 100),
    ] {
        let run = || {
            let mut budget = Budget::new(Limits::default().with(kind, Limit::Finite(limit)));
            let mut diag = Diagnostics::new();
            let cancel = CancelToken::new();
            let result = file.lift(&mut Cx::new(&mut budget, &mut diag, &cancel));
            format!("{result:?} {:?}", diag.into_vec())
        };
        let first = run();
        assert!(
            first.starts_with("Err(Stopped"),
            "{kind:?} {limit}: {first}"
        );
        assert_eq!(first, run(), "{kind:?} {limit}");
    }
}

#[test]
fn reads_flate_without_checksums_and_with_predictors() {
    let content = b"0 0 m 72 0 l S";
    // Adler-32 が違う zlib と、Adler-32 のない zlib は、deflate を終わりまで読めるので、壊れていない
    let mut wrong = common::zlib(content);
    if let Some(last) = wrong.last_mut() {
        *last ^= 0xff;
    }
    let mut missing = common::zlib(content);
    missing.truncate(missing.len() - 4);
    // 配列の DecodeParms の予測子（7.4.4.4）。行ごとに、予測なし（0）の種類のバイトを頭に付ける
    let mut predicted = vec![0u8];
    predicted.extend(content);
    for (dict, data) in [
        ("/Filter /FlateDecode", wrong),
        ("/Filter /FlateDecode", missing),
        (
            "/Filter [/FlateDecode] /DecodeParms [<< /Predictor 12 /Columns 14 >>]",
            common::zlib(&predicted),
        ),
    ] {
        let mut document = Document::new(vec![Page::new("").contents("3 0 R")]);
        document.objects = vec![common::stream(dict, data)];
        let (drawing, diag) = lift(&document.bytes());
        assert_eq!(elements(&drawing).len(), 1, "{dict}");
        assert!(diag.is_empty(), "{dict}: {diag:?}");
    }
}

/// 段ごとに次の段のフォームを 1 回呼ぶ、`levels` 段のフォームの鎖。最後の段は線を描く
fn nested_forms(levels: usize) -> Vec<u8> {
    let mut document = Document::new(vec![
        Page::new("/X Do").resources("/XObject << /X 3 0 R >>"),
    ]);
    document.objects = (1..=levels)
        .map(|level| {
            if level == levels {
                common::stream(
                    "/Type /XObject /Subtype /Form /BBox [0 0 100 100]",
                    "0 0 m 10 10 l S",
                )
            } else {
                let dict = format!(
                    "/Type /XObject /Subtype /Form /BBox [0 0 100 100] \
                     /Resources << /XObject << /X {} 0 R >> >>",
                    3 + level
                );
                common::stream(&dict, "/X Do")
            }
        })
        .collect();
    document.bytes()
}

#[test]
fn reports_forms_nested_too_deep() {
    // hayro-interpret は、50 段までのフォームを解釈する
    let (drawing, diag) = lift(&nested_forms(50));
    assert_eq!(elements(&drawing).len(), 1);
    assert!(diag.is_empty(), "{diag:?}");
    let (drawing, diag) = lift(&nested_forms(51));
    assert!(elements(&drawing).is_empty());
    assert_eq!(codes_of(&diag), [(codes::PDF_NESTING_DROPPED.name(), 1)]);
}

#[test]
fn follows_marked_content_across_forms() {
    // 閉じない BDC を持つフォームの後ろは、hayro-interpret と同じく隠れる。同じフォームの 2 回目も、たどり直す。
    // BBox のないフォーム（/Bad）は、見えるときだけ診断するので、走査の見え方を確かめられる
    let mut document = Document::new(vec![
        Page::new("/Open Do /Bad Do EMC /Bad Do /Line Do /Open Do /Bad Do /Line Do").resources(
            "/XObject << /Open 4 0 R /Line 5 0 R /Bad 6 0 R >> /Properties << /oc 3 0 R >>",
        ),
    ]);
    document.objects = vec![
        b"<< /Type /OCG /Name (Hidden) >>".to_vec(),
        common::stream(
            "/Type /XObject /Subtype /Form /BBox [0 0 100 100] \
             /Resources << /Properties << /oc 3 0 R >> >>",
            "/OC /oc BDC",
        ),
        common::stream(
            "/Type /XObject /Subtype /Form /BBox [0 0 100 100]",
            "0 0 m 20 20 l S",
        ),
        common::stream("/Type /XObject /Subtype /Form", "0 0 m 30 30 l S"),
    ];
    document.catalog = "/OCProperties << /OCGs [3 0 R] /D << /OFF [3 0 R] >> >>";
    let (drawing, diag) = lift(&document.bytes());
    // 見えるのは、EMC の後ろの /Bad と、1 回目の /Line だけ
    assert_eq!(elements(&drawing).len(), 1);
    let invalid: Vec<_> = codes_of(&diag)
        .into_iter()
        .filter(|(code, _)| *code == codes::PDF_CONTENT_INVALID.name())
        .collect();
    assert_eq!(invalid, [(codes::PDF_CONTENT_INVALID.name(), 1)]);
}

#[test]
fn reports_round_caps_and_joins() {
    for (content, approximated) in [
        ("0 0 m 72 0 l S", false),
        ("1 J 0 0 m 72 0 l S", true),
        ("2 J 0 0 m 72 0 l S", true),
        // 線分が 1 つなら、つなぎはない
        ("1 j 0 0 m 72 0 l S", false),
        ("1 j 0 0 m 72 0 l 72 72 l S", true),
        ("2 j 0 0 72 72 re S", true),
        // 閉じた部分経路には、端がない
        ("1 J 0 0 72 72 re S", false),
    ] {
        let (drawing, diag) = lift(&one_page(Page::new(content)));
        assert_eq!(elements(&drawing).len(), 1, "{content}");
        let found = codes_of(&diag);
        let expected: &[(&str, u64)] = if approximated {
            &[("pdf.stroke.approximated", 1)]
        } else {
            &[]
        };
        assert_eq!(found, expected, "{content}");
    }
}

#[test]
fn counts_functions_made_for_each_operator() {
    use zumen_base::{Kind, Limit, Limits};
    // 1 万バイトの Type 4 の関数（3 番）。hayro-interpret は、使う演算子ごとに作り直して展開する
    let mut program = String::from("{ ");
    program.push_str(&" ".repeat(10_000));
    program.push('}');
    let function = common::stream("/FunctionType 4 /Domain [0 1] /Range [0 1]", program);
    let image = common::stream(
        "/Type /XObject /Subtype /Image /Width 1 /Height 1 /BitsPerComponent 8 \
         /ColorSpace [/Separation /Spot /DeviceGray 3 0 R]",
        [0u8],
    );
    let states = "/T gs ".repeat(100);
    let shadings = "/Sh sh ".repeat(100);
    let images = "q 10 0 0 10 0 0 cm /Im Do Q ".repeat(100);
    for (content, resources) in [
        (states.as_str(), "/ExtGState << /T << /TR 3 0 R >> >>"),
        (
            shadings.as_str(),
            "/Shading << /Sh << /ShadingType 2 /ColorSpace /DeviceGray /Coords [0 0 1 0] \
             /Function 3 0 R >> >>",
        ),
        (images.as_str(), "/XObject << /Im 4 0 R >>"),
    ] {
        let mut document = Document::new(vec![Page::new(content).resources(resources)]);
        document.objects = vec![function.clone(), image.clone()];
        let src = document.bytes();
        // 100 回で 1 MB を超えるので、上限 500,000 バイトで止まる
        let file = testkit::read::<Pdf>(&src).0.unwrap();
        let limits = Limits::default().with(Kind::DecodedBytes, Limit::Finite(500_000));
        let mut budget = Budget::new(limits);
        let mut diag = Diagnostics::new();
        let cancel = CancelToken::new();
        let result = file.lift(&mut Cx::new(&mut budget, &mut diag, &cancel));
        assert!(
            matches!(result, Err(Error::Stopped(_))),
            "{resources}: {result:?}"
        );
        assert!(
            diag.iter()
                .any(|d| d.code() == codes::BUDGET_DECODED_BYTES_EXCEEDED),
            "{resources}"
        );
        // 既定の上限なら移せる
        let (_, diag) = lift(&src);
        assert!(
            !diag
                .iter()
                .any(|d| d.code() == codes::BUDGET_DECODED_BYTES_EXCEEDED),
            "{resources}"
        );
    }
}

#[test]
fn reports_missing_x_objects() {
    // 資源にない名前と、自分を呼ぶフォーム（hayro-syntax は、循環する参照を引かない）
    let mut document = Document::new(vec![
        Page::new("/Nope Do /Me Do").resources("/XObject << /Me 3 0 R >>"),
    ]);
    document.objects = vec![common::stream(
        "/Type /XObject /Subtype /Form /BBox [0 0 100 100] \
         /Resources << /XObject << /Me 3 0 R >> >>",
        "0 0 m 10 10 l S /Me Do",
    )];
    let (drawing, diag) = lift(&document.bytes());
    assert_eq!(elements(&drawing).len(), 1);
    assert_eq!(
        named(&diag),
        [
            ("pdf.xobject.missing", "Me".to_owned(), 1),
            ("pdf.xobject.missing", "Nope".to_owned(), 1),
        ]
    );
}

#[test]
fn counts_diagnostics_in_each_call_of_a_form() {
    // 2 回呼ぶフォームの中の、BBox のないフォーム。走査は 1 度だけたどるが、件数は 2
    let mut document = Document::new(vec![
        Page::new("/A Do /A Do").resources("/XObject << /A 3 0 R >>"),
    ]);
    document.objects = vec![
        common::stream(
            "/Type /XObject /Subtype /Form /BBox [0 0 100 100] \
             /Resources << /XObject << /Bad 4 0 R >> >>",
            "/Bad Do",
        ),
        common::stream("/Type /XObject /Subtype /Form", "0 0 m 10 10 l S"),
    ];
    let (drawing, diag) = lift(&document.bytes());
    assert!(elements(&drawing).is_empty());
    assert_eq!(codes_of(&diag), [(codes::PDF_CONTENT_INVALID.name(), 2)]);
    // 入れ子の中でも、呼んだ回数を掛ける（2 回 × 2 回）
    let mut document = Document::new(vec![
        Page::new("/A Do /A Do").resources("/XObject << /A 3 0 R >>"),
    ]);
    document.objects = vec![
        common::stream(
            "/Type /XObject /Subtype /Form /BBox [0 0 100 100] \
             /Resources << /XObject << /B 4 0 R >> >>",
            "/B Do /B Do",
        ),
        common::stream(
            "/Type /XObject /Subtype /Form /BBox [0 0 100 100] \
             /Resources << /XObject << /Bad 5 0 R >> >>",
            "/Bad Do",
        ),
        common::stream("/Type /XObject /Subtype /Form", "0 0 m 10 10 l S"),
    ];
    let (_, diag) = lift(&document.bytes());
    assert_eq!(codes_of(&diag), [(codes::PDF_CONTENT_INVALID.name(), 4)]);
}

#[test]
fn reports_vertical_fonts_with_other_origins() {
    let content = "BT /F2 10 Tf 100 100 Td <00010002> Tj ET";
    // 字形の幅が 1,000 で、位置のベクトルが既定（DW2 と W2）なら、診断しない
    for descendant in [
        "",
        "/W [1 [1000 1000] 3 4 1000]",
        "/W2 [1 [-1000 500 880]]",
        "/W2 [1 2 -1000 500 880]",
    ] {
        let (_, diag) = lift(&cid_font_with(
            "MS-Gothic",
            "Identity-V",
            descendant,
            content,
        ));
        assert!(diag.is_empty(), "{descendant}: {diag:?}");
    }
    for descendant in [
        "/W2 [1 [-1000 400 880]]",
        "/W2 [1 2 -1000 500 800]",
        "/DW2 [900 -1000]",
        "/W [1 [500 1000]]",
        "/W [1 2 500]",
    ] {
        let (_, diag) = lift(&cid_font_with(
            "MS-Gothic",
            "Identity-V",
            descendant,
            content,
        ));
        assert_eq!(
            named(&diag),
            [(
                "pdf.vertical-origin.approximated",
                "MS-Gothic".to_owned(),
                1
            )],
            "{descendant}"
        );
    }
    // 横書きは、原点を動かさない
    let src = cid_font_with(
        "MS-Gothic",
        "Identity-H",
        "/W2 [1 [-1000 400 880]]",
        content,
    );
    let (_, diag) = lift(&src);
    assert!(diag.is_empty(), "{diag:?}");
    // 見えない所で選んだフォントと、拡張グラフィックス状態の Font で選んだフォントも、見える所で描けば診断する
    for (content, resources) in [
        (
            "/OC /oc BDC BT /F2 10 Tf ET EMC BT 100 100 Td <00010002> Tj ET",
            "/Font << /F2 3 0 R >> /Properties << /oc 5 0 R >>",
        ),
        (
            "BT /G gs 100 100 Td <00010002> Tj ET",
            "/Font << /F2 3 0 R >> /ExtGState << /G << /Font [3 0 R 10] >> >>",
        ),
    ] {
        let mut document = cid_font_document("MS-Gothic", "Identity-V", "/W [1 2 500]", content);
        document.pages[0] = document.pages[0].resources(resources);
        document
            .objects
            .push(b"<< /Type /OCG /Name (Hidden) >>".to_vec());
        document.catalog = "/OCProperties << /OCGs [5 0 R] /D << /OFF [5 0 R] >> >>";
        let (drawing, diag) = lift(&document.bytes());
        assert_eq!(elements(&drawing).len(), 1, "{content}");
        assert_eq!(
            named(&diag),
            [
                ("pdf.optional-content.dropped", "Hidden".to_owned(), 1),
                (
                    "pdf.vertical-origin.approximated",
                    "MS-Gothic".to_owned(),
                    1
                ),
            ],
            "{content}"
        );
    }
}

#[test]
fn reports_letter_spacing() {
    // すべての字形の間に同じだけ足した字間も、字形ごとの位置を移さなかったことを診断する。水平の伸縮は、幅の
    // 倍率に移すので、診断しない
    for (content, expected, approximated) in [
        ("3 Tc (ABCD) Tj", vec!["ABCD"], true),
        ("-3 Tc (ABCD) Tj", vec!["ABCD"], true),
        ("[(A) 300 (B) 300 (C) 300 (D)] TJ", vec!["ABCD"], true),
        ("[(A) -300 (B) -300 (C) -300 (D)] TJ", vec!["ABCD"], true),
        ("50 Tz (ABCD) Tj", vec!["ABCD"], false),
        // 大きく詰めた所が半分あっても、詰めていない所では分けない
        ("[(A) 600 (BC)] TJ", vec!["A", "BC"], false),
        ("[(A) 600 (B) 600 (CD)] TJ", vec!["ABC", "D"], true),
    ] {
        let content = format!("BT /F1 12 Tf 72 700 Td {content} ET");
        let (drawing, diag) = lift(&one_page(Page::new(&content).resources(HELVETICA)));
        assert_eq!(contents(&drawing), expected, "{content}");
        let found = codes_of(&diag).contains(&(codes::PDF_GLYPH_POSITION_APPROXIMATED.name(), 1));
        assert_eq!(found, approximated, "{content}: {diag:?}");
    }
}

/// 3 番から、`levels` 個のオブジェクトの鎖と、最後のオブジェクト `leaf` を置く。`link(n)` は、n 番を指す
/// オブジェクト
fn chain(levels: usize, link: impl Fn(usize) -> Vec<u8>, leaf: &[u8]) -> Vec<Vec<u8>> {
    (0..levels)
        .map(|index| link(4 + index))
        .chain([leaf.to_vec()])
        .collect()
}

#[test]
fn refuses_pages_nested_too_deep_for_the_stack() {
    // hayro-interpret は、関数と色空間を深さの上限なしに再帰して作るので、16 段より深い入れ子を使うページは
    // 解釈しない。そのページの走査の診断は返さず、ほかのページは移す
    let function = |next: usize| {
        format!(
            "<< /FunctionType 3 /Domain [0 1] /Functions [{next} 0 R] /Bounds [] /Encode [0 1] >>"
        )
        .into_bytes()
    };
    let last_function = b"<< /FunctionType 2 /Domain [0 1] /C0 [0] /C1 [1] /N 1 >>";
    let indexed = |next: usize| format!("[/Indexed {next} 0 R 0 <00>]").into_bytes();
    let last_indexed = b"[/Indexed /DeviceGray 0 <00>]";
    let icc = |next: usize| {
        common::stream(
            &format!("/N 1 /Alternate [/ICCBased {next} 0 R]"),
            "not a profile",
        )
    };
    let last_icc = common::stream("/N 1 /Alternate /DeviceGray", "not a profile");
    // ストリームで書いた Type 3 の関数も、`Functions` をたどる（hayro-interpret は、辞書と同じく読む）
    let stream_function = |next: usize| {
        common::stream(
            &format!(
                "/FunctionType 3 /Domain [0 1] /Functions [{next} 0 R] /Bounds [] /Encode [0 1]"
            ),
            "",
        )
    };
    for levels in [16, 17] {
        let image = format!("/XObject << /Im {} 0 R >>", 4 + levels);
        let mut images = chain(levels, indexed, last_indexed);
        images.push(common::stream(
            "/Type /XObject /Subtype /Image /Width 1 /Height 1 /BitsPerComponent 8 \
             /ColorSpace 3 0 R",
            [0u8],
        ));
        let cases = [
            (
                "/T gs 0 0 m 10 10 l S",
                "/ExtGState << /T << /TR 3 0 R >> >>",
                chain(levels, function, last_function),
            ),
            (
                "/T gs 0 0 m 10 10 l S",
                "/ExtGState << /T << /TR 3 0 R >> >>",
                chain(levels, stream_function, last_function),
            ),
            // 直接書いた資源は、同じ資源の辞書のほかの資源の手間を使わずにたどる
            (
                "/A gs /B gs 0 0 m 10 10 l S",
                "/ExtGState << /A << /LW 1 >> /B << /TR 3 0 R >> >>",
                chain(levels, function, last_function),
            ),
            (
                "0 0 m 10 10 l S /A sh /B sh",
                "/Shading << /A << /ShadingType 2 /ColorSpace /DeviceGray /Coords [0 0 1 0] \
                 /Function << /FunctionType 2 /Domain [0 1] /C0 [0] /C1 [1] /N 1 >> >> \
                 /B << /ShadingType 2 /ColorSpace /DeviceGray /Coords [0 0 1 0] \
                 /Function 3 0 R >> >>",
                chain(levels, function, last_function),
            ),
            (
                "0 0 m 10 10 l S /Sh sh",
                "/Shading << /Sh << /ShadingType 2 /ColorSpace /DeviceGray /Coords [0 0 1 0] \
                 /Function 3 0 R >> >>",
                chain(levels, function, last_function),
            ),
            (
                "/CS0 cs 0 sc 0 0 10 10 re f",
                "/ColorSpace << /CS0 3 0 R >>",
                chain(levels, indexed, last_indexed),
            ),
            (
                "/CS0 cs 0 sc 0 0 10 10 re f",
                "/ColorSpace << /CS0 [/ICCBased 3 0 R] >>",
                chain(levels, icc, &last_icc),
            ),
            ("q 10 0 0 10 0 0 cm /Im Do Q", image.as_str(), images),
        ];
        for (content, resources, objects) in cases {
            // 1 ページ目は、資源にないフォントでも文字を描く（走査が診断する）
            let content = format!("{content} BT /F9 12 Tf (A) Tj ET");
            let mut document = Document::new(vec![
                Page::new(&content).resources(resources),
                Page::new("0 0 m 10 10 l S"),
            ]);
            document.objects = objects;
            let (drawing, diag) = lift(&document.bytes());
            assert_eq!(drawing.sheets[1].elements.len(), 1, "{resources}");
            if levels > 16 {
                assert!(drawing.sheets[0].elements.is_empty(), "{resources}");
                assert_eq!(
                    codes_of(&diag),
                    [(codes::PDF_PAGE_DROPPED.name(), 1)],
                    "{resources}"
                );
            } else {
                assert!(!drawing.sheets[0].elements.is_empty(), "{resources}");
                assert!(
                    diag.iter().all(|d| d.code() != codes::PDF_PAGE_DROPPED),
                    "{resources}: {diag:?}"
                );
            }
        }
    }
    // Type 4 の関数の手続きの入れ子は、64 段まで解釈する。コメントの中の括弧は数えない
    for (depth, dropped) in [(64, false), (65, true)] {
        let mut program = String::from("{ % {{{{ in a comment\n");
        program.push_str(&"{".repeat(depth - 1));
        program.push_str(&"}".repeat(depth - 1));
        program.push('}');
        let mut document = Document::new(vec![
            Page::new("/T gs 0 0 m 10 10 l S").resources("/ExtGState << /T << /TR 3 0 R >> >>"),
        ]);
        document.objects = vec![common::stream(
            "/FunctionType 4 /Domain [0 1] /Range [0 1]",
            program,
        )];
        let (drawing, diag) = lift(&document.bytes());
        assert_eq!(elements(&drawing).is_empty(), dropped, "{depth}");
        let found = diag.iter().any(|d| d.code() == codes::PDF_PAGE_DROPPED);
        assert_eq!(found, dropped, "{depth}: {diag:?}");
    }
}

#[test]
fn refuses_pages_with_too_many_widths() {
    // hayro-font は、W・W2 の範囲の CID を 1 つずつ表に入れるので、項目が 2 の 18 乗より多いフォントを使う
    // ページは、フォントを読む前に、解釈しないことにする
    let content = "BT /F2 10 Tf 100 100 Td <00010002> Tj ET";
    for descendant in [
        "/W [0 262144 500]",
        "/W [0 [500] 1 262144 500]",
        "/W [0 4294967295 500]",
        "/W2 [0 4294967295 -1000 500 880]",
    ] {
        let src = cid_font_with("MS-Gothic", "Identity-H", descendant, content);
        let (drawing, diag) = lift(&src);
        assert!(elements(&drawing).is_empty(), "{descendant}");
        assert_eq!(
            codes_of(&diag),
            [(codes::PDF_PAGE_DROPPED.name(), 1)],
            "{descendant}"
        );
    }
    let src = cid_font_with("MS-Gothic", "Identity-H", "/W [0 262143 500]", content);
    let (drawing, diag) = lift(&src);
    assert_eq!(elements(&drawing).len(), 1);
    assert!(diag.iter().all(|d| d.code() != codes::PDF_PAGE_DROPPED));

    // 直接書いたフォントは、同じ資源の辞書のほかのフォントの項目の数を使わずに数える。3 番は使わない
    let fonts = format!(
        "/Font << /F1 {} /F2 {} >>",
        cid_font("MS-Gothic", "Identity-H", ""),
        cid_font("MS-Gothic", "Identity-H", "/W [0 262144 500]")
    );
    let mut document = Document::new(vec![
        Page::new("BT /F1 10 Tf 100 100 Td <0001> Tj /F2 10 Tf <0002> Tj ET").resources(&fonts),
    ]);
    document.objects = vec![b"<< >>".to_vec(), cid_to_unicode()];
    let (drawing, diag) = lift(&document.bytes());
    assert!(elements(&drawing).is_empty());
    assert_eq!(codes_of(&diag), [(codes::PDF_PAGE_DROPPED.name(), 1)]);

    // 幅の表のメモリ（1 項目を 160 バイト）は、文書の中で 1 度だけ数える。走査と hayro-interpret は、文書の間、
    // フォントごとに表を持つ
    let font = cid_font("MS-Gothic", "Identity-H", "/W [0 99999 500]");
    let mut bytes = Vec::new();
    for pages in [1, 2] {
        let page = Page::new(content).resources("/Font << /F2 3 0 R >>");
        let mut document = Document::new(vec![page; pages]);
        document.objects = vec![font.clone().into_bytes(), cid_to_unicode()];
        bytes.push(used(&document.bytes(), zumen_base::Kind::DecodedBytes));
    }
    assert!(bytes[0] >= 100_000 * 160, "{bytes:?}");
    assert!(bytes[1] < bytes[0] + 100_000 * 160, "{bytes:?}");
}

#[test]
fn refuses_pages_with_sampled_functions_too_large() {
    // hayro-interpret は、Type 0 の関数の標本の位置を f32 で求めるので、大きさが 2 の 24 乗を超えると、丸めた
    // 位置が標本の外を指してパニックする。その関数を使うページは解釈しない
    for (size, dropped) in [(16_777_216u32, false), (16_777_217, true)] {
        let mut document = Document::new(vec![
            Page::new("/T gs 0 0 m 10 10 l S").resources("/ExtGState << /T << /TR 3 0 R >> >>"),
        ]);
        document.objects = vec![common::stream(
            &format!("/FunctionType 0 /Domain [0 1] /Range [0 1] /Size [{size}] /BitsPerSample 1"),
            [0u8],
        )];
        let (drawing, diag) = lift(&document.bytes());
        assert_eq!(elements(&drawing).is_empty(), dropped, "{size}");
        let found = diag.iter().any(|d| d.code() == codes::PDF_PAGE_DROPPED);
        assert_eq!(found, dropped, "{size}: {diag:?}");
    }
}

#[test]
fn refuses_pages_with_empty_lattice_rows() {
    // hayro-interpret は、格子のメッシュ（型 5）の行の頂点の数が 0 だと、頂点を読まずに行を足し続け、メモリを
    // 使い切る。そのメッシュを `sh` かパターンで使うページは解釈しない
    for (vertices, dropped) in [(1, false), (0, true)] {
        let mesh = common::stream(
            &format!(
                "/ShadingType 5 /ColorSpace /DeviceGray /BitsPerCoordinate 8 /BitsPerComponent 8 \
                 /VerticesPerRow {vertices} /Decode [0 100 0 100 0 1]"
            ),
            [0u8, 0, 0, 100, 0, 0],
        );
        for (content, resources) in [
            ("0 0 m 10 10 l S /Sh sh", "/Shading << /Sh 3 0 R >>"),
            (
                "0 0 m 10 10 l S /Pattern cs /P scn 0 0 10 10 re f",
                "/Pattern << /P << /PatternType 2 /Shading 3 0 R >> >>",
            ),
        ] {
            let mut document = Document::new(vec![Page::new(content).resources(resources)]);
            document.objects = vec![mesh.clone()];
            let (drawing, diag) = lift(&document.bytes());
            assert_eq!(
                elements(&drawing).is_empty(),
                dropped,
                "{vertices} {content}"
            );
            let found = diag.iter().any(|d| d.code() == codes::PDF_PAGE_DROPPED);
            assert_eq!(found, dropped, "{vertices} {content}: {diag:?}");
        }
    }
}

/// 既定の予算で移し、使った量を返す
#[cfg(test)]
fn used(src: &[u8], kind: zumen_base::Kind) -> u64 {
    let file = testkit::read::<Pdf>(src).0.unwrap();
    let mut budget = Budget::default();
    let mut diag = Diagnostics::new();
    let cancel = CancelToken::new();
    file.lift(&mut Cx::new(&mut budget, &mut diag, &cancel))
        .unwrap();
    budget.used(kind)
}

#[test]
fn counts_work_that_grows_with_sizes() {
    use zumen_base::Kind;
    // hayro-interpret は、線と文字を描くたびに破線の配列を写すので、描くたびに長さを数える
    let dash = format!("[{}] 0 d ", "1 ".repeat(1_000));
    let strokes = format!("{dash}{}", "0 0 m 10 10 l S ".repeat(100));
    assert!(used(&one_page(Page::new(&strokes)), Kind::Work) >= 100 * 1_000);
    let texts = format!("{dash}BT {} ET", "(A) Tj ".repeat(100));
    assert!(used(&one_page(Page::new(&texts)), Kind::Work) >= 100 * 1_000);
    // 1 回の線の部分経路では、線種を 1 度だけ作る
    let subpaths = format!("{dash}{}S", "0 0 m 10 10 l ".repeat(100));
    let work = used(&one_page(Page::new(&subpaths)), Kind::Work);
    assert!(work < 10 * 1_000, "{work}");
    // 切り抜きにする文字は、1 バイトを 40 と数える
    let clip = format!("BT 7 Tr {} ET", "(AAAAAAAAAA) Tj ".repeat(10));
    assert!(used(&one_page(Page::new(&clip)), Kind::Work) >= 40 * 100);

    // hayro-syntax は、資源を引くたびに辞書を読み直すので、引いたオブジェクトの大きさ（4 バイトを 1）を数える
    let pad = format!("/Pad [{}]", "0 ".repeat(400));
    let lookups = [
        (
            "/G gs ",
            "/ExtGState << /G 3 0 R >>",
            format!("<< /LW 1 {pad} >>").into_bytes(),
        ),
        (
            "/F Do ",
            "/XObject << /F 3 0 R >>",
            common::stream(
                &format!("/Type /XObject /Subtype /Form /BBox [0 0 1 1] {pad}"),
                "",
            ),
        ),
        (
            "/CS0 cs ",
            "/ColorSpace << /CS0 3 0 R >>",
            format!(
                "[/Separation /Spot /DeviceGray \
                 << /FunctionType 2 /Domain [0 1] /C0 [0] /C1 [1] /N 1 {pad} >>]"
            )
            .into_bytes(),
        ),
        (
            "BT /F1 10 Tf ET ",
            "/Font << /F1 3 0 R >>",
            format!("<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica {pad} >>").into_bytes(),
        ),
        (
            "/OC /oc BDC EMC ",
            "/Properties << /oc 3 0 R >>",
            format!("<< /Type /OCG /Name (Shown) {pad} >>").into_bytes(),
        ),
    ];
    for (op, resources, object) in lookups {
        let content = op.repeat(100);
        let mut document = Document::new(vec![Page::new(&content).resources(resources)]);
        document.objects = vec![object];
        let work = used(&document.bytes(), Kind::Work);
        assert!(work >= 100 * 200, "{op}: {work}");
    }
    // hayro-interpret は、マーク付きの内容の名前の性質の辞書を、タグによらず読み直す
    let marks = "/Span /P BDC EMC ".repeat(100);
    let properties = format!("/Properties << /P << /MCID 0 {pad} >> >>");
    let src = one_page(Page::new(&marks).resources(&properties));
    assert!(used(&src, Kind::Work) >= 100 * 200);
    // OCMD の `OCGs`（参照の先の配列）は、hayro-interpret・走査・装置が、マーク付きの内容のたびに読む
    let marks = "/OC /m BDC EMC ".repeat(100);
    let mut document = Document::new(vec![
        Page::new(&marks).resources("/Properties << /m 3 0 R >>"),
    ]);
    document.objects = vec![
        b"<< /Type /OCMD /OCGs 4 0 R >>".to_vec(),
        format!("[{}]", "5 0 R ".repeat(300)).into_bytes(),
        b"<< /Type /OCG /Name (Shown) >>".to_vec(),
    ];
    assert!(used(&document.bytes(), Kind::Work) >= 100 * 3 * 450);
    // シェーディングの関数は、`sh` のたびに作り直すので、参照の先の関数の辞書も読み直す
    let shadings = "/Sh sh ".repeat(100);
    let mut document = Document::new(vec![
        Page::new(&shadings).resources("/Shading << /Sh 3 0 R >>"),
    ]);
    document.objects = vec![
        b"<< /ShadingType 2 /ColorSpace /DeviceGray /Coords [0 0 1 0] /Function 4 0 R >>".to_vec(),
        format!("<< /FunctionType 2 /Domain [0 1] /C0 [0] /C1 [1] /N 1 {pad} >>").into_bytes(),
    ];
    assert!(used(&document.bytes(), Kind::Work) >= 100 * 200);
    // 関数の辞書の数の配列（`C0` など）は、作るたびに項目ごとに数にするので、項目の数も数える（2 バイトの `0 ` の
    // 並びは、バイト数で数える手間より重い）
    let shadings = "/Sh sh ".repeat(10);
    let mut document = Document::new(vec![
        Page::new(&shadings).resources("/Shading << /Sh 3 0 R >>"),
    ]);
    document.objects = vec![
        b"<< /ShadingType 2 /ColorSpace /DeviceGray /Coords [0 0 1 0] /Function 4 0 R >>".to_vec(),
        format!(
            "<< /FunctionType 2 /Domain [0 1] /C0 [{}] /C1 [1] /N 1 >>",
            "0 ".repeat(100_000)
        )
        .into_bytes(),
    ];
    assert!(used(&document.bytes(), Kind::Work) >= 10 * 100_000 * 3 / 2);

    // メッシュのシェーディングは、三角形とパッチをすべて作るので、頂点のビット数から数の上限を求め、頂点か
    // パッチの 1 つを 4 と、三角形の 1 つを 1,024 バイトと数える。型 4 の 8 ビットの頂点は 4 バイトで、型 5 の頂点は
    // バイトの境から読み、三角形を 2 つまで作る。`Function` がなく、`Decode` が座標の分だけのメッシュは、色を
    // 読まないので、成分のビット数によらず同じ数を数える
    for (mesh, count, bytes) in [
        (
            "/ShadingType 4 /BitsPerCoordinate 8 /BitsPerComponent 8 /BitsPerFlag 8 \
             /Decode [0 1 0 1 0 1]",
            1000 / 4,
            1000 / 4 * 1024,
        ),
        (
            "/ShadingType 5 /BitsPerCoordinate 1 /BitsPerComponent 1 /VerticesPerRow 2 \
             /Decode [0 1 0 1 0 1]",
            1000,
            1000 * (2 * 1024 + 256),
        ),
        (
            "/ShadingType 5 /BitsPerCoordinate 1 /BitsPerComponent 255 /VerticesPerRow 2 \
             /Decode [0 1 0 1]",
            1000,
            1000 * (2 * 1024 + 256),
        ),
        (
            "/ShadingType 6 /BitsPerCoordinate 1 /BitsPerComponent 255 /BitsPerFlag 2 \
             /Decode [0 1 0 1]",
            1000 * 8 / 18,
            1000 * 8 / 18 * 2048,
        ),
    ] {
        let mut document = Document::new(vec![
            Page::new("/Sh sh").resources("/Shading << /Sh 3 0 R >>"),
        ]);
        document.objects = vec![common::stream(
            &format!("{mesh} /ColorSpace /DeviceGray"),
            vec![0u8; 1000],
        )];
        let src = document.bytes();
        let work = used(&src, Kind::Work);
        assert!(work >= 4 * count, "{mesh}: {work}");
        let used = used(&src, Kind::DecodedBytes);
        assert!(used >= 1000 + bytes, "{mesh}: {used}");
    }
    // 幅が欠けるか u8 に収まらないメッシュと、座標か旗の幅が 1〜32 でないメッシュは、hayro-interpret が三角形も
    // パッチも作らないので、数えない
    for mesh in [
        "/ShadingType 5 /BitsPerComponent 8 /VerticesPerRow 2",
        "/ShadingType 5 /BitsPerCoordinate 0 /BitsPerComponent 8 /VerticesPerRow 2",
        "/ShadingType 5 /BitsPerCoordinate 8 /VerticesPerRow 2",
        "/ShadingType 4 /BitsPerCoordinate 40 /BitsPerComponent 8 /BitsPerFlag 8",
        "/ShadingType 6 /BitsPerCoordinate 1 /BitsPerComponent 8",
        "/ShadingType 7 /BitsPerCoordinate 1 /BitsPerComponent 8 /BitsPerFlag 0",
    ] {
        let mut document = Document::new(vec![
            Page::new("/Sh sh").resources("/Shading << /Sh 3 0 R >>"),
        ]);
        document.objects = vec![common::stream(
            &format!("{mesh} /ColorSpace /DeviceGray /Decode [0 1 0 1]"),
            vec![0u8; 100_000],
        )];
        let used = used(&document.bytes(), Kind::DecodedBytes);
        assert!(used < 1_000_000, "{mesh}: {used}");
    }
    // 経路を塗るか線を描く演算子は、装置の手間（経路の要素ごとに 1 と、経路ごとに 1）に 6 を足す。`B` は、塗りと
    // 線を別に描く
    let fills = "0 0 10 10 re f ".repeat(100);
    assert!(used(&one_page(Page::new(&fills)), Kind::Work) >= 100 * (2 + 6 + 6));
    let both = "0 0 10 10 re B ".repeat(100);
    assert!(used(&one_page(Page::new(&both)), Kind::Work) >= 100 * (2 + 2 * 6 + 2 * 6));

    // Type 0 の関数は、作るたびに標本をすべて読んで f32 にして持つ。1 ビットの標本 8 万個を、読む手間を 8 個で 1、
    // メモリを配列を伸ばす間の写しを含めて 128 万バイトと数える
    let mut document = Document::new(vec![
        Page::new("/T gs").resources("/ExtGState << /T << /TR 3 0 R >> >>"),
    ]);
    document.objects = vec![common::stream(
        "/FunctionType 0 /Domain [0 1] /Range [0 1] /Size [80000] /BitsPerSample 1",
        vec![0u8; 10_000],
    )];
    let src = document.bytes();
    assert!(used(&src, Kind::Work) >= 80_000 / 8);
    assert!(used(&src, Kind::DecodedBytes) >= 10_000 + 80_000 * 16);

    // 多成分の DeviceN の色は、塗るたびに色変換の関数を当てる。8 入力の Type 0 の関数は、出力ごとに、端の標本
    // 2 の 8 乗個を足す
    let tint = common::stream(
        "/FunctionType 0 /Domain [0 1 0 1 0 1 0 1 0 1 0 1 0 1 0 1] /Range [0 1 0 1 0 1] \
         /Size [2 2 2 2 2 2 2 2] /BitsPerSample 8",
        vec![0u8; 3 * 256],
    );
    let mut work = Vec::new();
    for fills in [10, 20] {
        let content = format!(
            "/CS0 cs 0.5 0.5 0.5 0.5 0.5 0.5 0.5 0.5 scn {}",
            "0 0 10 10 re f ".repeat(fills)
        );
        let mut document = Document::new(vec![Page::new(&content).resources(
            "/ColorSpace << /CS0 [/DeviceN [/a /b /c /d /e /f /g /h] /DeviceRGB 3 0 R] >>",
        )]);
        document.objects = vec![tint.clone()];
        work.push(used(&document.bytes(), Kind::Work));
    }
    assert!(work[1] >= work[0] + 10 * 256 * 3, "{work:?}");

    // TR と TR2 を両方持つ状態は、関数を 2 回作る。転送関数は、状態を当てるたびに 256 個の値に当てて表を作り、
    // Type 4 の関数は、1 度当てるたびに手続きを読むと数える
    let mut program = String::from("{ ");
    program.push_str(&" ".repeat(10_000));
    program.push('}');
    let function = common::stream("/FunctionType 4 /Domain [0 1] /Range [0 1]", program);
    let states = "/T gs ".repeat(10);
    let mut bytes = Vec::new();
    for state in ["/TR 3 0 R", "/TR 3 0 R /TR2 3 0 R"] {
        let resources = format!("/ExtGState << /T << {state} >> >>");
        let mut document = Document::new(vec![Page::new(&states).resources(&resources)]);
        document.objects = vec![function.clone()];
        let src = document.bytes();
        assert!(used(&src, Kind::Work) >= 10 * 256 * 10_000, "{state}");
        bytes.push(used(&src, Kind::DecodedBytes));
    }
    assert!(bytes[1] >= bytes[0] + 10 * 10_000, "{bytes:?}");
    // Type 4 の関数は、作るたびに手続きを読み（4 バイトを 1）、演算子の配列にする（1 バイトを 128 バイト）。
    // シェーディングの関数は、表を作らない
    let shadings = "/Sh sh ".repeat(10);
    let mut document = Document::new(vec![Page::new(&shadings).resources(
        "/Shading << /Sh << /ShadingType 2 /ColorSpace /DeviceGray /Coords [0 0 1 0] \
         /Function 3 0 R >> >>",
    )]);
    document.objects = vec![function];
    let src = document.bytes();
    assert!(used(&src, Kind::Work) >= 10 * 10_000 / 4);
    assert!(used(&src, Kind::DecodedBytes) >= 10 * 10_000 * 128);
}

#[test]
fn counts_copies_of_the_graphics_state() {
    use zumen_base::Kind;
    // hayro-interpret は、`q` の後で状態を変えるとき・フォームを呼ぶとき・Type 3 の字形ごとに、グラフィックス
    // 状態を写す。写すたびに、破線の配列と、切り抜きに貯めた文字の輪郭も写す
    let dash = format!("[{}] 0 d ", "1 ".repeat(1_000));
    let saves = format!("{dash}{}", "q 1 w ".repeat(100));
    let src = one_page(Page::new(&saves));
    assert!(used(&src, Kind::Work) >= 100 * 1_000);
    assert!(used(&src, Kind::DecodedBytes) >= 100 * 1_000 * 8);
    // 切り抜きに貯めた 1,000 字の輪郭を、`q` のたびに写す（Helvetica の A の輪郭は、経路の要素が 4 つより多く、
    // 写すときは要素の 1 つを 128 バイトと数える）
    let clip = |saves: usize| {
        format!(
            "BT /F1 12 Tf 7 Tr ({}) Tj {}ET",
            "A".repeat(1_000),
            "q 0 Tr Q ".repeat(saves)
        )
    };
    let copied = |saves: usize| {
        used(
            &one_page(Page::new(&clip(saves)).resources(HELVETICA)),
            Kind::DecodedBytes,
        )
    };
    assert!(copied(10) >= copied(0) + 10 * 1_000 * 4 * 128);

    let calls = format!("{dash}{}", "/X Do ".repeat(100));
    let mut document = Document::new(vec![Page::new(&calls).resources("/XObject << /X 3 0 R >>")]);
    document.objects = vec![common::stream(
        "/Type /XObject /Subtype /Form /BBox [0 0 1 1]",
        "",
    )];
    assert!(used(&document.bytes(), Kind::Work) >= 100 * 1_000);

    let glyphs = format!("{dash}BT /T3 12 Tf ({}) Tj ET", "a".repeat(100));
    let mut document = Document::new(vec![Page::new(&glyphs).resources("/Font << /T3 3 0 R >>")]);
    document.objects = vec![
        b"<< /Type /Font /Subtype /Type3 /FontBBox [0 0 1000 1000] /FontMatrix [0.001 0 0 0.001 0 0] \
          /CharProcs << /a 4 0 R >> /Encoding << /Type /Encoding /Differences [97 /a] >> \
          /FirstChar 97 /LastChar 97 /Widths [1000] >>"
            .to_vec(),
        common::stream("", "1000 0 0 0 1000 1000 d1 0 0 1000 1000 re f"),
    ];
    assert!(used(&document.bytes(), Kind::Work) >= 100 * 1_000);
}

/// 辞書の値に直接書くストリーム（7.3.8.1 に反するが、hayro-syntax は読む）。hayro-syntax は、コンテナーの辞書を
/// 読むとき、ストリームの中身も字句として読み飛ばすので、区切りの文字（`>`・`{` など）と、数字の後に文字が続く
/// 字句を書けない。中身は ASCIIHexDecode の 16 進の数字を 1 つずつ空白で分けて書き、終わりの `>` は省く
/// （hayro-syntax は、なくても読む）
fn direct_stream(dict: &str, data: &[u8]) -> String {
    let digits: String = data
        .iter()
        .map(|byte| format!("{:X} {:X} ", byte >> 4, byte & 0xF))
        .collect();
    format!(
        "<< /Length {} /Filter /ASCIIHexDecode {dict} >>\nstream\n{digits}\nendstream",
        digits.len()
    )
}

fn be16(out: &mut Vec<u8>, value: u16) {
    out.extend(value.to_be_bytes());
}

fn be32(out: &mut Vec<u8>, value: u32) {
    out.extend(value.to_be_bytes());
}

/// `glyphs` を字形 0 から並べた、最小の TrueType のフォント（glyf・head・hhea・hmtx・loca・maxp）。loca は長い形
fn truetype(glyphs: &[Vec<u8>]) -> Vec<u8> {
    truetype_with(glyphs, None, Vec::new())
}

/// [`truetype`] に表 `extra` を足したフォント。`limits` があれば、`maxp` をバージョン 1.0 にして、影の領域の点・記憶域・
/// 関数の定義・命令の定義・スタックの項目の数を書く
fn truetype_with(
    glyphs: &[Vec<u8>],
    limits: Option<[u16; 5]>,
    extra: Vec<(&[u8; 4], Vec<u8>)>,
) -> Vec<u8> {
    let mut glyf = Vec::new();
    let mut loca = Vec::new();
    for glyph in glyphs {
        be32(&mut loca, glyf.len() as u32);
        glyf.extend(glyph);
        while glyf.len() % 4 != 0 {
            glyf.push(0);
        }
    }
    be32(&mut loca, glyf.len() as u32);
    let count = glyphs.len() as u16;
    let mut head = Vec::new();
    for value in [0x0001_0000, 0x0001_0000, 0, 0x5F0F_3CF5] {
        be32(&mut head, value);
    }
    // 旗・em の大きさ・作った日と変えた日・外接矩形・様式・最小の大きさ・向き・loca の形（長い）・字形の形
    for value in [
        0, 1000, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 100, 100, 0, 8, 2, 1, 0,
    ] {
        be16(&mut head, value);
    }
    let mut maxp = Vec::new();
    match limits {
        None => {
            be32(&mut maxp, 0x0000_5000);
            be16(&mut maxp, count);
        }
        Some([twilight, storage, functions, instructions, stack]) => {
            be32(&mut maxp, 0x0001_0000);
            for value in [
                count,
                100,
                10,
                100,
                10,
                2,
                twilight,
                storage,
                functions,
                instructions,
                stack,
                1000,
                10,
                2,
            ] {
                be16(&mut maxp, value);
            }
        }
    }
    let mut hhea = Vec::new();
    be32(&mut hhea, 0x0001_0000);
    for value in [
        800, 0xFF38, 0, 1000, 0, 0, 100, 1, 0, 0, 0, 0, 0, 0, 0, count,
    ] {
        be16(&mut hhea, value);
    }
    let mut hmtx = Vec::new();
    for _ in 0..count {
        be16(&mut hmtx, 1000);
        be16(&mut hmtx, 0);
    }
    let mut tables = vec![
        (b"glyf", glyf),
        (b"head", head),
        (b"hhea", hhea),
        (b"hmtx", hmtx),
        (b"loca", loca),
        (b"maxp", maxp),
    ];
    tables.extend(extra);
    // 表の目録は、タグの順に並べる
    tables.sort_by_key(|(tag, _)| **tag);
    let mut font = Vec::new();
    be32(&mut font, 0x0001_0000);
    for value in [tables.len() as u16, 64, 2, 32] {
        be16(&mut font, value);
    }
    let mut offset = 12 + 16 * tables.len();
    let mut body = Vec::new();
    for (tag, mut data) in tables {
        font.extend(tag);
        be32(&mut font, 0);
        be32(&mut font, offset as u32);
        be32(&mut font, data.len() as u32);
        while data.len() % 4 != 0 {
            data.push(0);
        }
        offset += data.len();
        body.extend(data);
    }
    font.extend(body);
    font
}

/// 字形 1 に、`points` 個の点を同じ所に置いた輪郭を 1 つ持つ、最小の TrueType のフォント。点は、旗の繰り返しと
/// 「前と同じ座標」で書くので、座標のバイトを持たない
fn outline_font(points: u16) -> Vec<u8> {
    truetype(&[Vec::new(), outline_glyph(points)])
}

/// [`outline_font`] の字形
fn outline_glyph(points: u16) -> Vec<u8> {
    let mut glyph = Vec::new();
    // 輪郭の数・外接矩形・最後の点・命令の長さ
    for value in [1, 0, 0, 100, 100, points - 1, 0] {
        be16(&mut glyph, value);
    }
    // 曲線の上・x も y も前と同じ、を 256 点ずつ繰り返す
    let mut left = usize::from(points);
    while left > 0 {
        let run = left.min(256);
        glyph.extend([0x01 | 0x08 | 0x10 | 0x20, (run - 1) as u8]);
        left -= run;
    }
    glyph
}

/// 字形の輪郭を TrueType の命令に頼るフォント（`is_hint_reliant`）の形をしたフォント。ファミリーの名前（`name` の
/// 1 番）を `family`、`fpgm` と `prep` を `program`、影の領域の点を 65,535 にする
fn hinted_font(family: &str, program: &[u8]) -> Vec<u8> {
    let text: Vec<u8> = family.encode_utf16().flat_map(u16::to_be_bytes).collect();
    let mut name = Vec::new();
    // 形式・数・文字列の位置と、Windows の英語のファミリーの名前の項目
    for value in [0, 1, 18, 3, 1, 0x409, 1, text.len() as u16, 0] {
        be16(&mut name, value);
    }
    name.extend(&text);
    truetype_with(
        &[Vec::new(), outline_glyph(3)],
        Some([65_535, 0, 0, 0, 100]),
        vec![
            (b"fpgm", program.to_vec()),
            (b"name", name),
            (b"prep", program.to_vec()),
        ],
    )
}

/// 字形 1 を覆う、1 バイトの字形を持つ `VARC` の表（variable composites のバージョン 1.0）を足したフォント
fn varc_font() -> Vec<u8> {
    let mut varc = Vec::new();
    // バージョンと、覆う字形・変化の値・条件・軸の並び・字形の位置
    for value in [0x0001_0000, 24, 0, 0, 0, 30] {
        be32(&mut varc, value);
    }
    // 覆う字形の形式 1
    for value in [1, 1, 1] {
        be16(&mut varc, value);
    }
    // 字形の INDEX（数・位置のバイト数・位置・中身）
    be32(&mut varc, 1);
    varc.extend([1, 1, 2, 0]);
    truetype_with(&[Vec::new(), outline_glyph(3)], None, vec![(b"VARC", varc)])
}

/// [`outline_font`] の字形 1 を `count` 字、描画モード `mode` で描くページ。フォントは Identity-H の Type0 の
/// CIDFontType2 で、CID をそのまま字形の番号にする
fn outline_document(points: u16, count: usize, mode: u32) -> Vec<u8> {
    font_document(
        "/CIDFontType2",
        "/FontFile2",
        outline_font(points),
        count,
        mode,
    )
}

#[test]
fn counts_outlines_of_clipping_text() {
    use zumen_base::Kind;
    // hayro-interpret は、切り抜きにする文字（描画モード 4〜7）の字ごとに、フォントのプログラムから輪郭を作り、
    // `ET` まで貯める。輪郭の大きさは字形で決まるので、走査も輪郭を作り、経路の要素の 1 つを、作る手間 3 と
    // 512 バイトと数える。Helvetica の A の輪郭は、要素が 4 つより多い
    let text = |mode: u32| format!("BT /F1 12 Tf {mode} Tr ({}) Tj ET", "A".repeat(1_000));
    let bytes = |mode: u32| {
        used(
            &one_page(Page::new(&text(mode)).resources(HELVETICA)),
            Kind::DecodedBytes,
        )
    };
    assert!(bytes(7) >= bytes(0) + 1_000 * 4 * 512);
    // 埋め込んだ TrueType の字形は、65,536 点まで持てる。6 万点の字形も、数百バイトのフォントで書ける
    let src = outline_document(60_000, 10, 7);
    assert!(used(&src, Kind::DecodedBytes) >= 10 * 60_000 * 512);
    assert!(used(&src, Kind::Work) >= 10 * 60_000 * 3);
    // 塗るだけの文字は、輪郭を作らない
    assert!(used(&outline_document(60_000, 10, 0), Kind::DecodedBytes) < 1_000_000);
    // 100 字は、解釈の前に予算で止める（hayro-interpret は、およそ 1.3 GB を使う）
    let file = testkit::read::<Pdf>(&outline_document(60_000, 100, 7))
        .0
        .unwrap();
    let mut budget = Budget::default();
    let mut diag = Diagnostics::new();
    let cancel = CancelToken::new();
    let result = file.lift(&mut Cx::new(&mut budget, &mut diag, &cancel));
    assert!(matches!(result, Err(Error::Stopped(_))), "{result:?}");
    assert!(
        diag.iter()
            .any(|d| d.code() == codes::BUDGET_DECODED_BYTES_EXCEEDED),
        "{diag:?}"
    );
}

#[test]
fn walks_streams_written_in_dictionaries_each_time() {
    use zumen_base::Kind;
    // hayro-syntax は、辞書の値や配列の項目に直接書いたストリームにも、コンテナーの間接オブジェクトの番号を返す。
    // 走査は、それ自身の間接オブジェクトのストリームだけを覚え、直接書いたストリームは使うたびにたどる。1 つ目の
    // 手間と中身を 2 つ目に使うと、2 つ目の中の、ページを解釈しない理由を見落とす
    let lattice = |rows: u32| {
        direct_stream(
            &format!(
                "/ShadingType 5 /ColorSpace /DeviceGray /BitsPerCoordinate 8 /BitsPerComponent 8 \
                 /VerticesPerRow {rows} /Decode [0 100 0 100 0 1]"
            ),
            &[0, 0, 0, 100, 0, 0],
        )
    };
    let shadings = format!("/Shading << /A {} /B {} >>", lattice(1), lattice(0));
    let program = |code: &str| {
        direct_stream(
            "/FunctionType 4 /Domain [0 1] /Range [0 1]",
            code.as_bytes(),
        )
    };
    let deep = format!("{}{}", "{ ".repeat(65), "} ".repeat(65));
    let functions = format!(
        "/Shading << /S << /ShadingType 2 /ColorSpace /DeviceGray /Coords [0 0 1 0] \
         /Function [{} {}] >> >>",
        program("{ }"),
        program(&deep)
    );
    // 18 重の Indexed の色空間（いちばん内の Indexed の深さは 17）
    let mut space = String::from("/DeviceGray");
    for _ in 0..18 {
        space = format!("[/Indexed {space} 0 <00>]");
    }
    let image = |space: &str| {
        direct_stream(
            &format!(
                "/Type /XObject /Subtype /Image /Width 1 /Height 1 /BitsPerComponent 8 \
                 /ColorSpace {space}"
            ),
            &[0],
        )
    };
    let images = format!(
        "/XObject << /A {} /B {} >>",
        image("/DeviceGray"),
        image(&space)
    );
    // 2 つ目のフォームは、行の頂点の数が 0 の格子（3 0 R）を塗る
    let form = |content: &str| {
        direct_stream(
            "/Type /XObject /Subtype /Form /BBox [0 0 10 10] \
             /Resources << /Shading << /Sh 3 0 R >> >>",
            content.as_bytes(),
        )
    };
    let forms = format!(
        "/XObject << /A {} /B {} >>",
        form("0 0 m 1 1 l S"),
        form("/Sh sh")
    );
    let empty = common::stream(
        "/ShadingType 5 /ColorSpace /DeviceGray /BitsPerCoordinate 8 /BitsPerComponent 8 \
         /VerticesPerRow 0 /Decode [0 100 0 100 0 1]",
        [0u8, 0, 0, 100, 0, 0],
    );
    for (content, resources) in [
        ("0 0 m 10 10 l S /A sh /B sh", shadings.as_str()),
        ("0 0 m 10 10 l S /S sh", functions.as_str()),
        ("0 0 m 10 10 l S /A Do /B Do", images.as_str()),
        ("0 0 m 10 10 l S /A Do /B Do", forms.as_str()),
    ] {
        let mut document = Document::new(vec![Page::new(content).resources(resources)]);
        document.objects = vec![empty.clone()];
        let (drawing, diag) = lift(&document.bytes());
        assert!(elements(&drawing).is_empty(), "{resources}");
        assert!(
            diag.iter().any(|d| d.code() == codes::PDF_PAGE_DROPPED),
            "{resources}: {diag:?}"
        );
    }
    // 直接書いたメッシュは、使うたびに三角形を作る手間を数える（3 頂点の後の 2 万頂点）
    let mesh = |vertices: usize| {
        direct_stream(
            "/ShadingType 4 /ColorSpace /DeviceGray /BitsPerCoordinate 8 /BitsPerComponent 8 \
             /BitsPerFlag 8 /Decode [0 100 0 100 0 1]",
            &vec![0; 4 * vertices],
        )
    };
    let meshes = format!("/Shading << /A {} /B {} >>", mesh(3), mesh(20_000));
    let paints = format!("/A sh {}", "/B sh ".repeat(20));
    let src = one_page(Page::new(&paints).resources(&meshes));
    assert!(used(&src, Kind::Work) >= 20 * 20_000 * 4);
}

#[test]
fn counts_memory_of_the_drawing_model() {
    use zumen_base::{Kind, Limit, Limits};
    // hayro-interpret は、フォームを呼ぶたびに中身を解釈し直すので、次の段を 2 回ずつ呼ぶフォームの鎖は、小さい
    // ファイルから要素を指数的に増やす。図面モデルのメモリも、要素の 1 つを 2,048 バイト、字形の Unicode の
    // 1 バイトを 4 バイトと数える（形の配列は `counts_memory_of_curves`）
    let chain = |levels: usize, leaf: &str, resources: &str| {
        let mut document = Document::new(vec![
            Page::new("/N Do").resources("/XObject << /N 3 0 R >>"),
        ]);
        document.objects = (0..levels)
            .map(|index| {
                common::stream(
                    &format!(
                        "/Type /XObject /Subtype /Form /BBox [0 0 100 100] \
                         /Resources << /XObject << /N {} 0 R >> >>",
                        4 + index
                    ),
                    "/N Do /N Do",
                )
            })
            .chain([common::stream(
                &format!(
                    "/Type /XObject /Subtype /Form /BBox [0 0 100 100] /Resources << {resources} >>"
                ),
                leaf,
            )])
            .collect();
        document.bytes()
    };
    // 1,024 本の線
    let lines = chain(10, "0 0 m 10 10 l S", "");
    assert!(used(&lines, Kind::DecodedBytes) >= 1_024 * 2_048);
    // 1,000 字の文字列の 64 回
    let text = format!("BT /F1 12 Tf 10 10 Td ({}) Tj ET", "A".repeat(1_000));
    let texts = chain(6, &text, HELVETICA);
    assert!(used(&texts, Kind::DecodedBytes) >= 64 * (2_048 + 1_000 * 4));
    // 予算を超えたら、要素を足さずに止める
    let file = testkit::read::<Pdf>(&lines).0.unwrap();
    let mut budget =
        Budget::new(Limits::default().with(Kind::DecodedBytes, Limit::Finite(1 << 20)));
    let mut diag = Diagnostics::new();
    let cancel = CancelToken::new();
    let result = file.lift(&mut Cx::new(&mut budget, &mut diag, &cancel));
    assert!(matches!(result, Err(Error::Stopped(_))), "{result:?}");
    assert!(
        diag.iter()
            .any(|d| d.code() == codes::BUDGET_DECODED_BYTES_EXCEEDED),
        "{diag:?}"
    );
}

#[test]
fn counts_memory_of_long_paths_and_glyph_runs() {
    use zumen_base::Kind;
    // hayro-interpret は、経路を塗る演算子まで配列に持ち、字形を文字列ごとに 1 つの並びの配列にする。どちらの
    // 配列も、ページの中で使い回すので、文書の中でいちばん長いものの、65,536 を超える要素を 1 つ 512 バイトと数える
    let long = format!("0 0 m {}S ", "1 1 l 0 0 l ".repeat(50_000));
    let once = used(&one_page(Page::new(&long)), Kind::DecodedBytes);
    assert!(once >= (100_000 - 65_536) * 512, "{once}");
    // 同じ長さの経路は、同じページでも別のページでも、もう 1 度は数えない
    let repeated = long.repeat(2);
    let twice = [
        one_page(Page::new(&repeated)),
        Document::new(vec![Page::new(&long), Page::new(&long)]).bytes(),
    ];
    for src in twice {
        let total = used(&src, Kind::DecodedBytes);
        assert!(total - once < (100_000 - 65_536) * 512, "{once} {total}");
    }
    // `TJ` は、すべての文字列を 1 つの並びの配列にする
    let half = "A".repeat(50_000);
    let text = format!("BT /F1 12 Tf [({half}) -10 ({half})] TJ ET");
    let run = used(
        &one_page(Page::new(&text).resources(HELVETICA)),
        Kind::DecodedBytes,
    );
    assert!(run >= (100_000 - 65_536) * 512, "{run}");
}

#[test]
fn counts_memory_of_curves() {
    use zumen_base::Kind;
    // 塗りはハッチングになり、曲線の区間はそれぞれ辺のスプライン（制御点 4 つと節点 8 つ）になる。線は 1 つの
    // スプライン（区間ごとに制御点 3 つと節点 3 つ）になる。形の配列のメモリを、型の大きさの 2 倍と数える
    let curves = format!("0 0 m {}", "10 10 20 10 30 0 c ".repeat(1_000));
    let fill = used(
        &one_page(Page::new(&format!("{curves}f"))),
        Kind::DecodedBytes,
    );
    assert!(fill >= 2 * 1_000 * (168 + 4 * 24 + 8 * 8), "{fill}");
    let stroke = used(
        &one_page(Page::new(&format!("{curves}S"))),
        Kind::DecodedBytes,
    );
    assert!(stroke >= 2 * 1_000 * (3 * 24 + 3 * 8), "{stroke}");
    // 部分経路ごとの輪も数える
    let loops = "0 0 m 10 10 20 10 30 0 c ".repeat(1_000);
    let hatch = used(
        &one_page(Page::new(&format!("{loops}f"))),
        Kind::DecodedBytes,
    );
    assert!(
        hatch >= 2 * 1_000 * (64 + 2 * 168 + 4 * 24 + 8 * 8),
        "{hatch}"
    );
    // 線分の折れ線は、頂点の 1 つを 32 バイトの 2 倍と数える
    let lines = format!("0 0 m {}S", "10 10 l 0 0 l ".repeat(500));
    let polyline = used(&one_page(Page::new(&lines)), Kind::DecodedBytes);
    assert!(polyline >= 2 * 1_001 * 32, "{polyline}");
    assert!(polyline < fill, "{polyline} {fill}");
}

#[test]
fn counts_memory_of_unicode_text() {
    use zumen_base::Kind;
    // ToUnicode の CMap の写す先は、長さが決まっていない。装置は、字形の Unicode を 3 つに写すので、1 バイトを
    // 4 バイトと数える。<0001> を 1,000 字の A に戻す字形の 100 字は、10 万字の文字になる
    let cmap = format!(
        "/CIDInit /ProcSet findresource begin 12 dict begin begincmap \
         /CIDSystemInfo << /Registry (Adobe) /Ordering (UCS) /Supplement 0 >> def \
         /CMapName /Adobe-Identity-UCS def /CMapType 2 def \
         1 begincodespacerange <0000> <FFFF> endcodespacerange \
         1 beginbfchar <0001> <{}> endbfchar \
         endcmap CMapName currentdict /CMap defineresource pop end end",
        "0041".repeat(1_000)
    );
    let content = format!("BT /F2 10 Tf 100 100 Td <{}> Tj ET", "0001".repeat(100));
    let mut document = Document::new(vec![Page::new(&content).resources("/Font << /F2 3 0 R >>")]);
    document.objects = vec![
        cid_font("MS-Gothic", "Identity-H", "").into_bytes(),
        common::stream("", cmap),
    ];
    let src = document.bytes();
    let (drawing, _) = lift(&src);
    let Shape::Text(text) = &elements(&drawing)[0].shape else {
        panic!("文字でない");
    };
    assert_eq!(text.content.len(), 100_000);
    let bytes = used(&src, Kind::DecodedBytes);
    assert!(bytes >= 100 * 1_000 * 4, "{bytes}");
}

/// TrueType のフォントの字形 1 が、空の字形 2 を `components` 回使う合成の字形のページ。`count` 字を描画モード
/// `mode` で描く
fn composite_document(components: u16, count: usize, mode: u32) -> Vec<u8> {
    font_document(
        "/CIDFontType2",
        "/FontFile2",
        truetype(&[Vec::new(), composite_glyph(components), Vec::new()]),
        count,
        mode,
    )
}

/// `components` 個の部品（字形 2）を持つ合成の字形
fn composite_glyph(components: u16) -> Vec<u8> {
    let mut glyph = Vec::new();
    // 輪郭の数（負は合成の字形）・外接矩形
    for value in [0xFFFF, 0, 0, 100, 100] {
        be16(&mut glyph, value);
    }
    for index in 0..components {
        // 引数は 1 バイトの x・y（ARGS_ARE_XY_VALUES）。最後のほかは、次の部品がある（MORE_COMPONENTS）
        let more = if index + 1 < components { 0x0020 } else { 0 };
        be16(&mut glyph, 0x0002 | more);
        be16(&mut glyph, 2);
        glyph.extend([0, 0]);
    }
    glyph
}

/// 字形を 1 つ（`endchar` だけ）持つ、最小の CFF のフォント（Adobe の Technical Note #5176）。名前・Top DICT・
/// 文字列・Global Subr・CharStrings の INDEX と、空の Private DICT
fn cff_font() -> Vec<u8> {
    let mut font = vec![1, 0, 4, 1];
    // Name INDEX（1 つ、「F」）
    font.extend([0, 1, 1, 1, 2, b'F']);
    // Top DICT INDEX。CharStrings は 30 バイト目、Private DICT は 38 バイト目の 0 バイト
    font.extend([0, 1, 1, 1, 12]);
    font.extend([28, 0, 30, 17, 28, 0, 0, 28, 0, 38, 18]);
    // String INDEX と Global Subr INDEX は空
    font.extend([0, 0, 0, 0]);
    // CharStrings INDEX（.notdef と字形 1。どちらも endchar）
    font.extend([0, 2, 1, 1, 2, 3, 14, 14]);
    assert_eq!(font.len(), 38);
    font
}

/// `font` を埋め込んだ Type0 のフォントで、CID 1 を `count` 字、描画モード `mode` で描くページ。`subtype` は
/// 子孫の CID フォントの種類、`file` はプログラムを持つ記述子のキー（9.9）
fn font_document(subtype: &str, file: &str, font: Vec<u8>, count: usize, mode: u32) -> Vec<u8> {
    let text = format!(
        "BT /F 12 Tf {mode} Tr 100 100 Td <{}> Tj ET",
        "0001".repeat(count)
    );
    let mut document = Document::new(vec![Page::new(&text).resources("/Font << /F 3 0 R >>")]);
    let stream = if file == "/FontFile3" {
        common::stream("/Subtype /CIDFontType0C", font)
    } else {
        common::stream("", font)
    };
    document.objects = vec![
        b"<< /Type /Font /Subtype /Type0 /BaseFont /Outline /Encoding /Identity-H \
          /DescendantFonts [4 0 R] >>"
            .to_vec(),
        format!(
            "<< /Type /Font /Subtype {subtype} /BaseFont /Outline /CIDSystemInfo \
             << /Registry (Adobe) /Ordering (Identity) /Supplement 0 >> /FontDescriptor 5 0 R \
             /CIDToGIDMap /Identity >>"
        )
        .into_bytes(),
        format!(
            "<< /Type /FontDescriptor /FontName /Outline /Flags 4 /FontBBox [0 0 100 100] \
             /ItalicAngle 0 /Ascent 800 /Descent -200 /CapHeight 700 /StemV 80 {file} 6 0 R >>"
        )
        .into_bytes(),
        stream,
    ];
    document.bytes()
}

#[test]
fn counts_programs_of_clipping_text() {
    use zumen_base::Kind;
    // hayro-font は、切り抜きにする字の輪郭を覚えずに字ごとに作り直し、作る時間はフォントのプログラムで決まる。
    // TrueType の合成の字形は、字形を 2,049 回までたどり、たどるたびに部品をすべて読むので、いちばん長い合成の
    // 字形の部品の数（6 バイトを 1 つ）に比例して数える
    let clip = used(&composite_document(1_000, 10, 7), Kind::Work);
    assert!(clip >= 10 * 2_049 * 1_000, "{clip}");
    let fill = used(&composite_document(1_000, 10, 0), Kind::Work);
    assert!(fill < 1_000_000, "{fill}");
    // CFF と Type 1 の字形は、1 字に 100 万の演算まで動くので、1 字を 65,536 と数える
    let cff = |mode| {
        used(
            &font_document("/CIDFontType0", "/FontFile3", cff_font(), 10, mode),
            Kind::Work,
        )
    };
    assert!(cff(7) >= 10 * 65_536, "{}", cff(7));
    assert!(cff(0) < 65_536, "{}", cff(0));
    // 埋め込まないフォントは、hayro-font が持つ標準のフォントで描くので、数えない
    let text = format!("BT /F1 12 Tf 7 Tr ({}) Tj ET", "A".repeat(100));
    let standard = used(&one_page(Page::new(&text).resources(HELVETICA)), Kind::Work);
    assert!(standard < 100 * 65_536, "{standard}");
}

#[test]
fn counts_composite_glyphs_of_a_table_at_the_start_of_the_file() {
    use zumen_base::Kind;
    // skrifa は、表の記録の位置が 0 の表も読む。glyf を、表の目録（12 + 16 × 6 = 108 バイト）から始まる表とし、
    // 記録の位置を 0、長さと loca を 108 だけ増やす
    let mut font = truetype(&[Vec::new(), composite_glyph(1_000), Vec::new()]);
    let be32_at = |font: &[u8], at: usize| u32::from_be_bytes(font[at..at + 4].try_into().unwrap());
    assert_eq!(&font[12..16], b"glyf");
    assert_eq!(&font[76..80], b"loca");
    let length = be32_at(&font, 24);
    font[20..24].copy_from_slice(&0u32.to_be_bytes());
    font[24..28].copy_from_slice(&(length + 108).to_be_bytes());
    let (loca, size) = (be32_at(&font, 84) as usize, be32_at(&font, 88) as usize);
    for entry in font[loca..loca + size].as_chunks_mut::<4>().0 {
        *entry = (u32::from_be_bytes(*entry) + 108).to_be_bytes();
    }
    let document = font_document("/CIDFontType2", "/FontFile2", font, 10, 7);
    let clip = used(&document, Kind::Work);
    assert!(clip >= 10 * 2_049 * 1_000, "{clip}");
}

#[test]
fn counts_setup_programs_of_hint_reliant_fonts() {
    use zumen_base::Kind;
    // skrifa は、命令に頼るフォントを読むときに、`fpgm` と `prep` を 1 度ずつ、100 万の命令まで動かす。走査と
    // hayro-interpret が読むので、2 × 2 × 100 万 × 8 を数える
    let program = [0xB0, 0, 0x21]; // PUSHB[0] 0・POP
    let document = |family, program: &[u8]| {
        font_document(
            "/CIDFontType2",
            "/FontFile2",
            hinted_font(family, program),
            1,
            0,
        )
    };
    let hinted = used(&document("MingLiU", &program), Kind::Work);
    assert!(hinted >= 2 * 2 * 1_000_000 * 8, "{hinted}");
    // 影の領域の点は 1 つ 17 バイト、走査と hayro-interpret で 2 度作る
    let bytes = used(&document("MingLiU", &program), Kind::DecodedBytes);
    assert!(bytes >= 2 * 65_539 * 17, "{bytes}");
    // 頼らないフォントと、空のプログラムは、動かない
    let plain = used(&document("Plain", &program), Kind::Work);
    assert!(plain < 1_000_000, "{plain}");
    let empty = used(&document("MingLiU", &[]), Kind::Work);
    assert!(empty < 1_000_000, "{empty}");
}

#[test]
fn refuses_clipping_text_of_programs_without_a_bound() {
    // 切り抜きにする字の輪郭は、字ごとに作り直す。命令に頼るフォントは、たどる字形ごとに 100 万の命令が動き、VARC は、
    // 部品ごとに条件を評価する。上限で数えると文書を読めないので、解釈しない
    for font in [hinted_font("MingLiU", &[]), varc_font()] {
        let clip = font_document("/CIDFontType2", "/FontFile2", font.clone(), 10, 7);
        let (drawing, diag) = lift(&clip);
        assert!(elements(&drawing).is_empty());
        assert!(
            diag.iter().any(|d| d.code() == codes::PDF_PAGE_DROPPED),
            "{diag:?}"
        );
        // 文字のない文字列は、輪郭を作らない
        let empty = font_document("/CIDFontType2", "/FontFile2", font.clone(), 0, 7);
        let (_, diag) = lift(&empty);
        assert!(
            !diag.iter().any(|d| d.code() == codes::PDF_PAGE_DROPPED),
            "{diag:?}"
        );
        // 塗るだけなら、輪郭を字ごとに作り直さない
        let fill = font_document("/CIDFontType2", "/FontFile2", font, 10, 0);
        let (_, diag) = lift(&fill);
        assert!(
            !diag.iter().any(|d| d.code() == codes::PDF_PAGE_DROPPED),
            "{diag:?}"
        );
    }
}

/// 線を描くフォーム
const LINE_FORM: &str = "/Type /XObject /Subtype /Form /BBox [0 0 100 100]";

/// 相互参照の表の 5 番の項目が 4 番の前の定義（線を描くフォーム）を指し、4 番の後ろの定義 `later` は表にないファイル。
/// ページの資源は `/XObject << /X 4 0 R /Y 5 0 R >>`、内容は `content`。`extra` は 7 番からのオブジェクトで、表に載せる
fn repaired_document(content: &str, later: Vec<u8>, extra: &[Vec<u8>]) -> Vec<u8> {
    let first = common::stream(LINE_FORM, "0 0 m 10 10 l S");
    let resources = "/XObject << /X 4 0 R /Y 5 0 R >>";
    repaired_with(resources, "", first, content, later, extra)
}

/// [`repaired_document`] の、ページの資源を `resources`、カタログに足す項目を `catalog`、4 番の前の定義を `first` に
/// したファイル
fn repaired_with(
    resources: &str,
    catalog: &str,
    first: Vec<u8>,
    content: &str,
    later: Vec<u8>,
    extra: &[Vec<u8>],
) -> Vec<u8> {
    let mut objects: Vec<(u32, Vec<u8>)> = vec![
        (
            1,
            format!("<< /Type /Catalog /Pages 2 0 R {catalog} >>").into_bytes(),
        ),
        (2, b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_vec()),
        (
            3,
            format!(
                "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Contents 6 0 R \
                 /Resources << {resources} >> >>"
            )
            .into_bytes(),
        ),
        (4, first),
        (6, common::stream("", content)),
    ];
    for (number, body) in (7u32..).zip(extra) {
        objects.push((number, body.clone()));
    }
    objects.push((4, later));
    let mut src = b"%PDF-1.7\n".to_vec();
    // 番号ごとに、はじめの定義の位置
    let mut offsets = std::collections::BTreeMap::new();
    for (number, body) in &objects {
        offsets.entry(*number).or_insert(src.len());
        src.extend(format!("{number} 0 obj\n").as_bytes());
        src.extend(body);
        src.extend(b"\nendobj\n");
    }
    let size = 7 + extra.len();
    let xref = src.len();
    src.extend(format!("xref\n0 {size}\n0000000000 65535 f \n").as_bytes());
    for number in 1..size as u32 {
        let first = if number == 5 { 4 } else { number };
        let offset = offsets.get(&first).copied().unwrap_or_default();
        src.extend(format!("{offset:010} 00000 n \n").as_bytes());
    }
    src.extend(
        format!("trailer\n<< /Size {size} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n").as_bytes(),
    );
    src
}

#[test]
fn walks_objects_after_repairing_cross_references() {
    use zumen_base::Kind;
    // hayro-syntax は、相互参照の表の項目が別の番号のオブジェクトを指すと、表をファイルの全体から作り直し、同じ番号の
    // 後ろの定義を使う。走査の前に作り直させるので、走査も hayro-interpret も、4 番の後ろの定義（10 万の `w` と
    // 長方形）を使う
    let form = LINE_FORM;
    let later = common::stream(form, format!("{}0 0 10 10 re f", "1 w ".repeat(100_000)));
    let src = repaired_document("/X Do /Y Do", later, &[]);
    assert!(used(&src, Kind::Work) >= 100_000);
    let (drawing, diag) = lift(&src);
    let shapes: Vec<&Shape<PaperPoint>> = elements(&drawing).iter().map(|e| &e.shape).collect();
    assert!(matches!(shapes.as_slice(), [Shape::Hatch(_)]), "{shapes:?}");
    assert!(
        diag.iter().any(|d| d.code() == codes::PDF_XOBJECT_MISSING),
        "{diag:?}"
    );
    // 後ろの定義が、行の頂点の数が 0 の格子を塗るフォームなら、呼ぶ順によらず、解釈せずにシートだけを作る。走査の
    // 途中で作り直すと、前の定義を数えた後に、hayro-interpret が後ろの定義を解釈して、メモリを使い切る
    let later = common::stream(
        &format!("{form} /Resources << /Shading << /Sh 7 0 R >> >>"),
        "/Sh sh",
    );
    let mesh = common::stream(
        "/ShadingType 5 /ColorSpace /DeviceGray /BitsPerCoordinate 8 /BitsPerComponent 8 \
         /VerticesPerRow 0 /Decode [0 100 0 100 0 1]",
        [0u8, 0, 0, 100, 0, 0],
    );
    for content in ["/X Do /Y Do", "/Y Do /X Do"] {
        let (drawing, diag) = lift(&repaired_document(
            content,
            later.clone(),
            std::slice::from_ref(&mesh),
        ));
        assert!(elements(&drawing).is_empty(), "{content}");
        assert!(
            diag.iter().any(|d| d.code() == codes::PDF_PAGE_DROPPED),
            "{content}: {diag:?}"
        );
    }
}

#[test]
fn settles_only_entries_reached_from_the_document() {
    use zumen_base::Kind;
    let later = common::stream(
        LINE_FORM,
        format!("{}0 0 10 10 re f", "1 w ".repeat(100_000)),
    );
    let first = || common::stream(LINE_FORM, "0 0 m 10 10 l S");
    let resources = "/XObject << /X 4 0 R >>";
    let shapes = |src: &[u8]| {
        let (drawing, _) = lift(src);
        elements(&drawing)
            .iter()
            .map(|element| format!("{:?}", element.shape))
            .collect::<Vec<_>>()
    };
    // カタログとページの辞書からたどれない項目は、別の番号のオブジェクトを指していても引かない。表を作り直さない
    // ので、4 番は前の定義（線）のまま。ハッシュの順に引くと、環境によって数える量が変わる
    let src = repaired_with(resources, "", first(), "/X Do", later.clone(), &[]);
    let work = used(&src, Kind::Work);
    assert!(work < 100_000, "{work}");
    assert!(shapes(&src).iter().all(|shape| !shape.contains("Hatch")));
    // カタログの知らないキーからでも、たどれる項目は引く。ページを読むときに引かれても、数えた定義と同じものを読む
    let src = repaired_with(
        resources,
        "/Unknown [<< /Deep 5 0 R >>]",
        first(),
        "/X Do",
        later.clone(),
        &[],
    );
    let work = used(&src, Kind::Work);
    assert!(work >= 100_000, "{work}");
    assert!(shapes(&src).iter().any(|shape| shape.contains("Hatch")));
    // 辞書の後ろに読めないストリームがある定義は、オブジェクトとしては読めず、辞書としてなら読めるので、辞書としても
    // 引く。hayro-interpret がフォントの辞書として引くと、そこで作り直す
    let junk = b"<< /Type /Junk >>\nstream\njunk".to_vec();
    let src = repaired_with(
        "/XObject << /X 4 0 R >> /Font << /Y 5 0 R >>",
        "",
        junk,
        "/X Do",
        later,
        &[],
    );
    let work = used(&src, Kind::Work);
    assert!(work >= 100_000, "{work}");
    assert!(shapes(&src).iter().any(|shape| shape.contains("Hatch")));
}

#[test]
fn reads_only_objects_reached_from_the_document() {
    use zumen_base::Kind;
    // 走査の前に引くのは、参照でたどれるオブジェクトだけ。どのキーの値かは見ない
    let big = format!("[{}]", "0 ".repeat(100_000)).into_bytes();
    let mut document = Document::new(vec![Page::new("0 0 m 10 10 l S")]);
    let plain = used(&document.bytes(), Kind::Work);
    document.objects = vec![big];
    let unreached = used(&document.bytes(), Kind::Work);
    assert!(unreached < plain + 100, "{plain} {unreached}");
    document.catalog = "/Unknown 3 0 R";
    let reached = used(&document.bytes(), Kind::Work);
    assert!(reached >= plain + 200_000 / 4, "{plain} {reached}");
}

#[test]
fn counts_streams_read_without_a_right_length() {
    use zumen_base::Kind;
    // `Length` が合わないストリームは、hayro-syntax が、読むたびに本体を読み進めて `endstream` を探す
    let body = format!("%{}", "x".repeat(99_999));
    let calls = "/F Do ".repeat(100);
    let form = |length: &str| {
        let mut document =
            Document::new(vec![Page::new(&calls).resources("/XObject << /F 3 0 R >>")]);
        document.objects =
            vec![format!("<< {length} {LINE_FORM} >>\nstream\n{body}\nendstream").into_bytes()];
        used(&document.bytes(), Kind::Work)
    };
    let right = form("/Length 100000");
    for length in ["/Length 1", ""] {
        let wrong = form(length);
        assert!(wrong >= right + 100 * 100_000 / 4, "{right} {wrong}");
    }
    // ページの内容も同じ
    let contents = format!("[{}]", "3 0 R ".repeat(100));
    let content = |length: &str| {
        let mut document = Document::new(vec![Page::new("").contents(&contents)]);
        document.objects = vec![format!("<< {length} >>\nstream\n{body}\nendstream").into_bytes()];
        used(&document.bytes(), Kind::Work)
    };
    let right = content("/Length 100000");
    let wrong = content("");
    assert!(wrong >= right + 100 * 100_000 / 4, "{right} {wrong}");
}

#[test]
fn counts_values_of_objects_read_again() {
    use zumen_base::Kind;
    // hayro-syntax は、キーを引くたびに値の参照を解き、型が違っても読み飛ばす。読み直す辞書の値が大きな配列を指せば、
    // 使うたびに読む
    let big = format!("[{}]", "0 ".repeat(10_000)).into_bytes();
    let many = |calls: &str, resources: &str, objects: Vec<Vec<u8>>| {
        let mut document = Document::new(vec![Page::new(calls).resources(resources)]);
        document.objects = objects;
        used(&document.bytes(), Kind::Work)
    };
    let calls = "/F Do ".repeat(100);
    let resources = "/XObject << /F 3 0 R >>";
    let form = |extra: &str| common::stream(&format!("{LINE_FORM} {extra}"), "");
    let plain = many(&calls, resources, vec![form("")]);
    let once = 100 * 20_000 / 4;
    // 1 段
    let work = many(&calls, resources, vec![form("/Matrix 4 0 R"), big.clone()]);
    assert!(work >= plain + once, "{plain} {work}");
    // `DecodeParms` の配列の辞書の値まで、3 段
    let filtered = common::stream(
        &format!("{LINE_FORM} /Filter /ASCIIHexDecode /DecodeParms 4 0 R"),
        ">",
    );
    let chain = vec![
        filtered,
        b"[5 0 R]".to_vec(),
        b"<< /Predictor 6 0 R >>".to_vec(),
        big.clone(),
    ];
    let work = many(&calls, resources, chain);
    assert!(work >= plain + once, "{plain} {work}");
    // 拡張グラフィックス状態
    let state = "/G gs ".repeat(100);
    let plain = many(&state, "/ExtGState << /G << /LW 1 >> >>", Vec::new());
    let chain = vec![b"[4 0 R 12]".to_vec(), big.clone()];
    let work = many(&state, "/ExtGState << /G << /Font 3 0 R >> >>", chain);
    assert!(work >= plain + once, "{plain} {work}");
    // 同じ参照は、書いた回数だけ読む
    let list = format!("[[{}] 0]", "4 0 R ".repeat(10)).into_bytes();
    let chain = vec![list, big.clone()];
    let work = many(&state, "/ExtGState << /G << /D 3 0 R >> >>", chain);
    assert!(work >= plain + 10 * once, "{plain} {work}");
    // 資源の辞書の、種類の辞書の中の資源は、名前で引くまで読まない
    let plain = many(&calls, resources, vec![form("")]);
    let unused = vec![form("/Resources << /Font << /F1 4 0 R >> >>"), big.clone()];
    let work = many(&calls, resources, unused);
    assert!(work < plain + once, "{plain} {work}");
    // 種類の辞書を参照で書けば、読む
    let kinds = vec![form("/Resources << /Font 4 0 R >>"), big.clone()];
    let work = many(&calls, resources, kinds);
    assert!(work >= plain + once, "{plain} {work}");
    // ほかのキーは、hayro-syntax が読まない
    let other = vec![form("/Resources << /Other 4 0 R >>"), big.clone()];
    let work = many(&calls, resources, other);
    assert!(work < plain + once, "{plain} {work}");
    // ページの値も、ページごとに読む
    let mut document = Document::new(vec![Page::new("").extra("/Group 3 0 R"); 100]);
    document.objects = vec![big.clone()];
    let work = used(&document.bytes(), Kind::Work);
    assert!(work >= once, "{work}");
    // 注釈は、項目を読むたびに数える
    let annots = format!("/Annots [{}]", "3 0 R ".repeat(100));
    let mut document = Document::new(vec![Page::new("").extra(&annots)]);
    document.objects =
        vec![format!("<< /Subtype /Text /A {} >>", String::from_utf8_lossy(&big)).into_bytes()];
    let work = used(&document.bytes(), Kind::Work);
    assert!(work >= once, "{work}");
    // 注釈が指す親のページは読まないので、注釈の数の 2 乗にならない
    let refs: String = (0..1_000).map(|at| format!("{} 0 R ", at + 3)).collect();
    let annots = format!("/Annots [{refs}]");
    let mut document = Document::new(vec![Page::new("").extra(&annots)]);
    document.objects = (0..1_000)
        .map(|_| b"<< /Subtype /Link /Rect [0 0 1 1] /P 1003 0 R >>".to_vec())
        .collect();
    let work = used(&document.bytes(), Kind::Work);
    assert!(work < 100_000, "{work}");
    // 親は読まないので、ページの木の配列は、ページごとに数えない
    let document = Document::new(vec![Page::new(""); 1_000]);
    let work = used(&document.bytes(), Kind::Work);
    assert!(work < 1_000_000, "{work}");
}

/// オブジェクトを順に書いたファイル。項目は、定義のヘッダーと本体。表には、番号ごとに、はじめの定義を載せる。カタログは
/// 1 番。`zero` なら、表の 0 番を、使用中の項目として、はじめの定義（番号の違うオブジェクト）に向ける
fn first_definitions(objects: &[(&str, Vec<u8>)], zero: bool) -> Vec<u8> {
    let mut src = b"%PDF-1.7\n".to_vec();
    let mut offsets = std::collections::BTreeMap::new();
    for (header, body) in objects {
        let digits = header.chars().take_while(char::is_ascii_digit);
        let number: u32 = digits.collect::<String>().parse().unwrap_or_default();
        offsets.entry(number).or_insert(src.len());
        src.extend(format!("{header}\n").as_bytes());
        src.extend(body);
        src.extend(b"\nendobj\n");
    }
    let size = offsets.keys().max().copied().unwrap_or_default() + 1;
    let xref = src.len();
    src.extend(format!("xref\n0 {size}\n").as_bytes());
    if zero {
        let first = offsets.get(&1).copied().unwrap_or_default();
        src.extend(format!("{first:010} 00000 n \n").as_bytes());
    } else {
        src.extend(b"0000000000 65535 f \n");
    }
    for number in 1..size {
        match offsets.get(&number) {
            Some(offset) => src.extend(format!("{offset:010} 00000 n \n").as_bytes()),
            None => src.extend(b"0000000000 00000 f \n"),
        }
    }
    src.extend(
        format!("trailer\n<< /Size {size} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n").as_bytes(),
    );
    src
}

/// カタログ・ページの木・ページ（資源は `resources`、内容は 4 番）と、`objects`
fn with_page<'a>(resources: &str, objects: Vec<(&'a str, Vec<u8>)>) -> Vec<(&'a str, Vec<u8>)> {
    let page = format!(
        "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Contents 4 0 R \
         /Resources << {resources} >> >>"
    );
    let mut all = vec![
        ("1 0 obj", b"<< /Type /Catalog /Pages 2 0 R >>".to_vec()),
        (
            "2 0 obj",
            b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_vec(),
        ),
        ("3 0 obj", page.into_bytes()),
    ];
    all.extend(objects);
    all
}

#[test]
fn settles_entries_that_only_some_types_repair() {
    use zumen_base::Kind;
    // hayro-syntax は、引く型で読めず、読み飛ばせもしない本体で、表を作り直す。符号か点のあとに数字のない数は、読めても
    // 読み飛ばせないので、オブジェクトとして引いても作り直さない。走査の前に作り直さないと、hayro-interpret は、走査が
    // 数えていない 6 番の後ろの定義を解釈する
    let later = common::stream(LINE_FORM, "1 w ".repeat(100_000));
    let resources = "/XObject << /X 5 0 R /Y 6 0 R >>";
    let content = common::stream("", "/Y Do /X Do");
    for body in ["-x", ".x", "-.x", "-+1"] {
        let objects = with_page(
            resources,
            vec![
                ("4 0 obj", content.clone()),
                ("5 0 obj", body.as_bytes().to_vec()),
                ("6 0 obj", common::stream(LINE_FORM, "")),
                ("6 0 obj", later.clone()),
            ],
        );
        let work = used(&first_definitions(&objects, false), Kind::Work);
        assert!(work >= 100_000, "{body}: {work}");
    }
    // 定義の番号の側でも、同じ
    let objects = with_page(
        resources,
        vec![
            ("4 0 obj", content.clone()),
            ("5.5 0 obj", b"<< >>".to_vec()),
            ("6 0 obj", common::stream(LINE_FORM, "")),
            ("6 0 obj", later.clone()),
        ],
    );
    let work = used(&first_definitions(&objects, false), Kind::Work);
    assert!(work >= 100_000, "{work}");
    // 符号か点だけの番号は 0 番で、世代番号と続けて書ける。表の 0 番が番号の違うオブジェクトを指せば、作り直す
    for reference in [".-0 R", "--0 R"] {
        let objects = with_page(
            &format!("/XObject << /X {reference} /Y 6 0 R >>"),
            vec![
                ("4 0 obj", content.clone()),
                ("6 0 obj", common::stream(LINE_FORM, "")),
                ("6 0 obj", later.clone()),
            ],
        );
        let work = used(&first_definitions(&objects, true), Kind::Work);
        assert!(work >= 100_000, "{reference}: {work}");
    }
    // 配列の中では、符号か点だけの項目に続けて、参照を読む（`[--0 0 R]` は、`-` と `-0 0 R`）
    for reference in ["--0 0 R", ".-0 0 R", "-.-0 0 R", "+.-0 0 R", "---0 R"] {
        let form = common::stream(&format!("{LINE_FORM} /Matrix [{reference} 1 0 0 1 0]"), "");
        let objects = with_page(
            resources,
            vec![
                ("4 0 obj", content.clone()),
                ("5 0 obj", form),
                ("6 0 obj", common::stream(LINE_FORM, "")),
                ("6 0 obj", later.clone()),
            ],
        );
        let work = used(&first_definitions(&objects, true), Kind::Work);
        assert!(work >= 100_000, "{reference}: {work}");
    }
}

/// 表の項目 `twice` と `shared` が 1 つの位置を指し、`twice` を 2 度定義したファイル。`later` は、ファイルの終わりに足す
/// 定義。カタログは 1 番、ページの木は 2 番で、`Kids` がない。文書の情報は 98 番
fn tied_entries(twice: u32, shared: u32, later: &str) -> Vec<u8> {
    let first = "<< /Type /Page /MediaBox [0 0 100 100] /Contents [] >>";
    tied_definitions(first, twice, shared, later)
}

/// [`tied_entries`] の、`twice` のはじめの定義を `first` にしたファイル
fn tied_definitions(first: &str, twice: u32, shared: u32, later: &str) -> Vec<u8> {
    let page = |side: u32| format!("<< /Type /Page /MediaBox [0 0 {side} {side}] /Contents [] >>");
    let mut src = b"%PDF-1.7\n".to_vec();
    let mut offsets = Vec::new();
    for (number, body) in [
        (98, "<< /Title (Kept) >>".to_owned()),
        (1, "<< /Type /Catalog /Pages 2 0 R >>".to_owned()),
        (2, "<< /Type /Pages >>".to_owned()),
        (twice, first.to_owned()),
    ] {
        offsets.push((number, src.len()));
        src.extend(format!("{number} 0 obj\n{body}\nendobj\n").as_bytes());
    }
    let tied = offsets.last().map_or(0, |entry: &(u32, usize)| entry.1);
    offsets.push((shared, tied));
    src.extend(format!("{twice} 0 obj\n{}\nendobj\n{later}", page(200)).as_bytes());
    let xref = src.len();
    src.extend(b"xref\n0 1\n0000000000 65535 f \n");
    for (number, offset) in offsets {
        src.extend(format!("{number} 1\n{offset:010} 00000 n \n").as_bytes());
    }
    src.extend(
        format!("trailer\n<< /Size 99 /Root 1 0 R /Info 98 0 R >>\nstartxref\n{xref}\n%%EOF\n")
            .as_bytes(),
    );
    src
}

/// オブジェクトのストリーム `container` を 2 度定義し（中の 30 番の本体は、`first` と `later`）、相互参照ストリームの項目
/// `shared` がはじめの定義を指すファイル。カタログは 1 番で、2 番のページの木は表にない。`plain` は、表に載せる定義
fn tied_object_stream(
    container: u32,
    shared: u32,
    first: &str,
    later: &str,
    plain: &[(u32, String)],
) -> Vec<u8> {
    let packed = |body: &str| {
        let head = "30 0 ";
        let dict = format!("/Type /ObjStm /N 1 /First {}", head.len());
        common::stream(&dict, format!("{head}{body}"))
    };
    let mut src = b"%PDF-1.7\n".to_vec();
    let mut entries = std::collections::BTreeMap::new();
    entries.insert(30, (2u8, container as usize));
    let catalog = "<< /Type /Catalog /Pages 2 0 R >>".to_owned();
    for (number, body) in core::iter::once((1, &catalog)).chain(plain.iter().map(|(n, b)| (*n, b)))
    {
        entries.insert(number, (1, src.len()));
        src.extend(format!("{number} 0 obj\n{body}\nendobj\n").as_bytes());
    }
    entries.insert(container, (1, src.len()));
    entries.insert(shared, (1, src.len()));
    for body in [first, later] {
        src.extend(format!("{container} 0 obj\n").as_bytes());
        src.extend(packed(body));
        src.extend(b"\nendobj\n");
    }
    let table = 99u32;
    let xref = src.len();
    entries.insert(table, (1, xref));
    let mut data = Vec::new();
    let mut index = String::new();
    for (number, (kind, field)) in &entries {
        index.push_str(&format!("{number} 1 "));
        data.push(*kind);
        data.extend((*field as u32).to_be_bytes());
        data.extend([0, 0]);
    }
    let dict = format!("/Type /XRef /Size 100 /W [1 4 2] /Index [{index}] /Root 1 0 R");
    src.extend(format!("{table} 0 obj\n").as_bytes());
    src.extend(common::stream(&dict, data));
    src.extend(format!("\nendobj\nstartxref\n{xref}\n%%EOF\n").as_bytes());
    src
}

#[test]
fn reads_pages_again_without_a_page_tree() {
    let one_sheet = |src: &[u8], label: &str| {
        let (drawing, _) = lift(src);
        assert_eq!(drawing.sheets.len(), 1, "{label}");
        let size = drawing.sheets[0].size.map(|[w, h]| (w.0, h.0));
        assert_size(size, 200.0, 200.0);
        let document = drawing
            .extensions
            .iter()
            .find(|extension| extension.namespace == "pdf.document")
            .unwrap();
        assert!(
            document
                .fields
                .iter()
                .any(|field| field.key.as_ref() == "title"
                    && field.value == FieldValue::Text("Kept".to_owned())),
            "{label}"
        );
    };
    // ページの木を読めないと、hayro-syntax は、表の項目を順に引いてページを探す。同じ位置の項目の順は、表のハッシュで
    // 決まり、32 ビットと 64 ビットの環境で違う。走査した表で読み直すので、番号によらず、後ろの定義を読む。文書の情報は、
    // 走査で選んだトレーラーから読む
    for twice in 5..=20 {
        for shared in 5..=20 {
            if twice == shared {
                continue;
            }
            let label = format!("{twice} {shared}");
            one_sheet(&tied_entries(twice, shared, ""), &label);
            // 作り直すと木を読めても、同じ
            let tree = format!("2 0 obj\n<< /Type /Pages /Kids [{twice} 0 R] >>\nendobj\n");
            one_sheet(&tied_entries(twice, shared, &tree), &label);
            // はじめの定義は内容がなく、作り直す前に引くと、ページが見つからない
            let first = "<< /Type /Page /MediaBox [0 0 100 100] >>";
            one_sheet(&tied_definitions(first, twice, shared, ""), &label);
            // ファイルの終わりで読みかけの定義は、読めるものにならない
            let mut cut = tied_entries(twice, shared, "");
            cut.extend(format!("{twice} 0 obj").as_bytes());
            one_sheet(&cut, &label);
            // 符号だけの数も、ファイルの終わりでは読めない
            let mut sign = tied_entries(twice, shared, "");
            sign.extend(format!("{twice} 0 obj -").as_bytes());
            one_sheet(&sign, &label);
            // 作り直したことは、ファイルの終わりに何があっても分かる。後ろの辞書は閉じず、番号が同じ定義は読めない
            let mut tail = tied_entries(twice, shared, &tree);
            tail.extend(b"2147483646 0 obj\n5-3.2\nendobj\n)>>\n<< /A (\n");
            one_sheet(&tail, &label);
        }
    }
    // カタログは、走査で選んだトレーラーのものを読む
    let other = "2 0 obj\n<< /Type /Pages /Kids [5 0 R] >>\nendobj\n\
                 7 0 obj\n<< /Type /Catalog /Pages 8 0 R >>\nendobj\n\
                 8 0 obj\n<< /Type /Pages /Kids [9 0 R] >>\nendobj\n\
                 9 0 obj\n<< /Type /Page /MediaBox [0 0 400 400] /Contents [] >>\nendobj\n\
                 10 0 obj\n<< /Root 7 0 R >>\nendobj\n";
    one_sheet(&tied_entries(5, 6, other), "other catalog");
}

#[test]
fn reads_object_streams_again_without_a_page_tree() {
    use zumen_base::Kind;
    let page = |side: u32| format!("<< /Type /Page /MediaBox [0 0 {side} {side}] /Contents [] >>");
    // 解いたオブジェクトのストリームは、作り直しても残るので、順によっては、前の定義の中身を読む。読み直せば残らない。
    // ページがストリームの中にある
    for container in 5..=20 {
        for shared in 5..=20 {
            if container == shared {
                continue;
            }
            let src = tied_object_stream(container, shared, &page(100), &page(200), &[]);
            let (drawing, _) = lift(&src);
            assert_eq!(drawing.sheets.len(), 1, "{container} {shared}");
            let size = drawing.sheets[0].size.map(|[w, h]| (w.0, h.0));
            assert_size(size, 200.0, 200.0);
        }
    }
    // フォントがストリームの中にあると、数える量が同じ
    let font = |name: &str| format!("<< /Type /Font /Subtype /Type1 /BaseFont /{name} >>");
    let plain = vec![
        (
            3,
            "<< /Type /Page /MediaBox [0 0 612 792] /Contents 4 0 R /Resources << /Font << /F 30 0 R >> >> >>"
                .to_owned(),
        ),
        (4, "<< /Length 28 >>\nstream\nBT /F 9 Tf (a) Tj ET\n        \nendstream".to_owned()),
    ];
    let mut works = std::collections::BTreeSet::new();
    for container in 5..=20 {
        for shared in 5..=20 {
            if container != shared {
                let src = tied_object_stream(
                    container,
                    shared,
                    &font("A"),
                    &font("LongerNameOfTheFontThatCostsMoreToRead"),
                    &plain,
                );
                works.insert(used(&src, Kind::Work));
            }
        }
    }
    assert_eq!(works.len(), 1, "{works:?}");
}

#[test]
fn keeps_the_table_of_documents_with_page_trees() {
    // ページの木を読める文書は、読み直さないので、表が指す定義を読む。走査した表で読み直すと、後ろの定義を読んでしまう
    let line = common::stream("", "0 0 m 10 10 l S");
    let box_later = common::stream("", "0 0 10 10 re f");
    let shapes = |objects: &[(&str, Vec<u8>)]| {
        let (drawing, _) = lift(&first_definitions(objects, false));
        let mut found = Vec::new();
        for sheet in &drawing.sheets {
            found.push((sheet.size, sheet.elements.len()));
            assert!(
                sheet
                    .elements
                    .iter()
                    .all(|element| !matches!(element.shape, Shape::Hatch(_))),
                "{:?}",
                sheet.elements
            );
        }
        found
    };
    let catalog = ("1 0 obj", b"<< /Type /Catalog /Pages 2 0 R >>".to_vec());
    // 受け継ぐ値（大きさ・切り抜き・回転・資源）、中間の節、直接書いたページ、どの子もない節、同じ参照
    let root = "<< /Type /Pages /Count 3 /MediaBox [0 0 100 200] /Rotate 90 \
                /Resources << /Font << /F 9 0 R >> >> /Kids [3 0 R 5 0 R 3 0 R 7 0 R] >>";
    let found = shapes(&[
        catalog.clone(),
        ("2 0 obj", root.as_bytes().to_vec()),
        (
            "3 0 obj",
            b"<< /Type /Pages /Parent 2 0 R /CropBox [0 0 50 50] /Kids [4 0 R] >>".to_vec(),
        ),
        (
            "4 0 obj",
            b"<< /Type /Page /Parent 3 0 R /Contents 6 0 R >>".to_vec(),
        ),
        (
            "5 0 obj",
            b"<< /Type /Pages /Parent 2 0 R /Kids [<< /Contents 6 0 R /Rotate 0 >>] >>".to_vec(),
        ),
        ("6 0 obj", line.clone()),
        ("7 0 obj", b"<< /Type /Pages /Parent 2 0 R >>".to_vec()),
        (
            "9 0 obj",
            b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>".to_vec(),
        ),
        ("6 0 obj", box_later.clone()),
    ]);
    assert_eq!(found.len(), 2, "{found:?}");
    assert!(
        found.iter().all(|(_, elements)| *elements == 1),
        "{found:?}"
    );
    // 種類のないページ
    let found = shapes(&[
        catalog,
        ("2 0 obj", b"<< /Type /Pages /Kids [3 0 R] >>".to_vec()),
        ("3 0 obj", b"<< /Contents 4 0 R >>".to_vec()),
        ("4 0 obj", line),
        ("4 0 obj", box_later),
    ]);
    assert_eq!(found.len(), 1, "{found:?}");
    assert_eq!(found[0].1, 1, "{found:?}");
}

#[test]
fn reads_files_whose_table_comes_first_as_they_are() {
    // 表と `startxref` がファイルの初めにあり、終わりのフォームに `endstream` がない。読むときに足す字は、ファイルの
    // 中身を写さないので、フォームの本体は閉じず、読めない
    let bodies = [
        "<< /Type /Catalog /Pages 2 0 R >>".to_owned(),
        "<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_owned(),
        "<< /Type /Page /MediaBox [0 0 100 100] /Contents 4 0 R \
         /Resources << /XObject << /X 5 0 R >> >> >>"
            .to_owned(),
        "<< /Length 5 >>\nstream\n/X Do\nendstream".to_owned(),
        "<< /Type /XObject /Subtype /Form /BBox [0 0 10 10] >>\nstream\n0 0 m 10 10 l S\n"
            .to_owned(),
    ];
    let head = b"%PDF-1.7\n";
    let table = |offsets: &[usize]| {
        let mut table = format!("xref\n0 {}\n0000000000 65535 f \n", offsets.len() + 1);
        for offset in offsets {
            table.push_str(&format!("{offset:010} 00000 n \n"));
        }
        let size = offsets.len() + 1;
        let at = head.len();
        table.push_str(&format!(
            "trailer\n<< /Size {size} /Root 1 0 R >>\nstartxref\n{at}\n%%EOF\n"
        ));
        table
    };
    let start = head.len() + table(&[0; 5]).len();
    let mut objects = String::new();
    let mut offsets = Vec::new();
    for (number, body) in bodies.iter().enumerate() {
        offsets.push(start + objects.len());
        objects.push_str(&format!("{} 0 obj\n{body}", number + 1));
        if number + 1 < bodies.len() {
            objects.push_str("\nendobj\n");
        }
    }
    let mut src = head.to_vec();
    src.extend(table(&offsets).as_bytes());
    src.extend(objects.as_bytes());
    let (drawing, diag) = lift(&src);
    assert_eq!(drawing.sheets.len(), 1);
    assert!(elements(&drawing).is_empty(), "{:?}", elements(&drawing));
    assert!(
        diag.iter().any(|d| d.code() == codes::PDF_XOBJECT_MISSING),
        "{diag:?}"
    );
}

/// 文書を読まず、`pdf.document.unsupported` を返すか
#[cfg(test)]
fn unsupported(objects: Vec<Vec<u8>>) -> bool {
    let mut document = Document::new(vec![Page::new("0 0 m 10 10 l S")]);
    document.objects = objects;
    let src = document.bytes();
    let mut budget = Budget::default();
    let mut diag = Diagnostics::new();
    let cancel = CancelToken::new();
    let file = testkit::read::<Pdf>(&src).0.unwrap();
    let result = file.lift(&mut Cx::new(&mut budget, &mut diag, &cancel));
    let codes: Vec<_> = diag.iter().map(|d| d.code()).collect();
    match result {
        Err(Error::Unsupported) => {
            assert_eq!(codes, [codes::PDF_DOCUMENT_UNSUPPORTED]);
            true
        }
        result => {
            assert!(result.is_ok(), "{result:?}");
            assert!(
                !codes.contains(&codes::PDF_DOCUMENT_UNSUPPORTED),
                "{codes:?}"
            );
            false
        }
    }
}

#[test]
fn refuses_values_that_panic_or_differ_by_platform() {
    let with = |dict: &str| vec![common::stream(dict, "x")];
    // 予測子の行の長さが 0 になるか、u8 であふれるか、1 行が大きすぎる。hayro-syntax は、1 のほかの値（0 を含む）で
    // 予測子を当て、0 で割ってパニックする
    for params in [
        "<< /Predictor 2 /Columns 0 >>",
        "<< /Predictor 2 /Colors 0 >>",
        "<< /Predictor 0 /Columns 0 >>",
        "<< /Predictor 0 /Colors 16 /BitsPerComponent 16 >>",
        "<< /Predictor 0.5 /Colors 0 >>",
        "<< /Predictor 2 /Columns 536870912 >>",
        "<< /Predictor 0 /Columns 536870912 >>",
        "<< /Predictor 2 /Columns 2305843009213693952 >>",
        "<< /Predictor 12 /Colors 32 /BitsPerComponent 16 >>",
        "<< /Predictor 2.5 /Columns 0.5 >>",
        "<< /Predictor 2 /Colors 255 /BitsPerComponent 1 /Columns 16777216 >>",
        "<< /Predictor 12 /Colors 4 /BitsPerComponent 16 /Columns 131073 >>",
    ] {
        let dict = format!("/Filter /FlateDecode /DecodeParms {params}");
        assert!(unsupported(with(&dict)), "{params}");
    }
    // 配列の項目と、短い名前
    assert!(unsupported(with(
        "/Filter [/AHx /Fl] /DecodeParms [null << /Predictor 2 /Columns 0 >>]"
    )));
    assert!(unsupported(with(
        "/F /Fl /DP << /Predictor 10 /Colors 0 >>"
    )));
    // 32 ビットの環境で読めない Columns（CCITT は予測子によらず読む）・N・First
    assert!(unsupported(with(
        "/Filter /CCITTFaxDecode /DecodeParms << /Columns 4294967296 >>"
    )));
    assert!(unsupported(with("/N 4294967299 /Alternate /DeviceRGB")));
    assert!(unsupported(with("/Type /ObjStm /N 1 /First 4294967296")));
    assert!(unsupported(with("/Type /ObjStm /N 2147483648 /First 0")));
    // 値の参照は、すべての定義の数を候補にする
    let parted = |params: &str| {
        vec![
            params.as_bytes().to_vec(),
            common::stream("/Filter /FlateDecode /DecodeParms 3 0 R", "x"),
            b"0".to_vec(),
        ]
    };
    assert!(unsupported(parted("<< /Predictor 2 /Columns 0 >>")));
    assert!(unsupported(parted("<< /Predictor 2 /Columns 5 0 R >>")));
    assert!(unsupported(parted("<< /Predictor 5 0 R /Columns 0 >>")));
    assert!(!unsupported(parted("<< /Predictor 12 /Columns 4 >>")));
    // 定義のない参照は、hayro-syntax も既定の値で展開する
    assert!(!unsupported(with(
        "/Filter /FlateDecode /DecodeParms 99 0 R"
    )));
    // ふつうの値と、予測子のない値
    for params in [
        "<< /Predictor 12 /Colors 3 /Columns 5 >>",
        "<< /Predictor 12 /Colors 4 /BitsPerComponent 16 /Columns 131072 >>",
        "<< /Predictor 1 /Columns 536870912 >>",
        "<< /Predictor 300 /Columns 0 >>",
        "<< /Predictor 2 /Columns -5 >>",
    ] {
        let dict = format!("/Filter /FlateDecode /DecodeParms {params}");
        assert!(!unsupported(with(&dict)), "{params}");
    }
}

#[test]
fn refuses_values_written_anywhere() {
    // hayro-syntax は、表が指す位置の定義と、どこに書いたストリームも読むので、辞書の中かを見ずに名前を見る
    let bad = "<< /Filter /FlateDecode /DecodeParms << /Predictor 2 /Columns 0 >> >>";
    for object in [
        // 文字列の中、ストリームの中、辞書の中の `endobj` の後ろ、直接書いたストリーム
        format!("({bad})"),
        String::from_utf8(common::stream("", bad)).unwrap(),
        format!("<< /Junk endobj /A {bad} >>"),
        format!("<< /Contents {bad}\nstream\nx\nendstream >>"),
        // 名前の `#xx` は解く
        "<< /Pr#65dictor 2 /Columns 0 >>".to_owned(),
        // ICCBased を色空間の配列に直接書く
        "[/ICCBased << /N 4294967299 /Alternate /DeviceRGB >>]".to_owned(),
        // オブジェクトのストリームの辞書を、間接オブジェクトの外に書く。hayro-syntax は、表を作り直すときに展開する
        "(<< /Type /ObjStm /N 1 /First 4 >>)".to_owned(),
    ] {
        assert!(unsupported(vec![object.clone().into_bytes()]), "{object}");
    }
    // 参照の先は、ストリームの中に書いた定義も候補にする
    assert!(unsupported(vec![
        b"<< /Predictor 2 /Columns 9 0 R >>".to_vec(),
        common::stream("", "9 0 obj 0 endobj"),
    ]));
    // ふつうの値
    assert!(!unsupported(vec![
        b"(<< /Predictor 2 /Columns 1 >>)".to_vec()
    ]));
    assert!(!unsupported(vec![
        b"<< /Predictor 2 /Columns 9 0 R >>".to_vec(),
        common::stream("", "9 0 obj 1 endobj"),
    ]));
}

/// オブジェクトのストリーム。中身は 16 進で書き（`ASCIIHexDecode`）、ファイルのバイト列には見えないようにする。
/// `first` は、展開した中身の最初の中身の位置
#[cfg(test)]
fn container(count: usize, first: usize, data: &str) -> Vec<u8> {
    let hex: String = data.bytes().map(|byte| format!("{byte:02X}")).collect();
    common::stream(
        &format!("/Type /ObjStm /N {count} /First {first} /Filter /ASCIIHexDecode"),
        format!("{hex}>"),
    )
}

/// 中身が 1 つのオブジェクトのストリーム。中身は、最初の `<` から
#[cfg(test)]
fn one_member(data: &str) -> Vec<u8> {
    container(1, data.find('<').unwrap_or(data.len()), data)
}

#[test]
fn refuses_object_streams_that_differ_by_platform() {
    // 中の位置が、32 ビットの環境で読めないか、足すとあふれる
    assert!(unsupported(vec![one_member("30 2147483648 << /A 1 >>")]));
    assert!(unsupported(vec![one_member("30 4294967296 << /A 1 >>")]));
    // 中に書いたストリームと辞書。hayro-syntax は読むので、展開した中身の名前も見る
    for data in [
        "30 0 << /Length 1 /DecodeParms << /Predictor 2 /Columns 0 >> >>\nstream\nx\nendstream",
        "30 0 << /Predictor 2 /Columns 0 >>",
    ] {
        assert!(unsupported(vec![one_member(data)]), "{data}");
    }
    // 参照の先は、中の数も候補にする
    let inside = |value: &str, object: &str| {
        vec![
            container(1, 5, &format!("31 0 {value}")),
            object.as_bytes().to_vec(),
        ]
    };
    assert!(unsupported(inside("4294967299", "<< /N 31 0 R >>")));
    assert!(unsupported(inside(
        "0",
        "<< /Predictor 2 /Columns 31 0 R >>"
    )));
    assert!(!unsupported(inside("3", "<< /N 31 0 R >>")));
    // 長さと展開の値が参照のもの。読む前に hayro-syntax と同じには展開できない
    for dict in [
        "/Type /ObjStm /N 1 /First 4 /Length 9 0 R",
        "/Type /ObjStm /N 1 /First 4 /Filter 9 0 R",
        "/Type /ObjStm /N 1 /First 4 /DecodeParms [null 9 0 R]",
        "/Type /ObjStm /N 9 0 R /First 4",
        // 入れ子の深い値と、行末で区切った参照。値はバイト列として見るので、深さと空白の字によらない
        "/Type /ObjStm /N 1 /First 4 /DP [[[[[[[[[[[[[[[[[[[[9 0 R]]]]]]]]]]]]]]]]]]]]",
        "/Type /ObjStm /N 1 /First 4 /DecodeParms << /K 9 0\nR >>",
        // 文字列の中の `R` も、参照として扱う（多めに断るだけ）
        "/Type /ObjStm /N 1 /First 4 /DecodeParms << /K (R) >>",
    ] {
        let mut object = format!("<< {dict} >>\nstream\n30 0 << >>\nendstream");
        if !dict.contains("/Length") {
            object = format!("<< /Length 10 {dict} >>\nstream\n30 0 << >>\nendstream");
        }
        assert!(unsupported(vec![object.into_bytes()]), "{dict}");
    }
    // 名前の中の `R` は、参照でない
    assert!(!unsupported(vec![common::stream(
        "/Type /ObjStm /N 1 /First 4 /DecodeParms << /Rx /R >>",
        "30 0 << >>"
    )]));
    // ふつうの中身と、負の位置（どの環境でも読まない）
    assert!(!unsupported(vec![one_member("30 0 << /A 1 >>")]));
    assert!(!unsupported(vec![one_member(
        "30 -1 << /A 1 >>\nstream\nx\nendstream"
    )]));
}

#[test]
fn reads_the_members_of_object_streams_in_linear_work() {
    use zumen_base::Kind;
    // 読めない中身（閉じない文字列）が並んでも、中身ごとに読み直さない
    let count = 50_000;
    let header: String = (0..count)
        .map(|index| format!("{index} {} ", index * 3))
        .collect();
    let data = format!("{header}{}", "<<(".repeat(count));
    let mut document = Document::new(vec![Page::new("0 0 m 10 10 l S")]);
    document.objects = vec![container(count, header.len(), &data)];
    let work = used(&document.bytes(), Kind::Work);
    assert!(work < 4 * count as u64, "{work}");
}

#[test]
fn counts_the_dictionaries_of_object_streams_before_reading_them() {
    use zumen_base::Kind;
    // hayro-syntax は、ストリームを読むと、辞書のキーの表を作る。オブジェクトのストリームになりうるストリームの辞書は、
    // 表を作る前に、キーの数を数える
    let count = 20_000;
    let keys: String = (0..count).map(|index| format!("/K{index} 0 ")).collect();
    let mut document = Document::new(vec![Page::new("0 0 m 10 10 l S")]);
    document.objects = vec![common::stream(
        &format!("/Type /ObjStm /N 1 /First 4 {keys}"),
        "30 0 << >>",
    )];
    let own = used(&document.bytes(), Kind::DecodedBytes);
    assert!(own >= 176 * count as u64, "{own}");
    // 展開するときに hayro-syntax が表を作る `F`・`DP`・`DecodeParms` の値も、展開の前に名前の数で数える
    for key in ["F", "DP", "DecodeParms"] {
        document.objects = vec![common::stream(
            &format!("/Type /ObjStm /N 1 /First 4 /{key} << {keys}>>"),
            "30 0 << >>",
        )];
        let parameters = used(&document.bytes(), Kind::DecodedBytes);
        assert!(parameters >= 176 * count as u64, "{key} {parameters}");
    }
    // ほかの辞書と、展開に使わない値の中の辞書と配列は、確かめが表を作らないので数えない
    document.objects = vec![
        common::stream(
            &format!(
                "/Type /ObjStm /N 1 /First 4 /Foo << {keys}>> /DP [{}]",
                "0 ".repeat(count)
            ),
            "30 0 << >>",
        ),
        format!("<< {keys}>>").into_bytes(),
    ];
    let values = used(&document.bytes(), Kind::DecodedBytes);
    assert!(values < own / 10, "{values} {own}");
}

#[test]
fn bounds_flate_object_streams_before_allocating_the_whole_output() {
    use std::io::Write;
    use zumen_base::{Kind, Limit, Limits, Stop};

    // 2 MiB だけを作り、小さい予算で確かめる。大きなメモリを使わずに、以前の展開後の確認と区別できる
    let mut plain = b"30 0 null ".to_vec();
    plain.resize(2 * 1024 * 1024, b' ');
    let mut encoder = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::best());
    encoder.write_all(&plain).unwrap();
    let compressed = encoder.finish().unwrap();
    assert!(compressed.len() < 4096);
    for filter in [
        "/Filter /FlateDecode",
        "/F /Fl",
        "/Filter [/FlateDecode]",
        "/Filter [/Unknown /FlateDecode]",
        "/Filter /FlateDecode /DecodeParms << /Predictor 12 /Columns 1 >>",
    ] {
        for fields in [
            "/N 1 /First 5",
            "/N 0 /First 0",
            "/N -1 /First 0",
            "/N /Bad /First 0",
        ] {
            let mut document = Document::new(vec![Page::new("0 0 m 10 10 l S")]);
            document.objects = vec![common::stream(
                &format!("/Type /ObjStm {fields} {filter}"),
                &compressed,
            )];
            let file = testkit::read::<Pdf>(&document.bytes()).0.unwrap();
            let run = || {
                let limits = Limits::default().with(Kind::DecodedBytes, Limit::Finite(32 * 1024));
                let mut budget = Budget::new(limits);
                let mut diagnostics = Diagnostics::new();
                let cancel = CancelToken::new();
                let result = file.lift(&mut Cx::new(&mut budget, &mut diagnostics, &cancel));
                assert!(
                    matches!(result,
                    Err(Error::Stopped(Stop::Exceeded(e)))
                        if e.kind == Kind::DecodedBytes && e.requested < 48 * 1024),
                    "{fields} {filter}: {result:?}"
                );
                let diagnostics = diagnostics.into_vec();
                assert_eq!(diagnostics[0].code(), codes::BUDGET_DECODED_BYTES_EXCEEDED);
                format!("{result:?} {budget:?} {diagnostics:?}")
            };
            assert_eq!(run(), run());
        }
    }
}

#[test]
fn keeps_flate_object_stream_contents_with_damage_and_predictors() {
    let plain = b"30 0 null ";
    let good = common::zlib(plain);
    let mut wrong = good.clone();
    *wrong.last_mut().unwrap() ^= 0xff;
    let mut predictor = vec![0];
    predictor.extend(plain);
    for (filter, data) in [
        ("/Filter /FlateDecode", good.clone()),
        ("/F /Fl", good[2..good.len() - 4].to_vec()),
        ("/Filter /FlateDecode", wrong),
        ("/Filter /FlateDecode", good[..good.len() - 4].to_vec()),
        ("/Filter /FlateDecode", good[..good.len() - 7].to_vec()),
        (
            "/Filter [/FlateDecode] /DecodeParms [<< /Predictor 12 /Columns 10 >>]",
            common::zlib(&predictor),
        ),
    ] {
        let mut document = Document::new(vec![Page::new("0 0 m 10 10 l S")]);
        document.objects = vec![common::stream(
            &format!("/Type /ObjStm /N 1 /First 5 {filter}"),
            data,
        )];
        let source = document.bytes();
        let (drawing, diagnostics) = lift(&source);
        assert_eq!(elements(&drawing).len(), 1, "{filter}");
        assert!(diagnostics.is_empty(), "{filter}: {diagnostics:?}");
        assert_eq!(drawing, lift(&source).0);
    }
}

#[test]
fn refuses_filter_arrays_longer_than_the_limit() {
    // hayro-syntax は、展開するたびに、フィルターの数だけ `DecodeParms` の配列の項目を辞書として読み、空の辞書でも
    // 1 KiB ほどの表を持つ。名前と参照の項目が 64 を超えるフィルターの配列は、読まない
    let names = |count: usize| "/AHx ".repeat(count);
    assert!(!unsupported(vec![common::stream(
        &format!("/Filter [{}]", names(64)),
        "x"
    )]));
    for dict in [
        format!("/Filter [{}]", names(65)),
        format!("/F [{}]", names(65)),
        format!("/Filter [{}]", "9 0 R ".repeat(65)),
        // コメントの中の配列は、前の配列が範囲の終わりから読んだ並びの、項目の数も覚えて足す
        format!("/Filter [%/Filter [/X\n{}]", "/X ".repeat(64)),
    ] {
        assert!(unsupported(vec![common::stream(&dict, "x")]), "{dict}");
    }
    assert!(!unsupported(vec![common::stream(
        &format!("/Filter [%/Filter [\n{}]", "/X ".repeat(64)),
        "x"
    )]));
    // 参照の先の配列も、同じく数える
    let target = |items: String| {
        vec![
            common::stream("/Filter 4 0 R", "x"),
            format!("[{items}]").into_bytes(),
        ]
    };
    assert!(!unsupported(target(names(64))));
    assert!(unsupported(target(names(65))));
    // 参照の項目は、名前に解けるとき（参照の先になりうる位置に名前があるとき）だけ数える。hayro-syntax は、名前に
    // 解けない参照の項目で止まる（添付ファイルの `/F n 0 R` と、注釈への参照を並べた配列など）
    let references = "9 0 R ".repeat(65);
    assert!(!unsupported(target(references.clone())));
    let mut named = target(references.clone());
    named.push(b"/AHx".to_vec());
    assert!(unsupported(named));
    let mut inside = target(references);
    inside.push(container(1, 5, "31 0 /AHx"));
    assert!(unsupported(inside));
}

#[test]
fn refuses_parameter_arrays_with_references() {
    // hayro-syntax は、展開のパラメーターの配列の参照の項目を、解くたびに辞書を読み直して表を作る。同じ大きな辞書を
    // 指す参照を並べると、その表をフィルターの数だけ持つので、フィルターの数より前に上の段の参照の項目が 2 つ以上ある
    // 配列は、読まない
    let with = |filters: usize, parameters: &str| {
        vec![
            common::stream(
                &format!("/Filter [{}] {parameters}", "/RL ".repeat(filters)),
                "x",
            ),
            b"<< /K 1 >>".to_vec(),
        ]
    };
    for (filters, parameters) in [
        (2, "/DecodeParms [4 0 R 4 0 R]".to_string()),
        (3, "/DP [null 4 0 R %\n4 0 R]".to_string()),
        (
            64,
            format!("/DecodeParms [{}4 0 R 4 0 R]", "null ".repeat(62)),
        ),
        // 辞書の後ろに `stream` がある項目は、hayro-syntax が中身を `Length` で飛ばすので、後ろの項目が分からない
        (
            64,
            "/DP [<< /Length 1 >> stream\n%endstream 4 0 R 4 0 R\n]".to_string(),
        ),
        (
            64,
            "/DP [<< /Length 3 >> stream\n(()\nendstream 4 0 R 4 0 R ())]".to_string(),
        ),
        (2, "/DP [null << /Length 1 >> %\nstream\n%\n]".to_string()),
        // 見るバイト数の上限で切れた項目も、後ろの項目が分からない
        (2, format!("/DP [({}) 4 0 R]", "a".repeat(1_100))),
        // 上限の終わりにかかる参照（2 つ目の `R` が外）は、数 2 つに読むが、参照の項目として数える
        (3, format!("/DP [4 0 R ({}) 4 0 R]", "a".repeat(1_011))),
        // 上限の最後のバイトが参照の世代番号の符号（`+` は、数字も `.` もなければ読めない）でも、参照の項目として数える
        (3, format!("/DP [4 0 R ({}) 4 +0 R]", "a".repeat(1_012))),
        (3, format!("/DP [4 0 R ({}) 4 +. R]", "a".repeat(1_012))),
        (3, format!("/DP [4 0 R ({}) .+0 R]", "a".repeat(1_013))),
        // 64 個目までの項目にかかる参照も、後ろの 2 つまで読んで数える
        (
            64,
            format!(
                "/DP [4 0 R ({}) {}4 0 R]",
                "a".repeat(706),
                "null ".repeat(61)
            ),
        ),
        (
            64,
            format!(
                "/DP [4 0 R ({}) {}4 0 R 4 0 R]",
                "a".repeat(711),
                "null ".repeat(60)
            ),
        ),
        // 上限の終わりにかかる `stream` の語（`str` までが中）も、辞書の項目をストリームとする
        (
            3,
            format!(
                "/DP [4 0 R << /Length 1 /P ({}) >> stream\n%\nendstream 4 0 R]",
                "a".repeat(993)
            ),
        ),
        // 数字のない数の字句は、hayro-syntax の読み方が数字の前の符号と `.` までを読み、後ろを参照として読む
        (64, "/DP [.-0 0 R .-0 0 R]".to_string()),
        (64, "/DP [--0 0 R --0 0 R]".to_string()),
        (64, "/DP [-.-0 0 R +.-0 0 R]".to_string()),
    ] {
        assert!(unsupported(with(filters, &parameters)), "{parameters}");
    }
    // 参照が 1 つのものと、辞書の中の参照と、フィルターの数より後ろの項目（フィルターの数の上限より後ろも）
    for (filters, parameters) in [
        (64, "/DecodeParms [4 0 R]".to_string()),
        (
            64,
            "/DecodeParms [null << /JBIG2Globals 4 0 R >> << /K 4 0 R >>]".to_string(),
        ),
        (
            64,
            format!("/DecodeParms [{}4 0 R 4 0 R]", "null ".repeat(63)),
        ),
        (1, "/DecodeParms [4 0 R 4 0 R]".to_string()),
        (2, "/DP [null 4 0 R %\n4 0 R]".to_string()),
        (1, "/DP [null << /Length 1 >> stream\n%\n]".to_string()),
        (64, format!("/DP [({}) 4 0 R]", "a".repeat(900))),
        (64, "/DP [.-0 0 R]".to_string()),
        // hayro-syntax が読めない項目（飛ばせても数として読めない字句）の後ろは、読まない（`[` の後ろからファイルの
        // 終わりまでが、見る上限より短い形。長ければ、切れた項目と見分けないので断る）
        (64, "/DP [5-5.5 4 0 R 4 0 R]".to_string()),
        // 入れ子の配列は、ファイルの飛ばし方で区切る（`R` の後ろに続く `+` は、内容のストリームの飛ばし方なら名前の
        // ような字句として読めるが、ファイルの飛ばし方では読めない）
        (64, "/DP [[4 0 R+] 4 0 R 4 0 R]".to_string()),
    ] {
        assert!(!unsupported(with(filters, &parameters)), "{parameters}");
    }
    // `[` の後ろからファイルの終わりまでが見る上限より長ければ（ふつうのファイルは当たる）、読めない項目も、切れた
    // 項目と見分けないので断る（多めに断るだけ）
    let mut long = with(64, "/DP [5-5.5 4 0 R 4 0 R]");
    long.push(format!("({})", "a".repeat(2_000)).into_bytes());
    assert!(unsupported(long));
    // 配列ごとの、いちばん前の項目で決める（前に見た配列の項目がフィルターの数より後ろでも）
    assert!(unsupported(vec![
        common::stream("/Filter [/RL /RL] /DP [null 4 0 R 4 0 R]", "x"),
        common::stream("/DP [4 0 R 4 0 R]", "x"),
    ]));
    // フィルターの値が名前なら、hayro-syntax は配列の項目を読まない
    assert!(!unsupported(vec![common::stream(
        "/Filter /RL /DP [4 0 R 4 0 R]",
        "x"
    )]));
    // フィルターの配列は、オブジェクトのストリームの中身にあっても数える
    assert!(unsupported(vec![
        common::stream("/DP [4 0 R 4 0 R]", "x"),
        one_member("30 0 << /Filter [/RL /RL] >>"),
    ]));
    // 値が参照なら、参照の先になりうる位置の配列を見る（参照がオブジェクトのストリームの中身にだけあっても）
    let indirect = |target: &[u8]| {
        vec![
            common::stream("/Filter [/RL /RL] /DP 4 0 R", "x"),
            target.to_vec(),
        ]
    };
    assert!(unsupported(indirect(b"[5 0 R 5 0 R]")));
    assert!(unsupported(indirect(
        b"[<< /Length 1 >> stream\n%endstream 5 0 R 5 0 R\n]"
    )));
    // `.-0 0 R` は hayro-syntax の項目の 2 つなので、2 つ目の参照は 4 つ目の項目
    let hidden = |filters: &str| {
        vec![
            common::stream(&format!("/Filter [{filters}] /DP 4 0 R"), "x"),
            b"[.-0 0 R .-0 0 R]".to_vec(),
        ]
    };
    assert!(unsupported(hidden("/RL /RL /RL /RL")));
    assert!(!unsupported(hidden("/RL /RL /RL")));
    // 参照の先の配列でも、上限の終わりにかかる参照を数える
    let far = |filters: usize, items: String| {
        vec![
            common::stream(
                &format!("/Filter [{}] /DP 4 0 R", "/RL ".repeat(filters)),
                "x",
            ),
            format!("[{items}]").into_bytes(),
        ]
    };
    assert!(unsupported(far(
        64,
        format!("5 0 R ({}) {}5 0 R", "a".repeat(706), "null ".repeat(61))
    )));
    assert!(unsupported(far(
        3,
        format!("5 0 R ({}) 5 +0 R", "a".repeat(1_012))
    )));
    // `+` で始まらない読めずに切れた項目は、その番号で断る（CID フォントの `W` のような長い入れ子の配列の前の項目は、
    // hayro-syntax の参照にならない）
    let widths = |items: String| {
        vec![
            common::stream("/Filter [/RL] /DP 4 0 R", "x"),
            b"<< /K 1 >>".to_vec(),
            format!("[{items}]").into_bytes(),
        ]
    };
    assert!(!unsupported(widths(format!("1 [{}]", "500 ".repeat(300)))));
    assert!(!unsupported(widths(format!("1 ({})", "a".repeat(1_100)))));
    assert!(unsupported(widths(format!("[{}]", "500 ".repeat(300)))));
    assert!(!unsupported(indirect(b"[5 0 R]")));
    assert!(!unsupported(indirect(b"<< /K 1 >>")));
    assert!(unsupported(vec![
        b"[5 0 R 5 0 R]".to_vec(),
        container(1, 5, "32 0 << /Filter [/RL /RL] /DP 3 0 R >>"),
    ]));
    // 参照の先の配列は、フィルターの配列がなければ、展開のパラメーターの配列として読まれない（注釈への参照を並べた
    // 配列など）。フィルターの値に参照があれば、参照の先の配列の名前の項目も、フィルターの数に入る
    let annotations = |filter: &str| {
        vec![
            common::stream(&format!("/Filter {filter} /DecodeParms 6 0 R"), "x"),
            b"[7 0 R 7 0 R]".to_vec(),
        ]
    };
    assert!(!unsupported(annotations("/FlateDecode")));
    assert!(!unsupported(annotations("[/FlateDecode]")));
    assert!(unsupported(annotations("[/FlateDecode /FlateDecode]")));
    let mut chained = annotations("9 0 R");
    chained.push(b"[/RL /RL]".to_vec());
    assert!(unsupported(chained));
}

#[test]
fn refuses_object_streams_that_only_the_rebuilt_table_reads() {
    // hayro-syntax は、表を作り直すとき、字句ごとに見て、型が `ObjStm` のどの辞書も展開する。確かめたストリームの
    // 上の段の型でない型は、読まない
    for object in [
        // 間接オブジェクトの辞書の中と、文字列の中
        "<< /X << /Type /ObjStm /N 1 /First 4 /Length 9 >>\nstream\n9 0 [0 0]\nendstream >>",
        "<< /K (<< /Type /ObjStm /N 1 /First 4 /Length 9 >> stream\n9 0 [0 0]\nendstream) >>",
        // 型の前のコメント。ほかの辞書が、コメントの中から同じ型を読みうる
        "<< /Length 9 /N 1 /First 4 %\n/Type /ObjStm >>\nstream\n9 0 [0 0]\nendstream",
        // `N` か `First` のない辞書と、ストリームでない辞書
        "<< /Length 9 /Type /ObjStm /N 1 >>\nstream\n9 0 [0 0]\nendstream",
        "<< /Type /ObjStm /N 1 /First 4 >>",
        // `#xx` で書いた型
        "<< /Length 9 /N 1 /First 4 /Type /Obj#53tm >>\nstream\n9 0 [0 0]\nendstream",
        // 型の前の文字列。名前の終わりの `\` は、文字列の中では次の `(` をエスケープするので、文字列の中から読む辞書と外から
        // 読む辞書が、同じ `)` の後ろの型を上の段に読む
        "[<< /Filter /AHx /X (6 0 obj << /K /a\\(z) /Type /ObjStm /N 1 /First 4 /Length 9 >>\nstream\n9 0 [0 0]\nendstream ]",
        "<< /N 1 /First 4 /A ( << /Filter /AHx /K /a\\(z) /Type /ObjStm /Length 9 >>\nstream\n9 0 [0 0]\nendstream",
        "<< /K (z) /Type /ObjStm /N 1 /First 4 /Length 9 >>\nstream\n9 0 [0 0]\nendstream",
    ] {
        assert!(unsupported(vec![object.as_bytes().to_vec()]), "{object}");
    }
    // 辞書の始まりの行の、前のコメント（コメントの中の `obj` から読む）
    let mut object = b"%obj ".to_vec();
    object.extend(one_member("30 0 << /A 1 >>"));
    assert!(unsupported(vec![object]));
    // 型が、ほかのキーの後ろにあるもの
    assert!(!unsupported(vec![
        b"<< /Length 9 /N 1 /First 4 /Type /ObjStm >>\nstream\n9 0 [0 0]\nendstream".to_vec()
    ]));
}

#[test]
fn refuses_object_streams_of_encrypted_documents() {
    // 暗号化の辞書の名前があると、オブジェクトのストリームの中身を読み込む前に復号できないので、オブジェクトの
    // ストリームになりうるもの（型が参照の辞書も、hayro-syntax は表を作り直すときに解く）を持つ文書は読まない
    let encrypted =
        |object: &[u8]| unsupported(vec![b"<< /Encrypt (x) >>".to_vec(), object.to_vec()]);
    assert!(encrypted(&one_member("30 0 << /A 1 >>")));
    assert!(encrypted(b"(<< /Type /ObjStm >>)"));
    assert!(encrypted(b"<< /Type 9 0 R >>"));
    assert!(!encrypted(b"<< /Type /Font >>"));
    assert!(!unsupported(vec![one_member("30 0 << /A 1 >>")]));
}

#[test]
fn refuses_coded_filters_whose_input_may_be_large() {
    // hayro-syntax は、LZW の符号を読む位置と ASCII85 の出力の大きさを、環境の大きさの整数で求める。前のフィルターが
    // どこまでも長くしうる入力の LZW と ASCII85 は、読まない
    for dict in [
        "/Filter [/FlateDecode /LZWDecode]",
        "/Filter [/Fl /A85]",
        "/F [/RL /LZW]",
        "/Filter [/LZWDecode /LZWDecode]",
        "/Filter [/Crypt /Fl /LZW]",
        "/Filter [3 0 R /LZWDecode]",
        // 配列の中の `F`・`Filter` は、hayro-syntax が知らない名前として飛ばす。後ろの名前を、そのキーの値としても読む
        "/Filter [/FlateDecode /F /LZWDecode]",
        "/Filter [/Fl /Filter /A85]",
    ] {
        assert!(unsupported(vec![common::stream(dict, "x")]), "{dict}");
    }
    // フィルターの値に参照があれば、参照の先になりうる位置（`obj` の後ろと、オブジェクトのストリームの中の位置）の
    // LZW・ASCII85 の名前は、倍率の分からないフィルターとして当たりうる。フィルターの値として読める名前も同じ（コメントの
    // 中の `4 0 obj` の後ろの名前は、`[/Fl 4 0 R]` の参照の先になる）。参照の先の配列は、参照の項目の後ろも見る
    for filter in ["/Filter 4 0 R", "/F [/AHx 4 0 R]", "/Filter [/Fl 4 0 R]"] {
        for target in [
            b"/LZWDecode".to_vec(),
            b"[/AHx 5 0 R /A85]".to_vec(),
            common::stream("/Filter %4 0 obj\n/A85", "x"),
            container(1, 5, "31 0 /LZW"),
            // 配列の中の `/Filter` の値として確かめた名前も、配列が参照の先なら倍率が分からない
            b"[/Fl %\n/Filter /LZWDecode]".to_vec(),
        ] {
            let text = String::from_utf8_lossy(&target).into_owned();
            assert!(
                unsupported(vec![common::stream(filter, "x"), target]),
                "{filter} {text}"
            );
        }
        // 参照の先になりえない所の名前は、hayro-syntax が当てない（添付ファイルの `/F n 0 R` と、LZW の画像など）
        for other in [
            b"(/A85)".to_vec(),
            b"[(x) /LZW]".to_vec(),
            common::stream("/Filter /LZWDecode", "x"),
        ] {
            let text = String::from_utf8_lossy(&other).into_owned();
            assert!(
                !unsupported(vec![common::stream(filter, "x"), other]),
                "{filter} {text}"
            );
        }
    }
    // フィルターの値の参照が、オブジェクトのストリームの中身にだけあるもの。参照の先の位置は、参照を見つけた順に
    // よらず見る（ファイルの `obj` の後ろと、前のコンテナーと同じコンテナーの中の位置）
    let inner = || container(1, 5, "32 0 << /Filter [/Fl 3 0 R] >>");
    for objects in [
        vec![b"/LZWDecode".to_vec(), inner()],
        vec![b"[/FlateDecode /LZWDecode]".to_vec(), inner()],
        vec![container(1, 5, "31 0 /LZW"), inner()],
        vec![container(2, 10, "31 0 32 5 /LZW << /Filter 31 0 R >>")],
    ] {
        let text = String::from_utf8_lossy(&objects.concat()).into_owned();
        assert!(unsupported(objects), "{text}");
    }
    // 範囲の終わりから読んだ並びの覚えに当たっても、その前の項目の参照を落とさない
    assert!(unsupported(vec![
        common::stream("/F [/AHx %/F [/Fl %/F [/Fl 4 0 R\n/AHx]", "x"),
        b"/LZWDecode".to_vec(),
    ]));
    // 配列の参照の項目は、名前に解ければ後ろの項目も読むので、後ろの名前の倍率は分からない
    for dict in ["/Filter [/AHx 4 0 R /LZW]", "/F [4 0 R /A85]"] {
        assert!(unsupported(vec![common::stream(dict, "x")]), "{dict}");
    }
    // 参照がなければ、hayro-syntax はフィルターとして当てない
    assert!(!unsupported(vec![b"(/LZWDecode)".to_vec()]));
    assert!(!unsupported(vec![container(1, 5, "31 0 /LZW")]));
    assert!(!unsupported(vec![
        common::stream("/Filter [[/LZWDecode]]", "x"),
        common::stream("/Filter [/AHx 4 /A85]", "x"),
    ]));
    // 入力の長さが、ファイルの長さの定数倍に収まるもの
    for dict in [
        "/Filter /LZWDecode",
        "/Filter [/LZW]",
        "/Filter [/A85 /LZW]",
        "/Filter [/AHx /A85]",
        "/Filter [/Crypt /LZWDecode]",
        "/Filter [/Unknown /LZWDecode]",
        "/F /A85",
        "/Filter %\n/LZWDecode",
    ] {
        assert!(!unsupported(vec![common::stream(dict, "x")]), "{dict}");
    }
}

#[test]
fn reads_names_in_comments_in_linear_work() {
    use zumen_base::Kind;
    // コメントの行の中の名前と `obj` から、後ろのコメントの行を読み直さない
    let count = 20_000;
    let plain = used(
        &Document::new(vec![Page::new("0 0 m 10 10 l S")]).bytes(),
        Kind::Work,
    );
    for line in [
        "%/N\n",
        "%/N \n",
        "%/A\n",
        "%obj\n",
        "%/Type\n",
        "%/Filter [\n",
        // 字句の途中の `obj`。数は、hayro-syntax が読む字だけを読み、字句の残りを読み直さない
        "obj0",
    ] {
        let mut document = Document::new(vec![Page::new("0 0 m 10 10 l S")]);
        document.objects = vec![format!("null\n{}", line.repeat(count)).into_bytes()];
        let work = used(&document.bytes(), Kind::Work);
        assert!(work < plain + 8 * count as u64, "{line:?} {plain} {work}");
    }
    // コメントの行の中のキーと `obj` は、同じ値の位置に飛ぶ。長い値を、キーと `obj` ごとに読み直さない
    let digits = "0".repeat(100_000);
    let name = format!("/{}", "a".repeat(100_000));
    let dict = format!("<<{}>>", " /A 1".repeat(20_000));
    // 辞書の後ろを飛ばしても、`obj` の側の覚えを変えない
    let stream = "<</N 1/First 0/Length 0>>stream\n\nendstream".to_string();
    // コメントの行の配列は、どれも後ろの同じ項目の並びを読む。配列の項目を飛ばしても、キーの側の覚えを変えない
    // （範囲の終わりの配列も、違う値のキーと交互に並べた配列も）。並びは、フィルターの配列の項目の上限（64）より短く
    // して、読む文書にする
    let list = "/a ".repeat(60);
    let few = "/a ".repeat(10);
    let short = "[/a]".to_string();
    let long = format!("[{list}]");
    for (line, value) in [
        ("%/Type\n", &digits),
        ("%/N\n", &digits),
        ("%/Type\n", &name),
        ("%/Filter\n", &name),
        ("%obj\n", &digits),
        ("%obj\n", &dict),
        ("%obj\n", &stream),
        ("%/Filter [\n", &list),
        ("%/DP [\n", &list),
        ("%/F [/AHx\n", &list),
        ("%/Filter [\n%/N\n", &few),
        ("%/F [\n%/F\n", &short),
        ("%/Filter\n%/F x\n", &short),
        ("%/Filter\n%/F [/x]\n", &long),
    ] {
        let mut document = Document::new(vec![Page::new("0 0 m 10 10 l S")]);
        document.objects = vec![format!("[\n{}{value}]", line.repeat(count)).into_bytes()];
        let work = used(&document.bytes(), Kind::Work);
        let most = plain + 8 * count as u64 + value.len() as u64;
        assert!(work < most, "{line:?} {plain} {work}");
    }
    // コメントの行の `obj<<` から読む辞書は、どれも `<<` の後ろで同じ位置に着き、後ろの同じ辞書を読む
    let tail = format!(
        "{}/N 1/First 0/Length 0>>stream\n\nendstream",
        " /A 1".repeat(20_000)
    );
    let mut document = Document::new(vec![Page::new("0 0 m 10 10 l S")]);
    document.objects = vec![format!("({}{tail})", "%obj<<\n".repeat(count)).into_bytes()];
    let work = used(&document.bytes(), Kind::Work);
    let most = plain + 8 * count as u64 + tail.len() as u64;
    assert!(work < most, "{plain} {work}");
    // フィルターの値に参照があるときに見る、参照の先の配列も、コメントの行の `obj [` から読み直さない（名前の配列と、
    // 展開のパラメーターの配列の 2 つの見方で、それぞれ飛ばす）
    let mut document = Document::new(vec![Page::new("0 0 m 10 10 l S")]);
    document.objects = vec![
        b"<< /F 9 0 R >>".to_vec(),
        format!("[\n{}{list}]", "%obj [\n".repeat(count)).into_bytes(),
    ];
    let work = used(&document.bytes(), Kind::Work);
    let most = plain + 10 * count as u64 + list.len() as u64;
    assert!(work < most, "{plain} {work}");
}

#[test]
fn counts_reads_that_grow_faster_than_the_file() {
    use zumen_base::Kind;
    // 中の位置が長い数の途中を指せば、位置ごとに数の終わりまで読む
    let count = 1_000;
    let header: String = (0..count)
        .map(|index| format!("{index} {index} "))
        .collect();
    let data = format!("{header}{}", "1".repeat(100_000));
    let mut document = Document::new(vec![Page::new("0 0 m 10 10 l S")]);
    document.objects = vec![container(count, header.len(), &data)];
    let work = used(&document.bytes(), Kind::Work);
    assert!(work >= count as u64 * 99_000 / 4, "{work}");
    // `Length` が合わないストリームは、hayro-syntax が `endstream` を探して、後ろを読む
    let line = "9 0 obj << /N 1 /First 0 /Length 0 >>\nstream\n";
    let mut document = Document::new(vec![Page::new("0 0 m 10 10 l S")]);
    document.objects = vec![format!("({})", line.repeat(count)).into_bytes()];
    let work = used(&document.bytes(), Kind::Work);
    let size = (count * line.len()) as u64;
    assert!(work >= size * count as u64 / 16, "{work}");
    // 展開のパラメーターの配列の項目は、閉じない文字列で飛ばせずに位置が戻っても、見たバイト列（1 KiB まで）を数える
    let mut document = Document::new(vec![Page::new("0 0 m 10 10 l S")]);
    document.objects = vec![format!("null %{}", "/DP [(".repeat(count)).into_bytes()];
    let work = used(&document.bytes(), Kind::Work);
    assert!(work >= (count as u64 - 200) * 1024 / 4, "{work}");
}

#[test]
fn counts_reads_of_values_that_cannot_be_used() {
    use zumen_base::Kind;
    // hayro-syntax は、型が違っても読み飛ばして確かめるので、使えない値も、引くたびに読む
    let big = format!("[{}]", "0 ".repeat(10_000)).into_bytes();
    let many = |calls: &str, resources: &str, objects: Vec<Vec<u8>>| {
        let mut document = Document::new(vec![Page::new(calls).resources(resources)]);
        document.objects = objects;
        used(&document.bytes(), Kind::Work)
    };
    let once = 100 * 20_000 / 4;
    // 資源の名前が、別の型を指す
    for (op, kind) in [
        ("/N Do ", "XObject"),
        ("/N gs ", "ExtGState"),
        ("/N 1 Tf ", "Font"),
        ("/M /N BDC EMC ", "Properties"),
        ("/N sh ", "Shading"),
        ("/N cs ", "ColorSpace"),
        ("/Pattern cs /N scn ", "Pattern"),
    ] {
        let calls = op.repeat(100);
        let plain = many(&calls, "", Vec::new());
        let work = many(
            &calls,
            &format!("/{kind} << /N 3 0 R >>"),
            vec![big.clone()],
        );
        assert!(work >= plain + once, "{op}: {plain} {work}");
    }
    // ストリームとして引くと、直接書いた辞書を読んでから、ストリームでないと分かる
    let calls = "/N Do ".repeat(100);
    let plain = many(&calls, "", Vec::new());
    let direct = format!("/XObject << /N << /A {} >> >>", "0 ".repeat(10_000));
    let work = many(&calls, &direct, Vec::new());
    assert!(work >= plain + once, "{plain} {work}");
    // フォントの名前と種類は、選ぶたびに読む
    let calls = "/F 1 Tf ".repeat(100);
    let resources = "/Font << /F 3 0 R >>";
    let plain = many(
        &calls,
        resources,
        vec![b"<< /Type /Font /Subtype /Type1 /BaseFont /A >>".to_vec()],
    );
    for font in [
        "<< /Type /Font /Subtype /Type1 /BaseFont 4 0 R >>",
        "<< /Type /Font /Subtype 4 0 R /BaseFont /A >>",
    ] {
        let work = many(
            &calls,
            resources,
            vec![font.as_bytes().to_vec(), big.clone()],
        );
        assert!(work >= plain + once, "{font}: {plain} {work}");
    }
    // 直接書いたフォントは、覚えないので、選ぶたびに値を読む
    let direct = "/Font << /F << /Type /Font /Subtype /Type0 /BaseFont /A /Encoding /Identity-H \
                  /DescendantFonts 3 0 R >> >>";
    let work = many(&calls, direct, vec![big.clone()]);
    assert!(work >= plain + once, "{plain} {work}");
    // フォームの資源が、ストリームを指す。hayro-syntax は、ストリームの辞書を資源の辞書として読む
    let calls = "/F Do ".repeat(100);
    let resources = "/XObject << /F 3 0 R >>";
    let form = |extra: &str| common::stream(&format!("{LINE_FORM} {extra}"), "");
    let plain = many(&calls, resources, vec![form("")]);
    for tail in ["\nendstream", ""] {
        let mut stream = b"<< /Length 0 /XObject 5 0 R >>\nstream\n".to_vec();
        stream.extend(tail.as_bytes());
        let objects = vec![form("/Resources 4 0 R"), stream, big.clone()];
        let work = many(&calls, resources, objects);
        assert!(work >= plain + once, "{tail}: {plain} {work}");
    }
    // ストリームの長さが、別の型を指す
    let target = b"<< /Length 5 0 R >>\nstream\n\nendstream".to_vec();
    for length in [
        big.clone(),
        format!("({})", "x".repeat(20_000)).into_bytes(),
        format!("/{}", "x".repeat(20_000)).into_bytes(),
    ] {
        let objects = vec![form("/Matrix 4 0 R"), target.clone(), length];
        let work = many(&calls, resources, objects);
        assert!(work >= plain + 2 * once, "{plain} {work}");
    }
    // カタログからたどるだけのストリームも、作り直しを済ませるときに読む
    let list = (4..104)
        .map(|number| format!("{number} 0 R "))
        .collect::<String>();
    let catalog = format!("/Unknown [{list}]");
    let mut document = Document::new(vec![Page::new("")]);
    document.catalog = &catalog;
    document.objects = vec![big.clone()];
    document
        .objects
        .extend((0..100).map(|_| b"<< /Length 3 0 R >>\nstream\n\nendstream".to_vec()));
    let work = used(&document.bytes(), Kind::Work);
    assert!(work >= once, "{work}");
    // ページの注釈とビューポートの項目が、辞書でない。辞書でない項目から後ろは読まない
    for key in ["/Annots", "/VP"] {
        let list = format!("{key} [3 0 R 3 0 R]");
        let mut document = Document::new(vec![Page::new("").extra(&list); 100]);
        document.objects = vec![big.clone()];
        let work = used(&document.bytes(), Kind::Work);
        assert!(work >= once, "{key}: {work}");
        assert!(work < 2 * once, "{key}: {work}");
    }
    // `endstream` のないストリームは、読むたびに、ファイルの終わりまで探す
    let padding = format!("({})", "x".repeat(100_000)).into_bytes();
    let objects = vec![form("/Foo 4 0 R"), b"<< >>\nstream\n".to_vec(), padding];
    let work = many(&calls, resources, objects);
    assert!(work >= 100 * 100_000 / 4, "{work}");
}

/// 前へ飛んで、値を積む命令のデータの中の `FDEF` に入る関数を、`calls` 回呼ぶプログラム。`jump` がなければ、飛ぶ命令を
/// 別の命令に変える。読み飛ばす本体は、`body` 個の命令
fn hidden_definition(body: usize, calls: usize, jump: bool) -> Vec<u8> {
    let (fdef, endf, pushb, call) = (0x2C, 0x2D, 0xB0, 0x2B);
    let mut program = vec![0xB1, 0, 0, fdef];
    program.extend([0xB2, 1, 1, 2, if jump { 0x1C } else { 0x21 }, pushb, fdef]);
    program.extend(vec![0; body]);
    program.push(endf);
    program.extend([0x58, endf, 0x59]);
    for _ in 0..calls {
        program.extend([pushb, 0, call]);
    }
    program
}

/// `fvar` と定義の表を持つ、命令に頼るフォント
fn hinted_with_axes(program: &[u8], axes: u16, stack: u16) -> Vec<u8> {
    let text: Vec<u8> = "MingLiU"
        .encode_utf16()
        .flat_map(u16::to_be_bytes)
        .collect();
    let mut name = Vec::new();
    for value in [0, 1, 18, 3, 1, 0x409, 1, text.len() as u16, 0] {
        be16(&mut name, value);
    }
    name.extend(&text);
    let mut fvar = Vec::new();
    for value in [1, 0, 16, 2, axes, 20, 0, 0] {
        be16(&mut fvar, value);
    }
    let mut extra = vec![
        (b"fpgm", program.to_vec()),
        (b"name", name),
        (b"prep", program.to_vec()),
    ];
    if axes != 0 {
        extra.push((b"fvar", fvar));
    }
    truetype_with(
        &[Vec::new(), outline_glyph(3)],
        Some([0, 0, 16, 0, stack]),
        extra,
    )
}

#[test]
fn counts_definitions_read_again_through_jumps() {
    use zumen_base::Kind;
    // skrifa は、定義の本体を読み飛ばす命令を上限に数えない。飛ぶ命令があれば、呼ぶたびに定義の命令に入れる。上限の
    // 命令の間に動かせる回数（100 万 ÷ 4）だけ、いちばん長い本体を読み飛ばすとする。1 回に本体の 1 / 16 を、
    // `fpgm` と `prep` と、走査と hayro-interpret の分で 4 回数える
    let document = |body, jump| {
        let font = hinted_with_axes(&hidden_definition(body, 2, jump), 0, 100);
        font_document("/CIDFontType2", "/FontFile2", font, 1, 0)
    };
    let base = 2 * 2 * 1_000_000 * 8;
    let work = used(&document(10_000, true), Kind::Work);
    assert!(work >= base + 4 * 250_000 * 10_000 / 16, "{work}");
    // 飛ぶ命令がなければ、定義の命令は 1 度ずつしか動かない
    let work = used(&document(10_000, false), Kind::Work);
    assert!(work < base + 1_000_000, "{work}");
}

#[test]
fn counts_values_pushed_for_each_axis() {
    use zumen_base::Kind;
    // `GETVARIATION` は、1 命令で、`fvar` の軸の数だけ、スタックに積む。積めるのは、スタックの大きさまで
    let program = [0xB0, 0, 0x21];
    let document = |axes, stack| {
        let font = hinted_with_axes(&program, axes, stack);
        used(
            &font_document("/CIDFontType2", "/FontFile2", font, 1, 0),
            Kind::Work,
        )
    };
    let many = document(60_000, 60_000);
    assert!(many >= 2 * 2 * 1_000_000 * (60_000 / 128), "{many}");
    let plain = document(0, 60_000);
    let small = document(60_000, 100);
    assert!(small < plain, "{small} {plain}");
    assert!(many > plain + 1_000_000_000, "{many} {plain}");
}

#[test]
fn does_not_read_fonts_with_too_large_character_ranges() {
    use zumen_base::Kind;
    let font = |first: u64, last: u64| {
        format!(
            "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica /FirstChar {first} /LastChar {last} /Widths [100] >>"
        )
    };
    let file = |font: &str| {
        let mut document = Document::new(vec![
            Page::new("BT /F 10 Tf (A) Tj ET").resources("/Font << /F 3 0 R >>"),
        ]);
        document.objects = vec![font.as_bytes().to_vec()];
        document.bytes()
    };
    // hayro-font は、`FirstChar` の数だけ表を伸ばし、どちらの値も環境の大きさの整数で読む
    for (first, last) in [
        (262_145, 262_145),
        (4_000_000_000, 4_000_000_000),
        (65, 262_145),
        (65, 4_294_967_296),
    ] {
        let (drawing, diagnostics) = lift(&file(&font(first, last)));
        assert!(drawing.sheets[0].elements.is_empty(), "{first} {last}");
        assert!(
            diagnostics
                .iter()
                .any(|d| d.code() == codes::PDF_PAGE_DROPPED),
            "{first} {last}"
        );
    }
    // 上限の中なら読む。ふつうの範囲を超える分の手間とメモリを数える
    let (usual, large) = (file(&font(65, 65)), file(&font(262_144, 262_144)));
    let (drawing, _) = lift(&large);
    assert!(!drawing.sheets[0].elements.is_empty());
    assert!(used(&large, Kind::Work) >= used(&usual, Kind::Work) + 262_144);
    assert!(used(&large, Kind::DecodedBytes) >= used(&usual, Kind::DecodedBytes) + (8 << 20));
}

#[test]
fn keeps_only_one_byte_of_page_label_styles() {
    // 数の書き方は 1 文字なので、長い名前を持たない。書き方を名前のエスケープで書いても読む
    let mut document = Document::new(vec![Page::new(""), Page::new("")]);
    document.catalog = "/PageLabels << /Nums [0 << /S /#44 >> 1 << /S /#44#44 /P (B-) >>] >>";
    let (drawing, _) = lift(&document.bytes());
    let names: Vec<_> = drawing
        .sheets
        .iter()
        .map(|sheet| sheet.name.clone())
        .collect();
    assert_eq!(names, [Some("1".to_owned()), Some("B-".to_owned())]);
    // 数の木の節は、直接書いたストリームの辞書でもよい
    let mut document = Document::new(vec![Page::new("")]);
    document.catalog = "/PageLabels << /Kids [<< /Nums [0 << /S /r >>] >>\nstream\n\nendstream] >>";
    let (drawing, _) = lift(&document.bytes());
    assert_eq!(drawing.sheets[0].name.as_deref(), Some("i"));
}

#[test]
fn reads_empty_flate_streams() {
    // 空の Flate は、hayro-syntax が空のバイト列にするので、何も失わない（ページの内容・フォーム・配列の項目）
    let empty = common::stream("/Filter /FlateDecode", "");
    let form = common::stream(
        "/Type /XObject /Subtype /Form /BBox [0 0 1 1] /Filter /FlateDecode",
        "",
    );
    let line = common::stream("", "0 0 m 10 10 l S");
    let mut document = Document::new(vec![
        Page::new("").contents("3 0 R"),
        Page::new("/F Do 0 0 m 10 10 l S").resources("/XObject << /F 4 0 R >>"),
        Page::new("").contents("[3 0 R 5 0 R]"),
    ]);
    document.objects = vec![empty, form, line];
    let (drawing, diag) = lift(&document.bytes());
    assert!(diag.is_empty(), "{diag:?}");
    let counts: Vec<usize> = drawing
        .sheets
        .iter()
        .map(|sheet| sheet.elements.len())
        .collect();
    assert_eq!(counts, [0, 1, 1]);
}
