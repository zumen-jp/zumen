//! #519: pdf.js を別のプログラムで呼び、寸法・文字の内容と位置・経路と画像の数を照らす。
//! PDF 32000-2 の 7.7.3（ページ）・8.5（経路）・8.9（画像）・9.4（文字）。
//! 公開サンプルの出所とライセンスは samples/ledger.toml。受け取った図面は読まない。

// 図面モデルへの移しと照らすので、drawing の機能で作る（#613）
#![cfg(test)]
#![cfg(feature = "drawing")]

use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};

use zumen_base::Diagnostic;
use zumen_drawing::{Drawing2D, Placement, Shape};
use zumen_json::Value;

mod common;
use common::{Document, Page, lift, mm, stream};

const SAMPLES: [(&str, &[u8], usize, usize, usize); 4] = [
    (
        "type3",
        include_bytes!("../../../samples/pdf/pdfium-type3.pdf"),
        0,
        0,
        3,
    ),
    (
        "ocg-invisible",
        include_bytes!("../../../samples/pdf/pdfium-ocg-invisible.pdf"),
        2,
        0,
        0,
    ),
    (
        "smask-blend",
        include_bytes!("../../../samples/pdf/pdfium-smask-blend.pdf"),
        2,
        0,
        0,
    ),
    (
        "jpx-lzw",
        include_bytes!("../../../samples/pdf/pdfium-jpx-lzw.pdf"),
        0,
        1,
        0,
    ),
];

fn counts(drawing: &Drawing2D) -> (usize, usize, usize) {
    let (mut paths, mut images, mut texts) = (0, 0, 0);
    for element in drawing.sheets.iter().flat_map(|sheet| &sheet.elements) {
        match element.shape {
            Shape::Text(_) => texts += 1,
            Shape::Underlay(_) => images += 1,
            Shape::Line { .. } | Shape::Polyline(_) | Shape::Curve(_) | Shape::Hatch(_) => {
                paths += 1
            }
            _ => panic!("比較の対象にない図形"),
        }
    }
    (paths, images, texts)
}

fn has_code(diag: &[Diagnostic], code: &str) -> bool {
    diag.iter()
        .any(|d| d.code().name() == code && d.count() > 0)
}

#[test]
fn reads_public_pdfium_samples() {
    for (name, data, paths, images, texts) in SAMPLES {
        let (drawing, diag) = lift(data);
        assert_eq!(drawing.sheets.len(), 1, "{name}");
        assert_eq!(counts(&drawing), (paths, images, texts), "{name}");
        match name {
            "type3" => assert!(has_code(&diag, "pdf.type3.approximated")),
            "ocg-invisible" => assert!(has_code(&diag, "pdf.optional-content.dropped")),
            "smask-blend" => assert!(has_code(&diag, "pdf.transparency.approximated")),
            _ => {}
        }
    }
}

// コアではなくテストの起動条件を読み、飛ばした理由をテストの出力に書く。
#[allow(clippy::disallowed_methods, clippy::print_stderr)]
fn oracle() -> Option<PathBuf> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../xtask/verify/pdfjs/read.mjs");
    let result = Command::new("node").arg(&path).arg("--check").output();
    let missing = match &result {
        Ok(output) if output.status.success() => return Some(path),
        Ok(output) => output.status.code() == Some(77),
        Err(error) => error.kind() == std::io::ErrorKind::NotFound,
    };
    assert!(missing, "pdf.js の起動に失敗: {result:?}");
    assert!(
        std::env::var_os("ZUMEN_PDFJS_REQUIRED").is_none(),
        "pdf.js が必要だが、入っていない"
    );
    eprintln!("pdf.js がないため外部の照合を飛ばす（xtask/verify/pdfjs/README.md）");
    None
}

fn read_external(path: &PathBuf, data: &[u8]) -> Value {
    let mut child = Command::new("node")
        .arg(path)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(data).unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "pdf.js が読めなかった: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    zumen_json::parse(&output.stdout).expect("pdf.js の出力が JSON でない")
}

fn field<'a>(value: &'a Value, key: &str) -> &'a Value {
    value.pointer(&format!("/{key}")).unwrap()
}

fn array(value: &Value) -> &[Value] {
    let Value::Array(items) = value else {
        panic!("配列でない");
    };
    items
}

fn number(value: &Value) -> f64 {
    let Value::Number(value) = value else {
        panic!("数でない");
    };
    value.to_f64().unwrap()
}

fn close(found: f64, expected_pt: f64) {
    let expected = mm(expected_pt);
    assert!(
        (found - expected).abs() <= 1e-4,
        "{found} mm と {expected} mm が違う"
    );
}

fn compare(name: &str, drawing: &Drawing2D, diag: &[Diagnostic], external: &Value) {
    let pages = array(field(external, "pages"));
    assert_eq!(drawing.sheets.len(), pages.len(), "{name}: ページの数");
    for (sheet, page) in drawing.sheets.iter().zip(pages) {
        let [width, height] = sheet.size.unwrap();
        close(width.0, number(field(page, "width")));
        close(height.0, number(field(page, "height")));
        let texts: Vec<_> = sheet
            .elements
            .iter()
            .filter_map(|e| match &e.shape {
                Shape::Text(text) => Some(text),
                _ => None,
            })
            .collect();
        let expected_texts = array(field(page, "texts"));
        assert_eq!(texts.len(), expected_texts.len(), "{name}: 文字の数");
        for (text, expected) in texts.iter().zip(expected_texts) {
            if name == "type3" {
                // ToUnicode がなく、字形名 C1 から Unicode に戻せない（9.10.2）。
                // pdf.js は元のコード 65 を A とし、zumen は置換文字と診断を返す（#520）。
                assert_eq!(text.content, "\u{fffd}");
                assert_eq!(*field(expected, "text"), Value::String("A".into()));
                assert!(
                    diag.iter()
                        .any(|d| d.code().name() == "pdf.glyph.dropped" && d.count() == 3)
                );
            } else {
                assert_eq!(
                    Value::String(text.content.clone()),
                    *field(expected, "text"),
                    "{name}: 文字の内容"
                );
            }
            let Placement::Anchor { point, .. } = text.placement else {
                panic!("1 点に置いた文字でない");
            };
            close(point.x, number(field(expected, "x")));
            close(point.y, number(field(expected, "y")));
        }
        let paths = sheet
            .elements
            .iter()
            .filter(|e| {
                matches!(
                    e.shape,
                    Shape::Line { .. } | Shape::Polyline(_) | Shape::Curve(_) | Shape::Hatch(_)
                )
            })
            .count();
        let images = sheet
            .elements
            .iter()
            .filter(|e| matches!(e.shape, Shape::Underlay(_)))
            .count();
        let expected_paths = number(field(page, "paths")) as usize;
        if name == "smask-blend" {
            // pdf.js はソフトマスク用の 2 経路も演算の一覧に含む。zumen はマスクを移さず、
            // 診断する（#520、PDF 32000-2 の 11.6.5）。差の数が変わったら、既知として通さない。
            assert_eq!((paths, expected_paths), (2, 4));
            assert_eq!(
                diag.iter()
                    .filter(|d| d.code().name() == "pdf.transparency.approximated")
                    .map(Diagnostic::count)
                    .sum::<u64>(),
                2
            );
        } else {
            assert_eq!(paths, expected_paths, "{name}: 経路の数");
        }
        assert_eq!(
            images,
            number(field(page, "images")) as usize,
            "{name}: 画像の数"
        );
    }
}

fn synthetic() -> Vec<u8> {
    let content = "10 10 m 30 10 l 40 10 m 60 10 l S \
        10 20 30 20 re B \
        60 20 m 65 40 85 40 90 20 c S \
        100 20 30 30 re 110 30 10 10 re f* \
        BT /F1 12 Tf 1 0 0 1 24 70 Tm (Zumen) Tj ET \
        q 20 0 0 20 140 40 cm /I1 Do Q \
        q 1 0 0 1 100 70 cm /Fm Do Q";
    let resources = "/Font << /F1 << /Type /Font /Subtype /Type1 /BaseFont /Helvetica >> >> \
        /XObject << /I1 3 0 R /Fm 4 0 R >>";
    let mut doc = Document::new(vec![
        Page::new(content)
            .resources(resources)
            .extra("/MediaBox [0 0 200 150]"),
        Page::new(content)
            .resources(resources)
            .extra("/MediaBox [0 0 220 170] /CropBox [5 5 205 155] /Rotate 90 /UserUnit 2"),
    ]);
    doc.objects.push(stream("/Type /XObject /Subtype /Image /Width 1 /Height 1 /ColorSpace /DeviceRGB /BitsPerComponent 8", [255, 0, 0]));
    doc.objects.push(stream(
        "/Type /XObject /Subtype /Form /BBox [0 0 40 40]",
        "0 0 30 30 re S",
    ));
    doc.bytes()
}

#[test]
fn matches_pdfjs() {
    let Some(oracle) = oracle() else { return };
    let data = synthetic();
    let (drawing, diag) = lift(&data);
    assert_eq!(counts(&drawing), (14, 2, 2));
    compare("synthetic", &drawing, &diag, &read_external(&oracle, &data));
    for (name, data, _, _, _) in SAMPLES {
        let (drawing, diag) = lift(data);
        compare(name, &drawing, &diag, &read_external(&oracle, data));
    }
}
