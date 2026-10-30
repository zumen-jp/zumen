//! 2007 より前の版の DXF に、図面モデルで変えた文字と名前を、見出しのコード・ページで書き戻す（決定 0192）。
#![cfg(feature = "drawing")]

use zumen_acad::{Dxf, DxfFile};
use zumen_base::{Budget, CancelToken, Code, Cx, Diagnostics, codes};
use zumen_drawing::{
    Applied, Changes, Drawing2D, Edit, Element, Hash, Layer, ModelPoint, Placed, Seen, Shape, id,
};
use zumen_sdk::{Apply, Error, Lift, testkit};

/// 外形（Shift_JIS）
const OUTLINE: &[u8] = b"\x8a\x4f\x8c\x60";
/// ア と ヂ。2 バイト目だけが ASCII の大文字と小文字の違い（0x41 と 0x61）
const A: &[u8] = b"\x83\x41";
const DI: &[u8] = b"\x83\x61";
/// 明朝
const MINCHO: &[u8] = b"\x96\xbe\x92\xa9";
/// 図面
const DRAWING: &[u8] = b"\x90\x7d\x96\xca";
/// 表示。2 バイト目が `\`
const SHOW: &[u8] = b"\x95\x5c\x8e\xa6";

/// コード・ページが `page` の R2000 の DXF。画層 0・外形・ア・ヂ、文字スタイル Standard・明朝を持ち、
/// TEXT（外形・明朝）・MTEXT（外形）と、ア・ヂの LINE を 1 つずつ持つ
#[cfg(test)]
fn src(page: &str) -> Vec<u8> {
    let layer = |handle: &'static str, name: &'static [u8]| -> Vec<(i16, &'static [u8])> {
        vec![
            (0, b"LAYER"),
            (5, handle.as_bytes()),
            (330, b"2"),
            (100, b"AcDbSymbolTableRecord"),
            (100, b"AcDbLayerTableRecord"),
            (2, name),
            (70, b"0"),
            (62, b"7"),
            (6, b"Continuous"),
        ]
    };
    let style = |handle: &'static str, name: &'static [u8]| -> Vec<(i16, &'static [u8])> {
        vec![
            (0, b"STYLE"),
            (5, handle.as_bytes()),
            (330, b"3"),
            (100, b"AcDbSymbolTableRecord"),
            (100, b"AcDbTextStyleTableRecord"),
            (2, name),
            (70, b"0"),
            (40, b"0.0"),
            (41, b"1.0"),
            (50, b"0.0"),
            (71, b"0"),
            (42, b"2.5"),
            (3, b"txt"),
            (4, b""),
        ]
    };
    let line = |handle: &'static str, layer: &'static [u8]| -> Vec<(i16, &'static [u8])> {
        vec![
            (0, b"LINE"),
            (5, handle.as_bytes()),
            (100, b"AcDbEntity"),
            (8, layer),
            (100, b"AcDbLine"),
            (10, b"0.0"),
            (20, b"0.0"),
            (30, b"0.0"),
            (11, b"1.0"),
            (21, b"1.0"),
            (31, b"0.0"),
        ]
    };
    let mut tags: Vec<(i16, &[u8])> = vec![
        (0, b"SECTION"),
        (2, b"HEADER"),
        (9, b"$ACADVER"),
        (1, b"AC1015"),
        (9, b"$DWGCODEPAGE"),
        (3, page.as_bytes()),
        (9, b"$HANDSEED"),
        (5, b"40"),
        (9, b"$CLAYER"),
        (8, OUTLINE),
        (0, b"ENDSEC"),
        (0, b"SECTION"),
        (2, b"TABLES"),
        (0, b"TABLE"),
        (2, b"LAYER"),
        (5, b"2"),
        (70, b"4"),
    ];
    tags.extend(layer("10", b"0"));
    tags.extend(layer("11", OUTLINE));
    tags.extend(layer("12", A));
    tags.extend(layer("13", DI));
    tags.extend([
        (0, &b"ENDTAB"[..]),
        (0, b"TABLE"),
        (2, b"STYLE"),
        (5, b"3"),
        (70, b"2"),
    ]);
    tags.extend(style("14", b"Standard"));
    tags.extend(style("15", MINCHO));
    tags.extend([
        (0, &b"ENDTAB"[..]),
        (0, b"ENDSEC"),
        (0, b"SECTION"),
        (2, b"ENTITIES"),
        (0, b"TEXT"),
        (5, b"1A"),
        (100, b"AcDbEntity"),
        (8, OUTLINE),
        (100, b"AcDbText"),
        (10, b"0.0"),
        (20, b"0.0"),
        (30, b"0.0"),
        (40, b"2.5"),
        (1, DRAWING),
        (7, MINCHO),
        (100, b"AcDbText"),
        (0, b"MTEXT"),
        (5, b"1B"),
        (100, b"AcDbEntity"),
        (8, OUTLINE),
        (100, b"AcDbMText"),
        (10, b"0.0"),
        (20, b"5.0"),
        (30, b"0.0"),
        (40, b"2.5"),
        (71, b"1"),
        (1, SHOW),
    ]);
    tags.extend(line("1C", A));
    tags.extend(line("1D", DI));
    tags.extend([(0, &b"ENDSEC"[..]), (0, b"EOF")]);
    let mut out = Vec::new();
    for (code, value) in tags {
        out.extend(format!("{code:>3}\n").into_bytes());
        out.extend_from_slice(value);
        out.push(b'\n');
    }
    out
}

#[cfg(test)]
fn lift(file: &DxfFile) -> Drawing2D {
    let mut diag = Diagnostics::new();
    file.lift(&mut Cx::new(
        &mut Budget::default(),
        &mut diag,
        &CancelToken::new(),
    ))
    .unwrap()
}

#[cfg(test)]
fn apply(file: &mut DxfFile, edits: Vec<Edit>) -> (Result<Vec<Applied>, Error>, Vec<Code>) {
    let mut diag = Diagnostics::new();
    let result = file.apply(
        &Changes {
            edits,
            document: None,
        },
        &mut Cx::new(&mut Budget::default(), &mut diag, &CancelToken::new()),
    );
    (result, diag.iter().map(|d| d.code()).collect())
}

/// 書いて読み直した図面モデルと、書いたバイト列
#[cfg(test)]
fn reread(file: &DxfFile) -> (Drawing2D, Vec<u8>) {
    let written = testkit::write::<Dxf>(file).unwrap();
    let again = testkit::read::<Dxf>(&written).0.unwrap();
    (lift(&again), written)
}

#[cfg(test)]
fn layer<'a>(drawing: &'a Drawing2D, name: &str) -> &'a Layer {
    drawing
        .layers
        .iter()
        .find(|layer| layer.name == name)
        .unwrap()
}

#[cfg(test)]
fn layer_seen(layer: &Layer) -> Seen {
    Seen {
        id: id::layer(layer),
        hash: Hash::layer(layer),
    }
}

/// 実寸の空間の `index` 番目の要素と、それを指す ID とハッシュ
#[cfg(test)]
fn element(drawing: &Drawing2D, index: usize) -> (Element<ModelPoint>, Seen) {
    let space = &drawing.spaces[0];
    let place = id::space(space.id);
    let mut namer = id::Namer::new(&place);
    let id = (0..=index)
        .map(|at| namer.next(at, &space.elements[at]))
        .last()
        .unwrap();
    let element = space.elements[index].clone();
    let hash = Hash::element(&element);
    (element, Seen { id, hash })
}

#[cfg(test)]
fn replace(target: Seen, element: Element<ModelPoint>) -> Edit {
    Edit::Replace {
        target,
        element: Box::new(Placed::Model(element)),
    }
}

/// `bytes` が `part` を含むか
#[cfg(test)]
fn contains(bytes: &[u8], part: &[u8]) -> bool {
    bytes.windows(part.len()).any(|window| window == part)
}

/// 内容 `content` を読んで得る区間。区間は内容から決まるので、内容を変えるときは区間も合わせる
#[cfg(test)]
fn spans(content: &str) -> Vec<zumen_drawing::Span> {
    let src = format!(
        "  0\nSECTION\n  2\nHEADER\n  9\n$ACADVER\n  1\nAC1021\n  0\nENDSEC\n  0\nSECTION\n  2\n\
         ENTITIES\n  0\nMTEXT\n  5\n1A\n  8\n0\n 10\n0.0\n 20\n0.0\n 40\n2.5\n  1\n{content}\n  0\n\
         ENDSEC\n  0\nEOF\n"
    );
    let file = testkit::read::<Dxf>(src.as_bytes()).0.unwrap();
    match &lift(&file).spaces[0].elements[0].shape {
        Shape::Formatted(text) => text.spans.clone(),
        shape => panic!("{shape:?}"),
    }
}

#[test]
fn texts_and_names_are_written_back_in_shift_jis() {
    let mut file = testkit::read::<Dxf>(&src("ANSI_932")).0.unwrap();
    let drawing = lift(&file);
    let (mut text, text_seen) = element(&drawing, 0);
    let (mut mtext, mtext_seen) = element(&drawing, 1);
    let Shape::Text(single) = &mut text.shape else {
        panic!("{:?}", text.shape);
    };
    assert_eq!(single.content, "図面");
    single.content = "通り芯 X1".to_owned();
    let Shape::Formatted(formatted) = &mut mtext.shape else {
        panic!("{:?}", mtext.shape);
    };
    assert_eq!(formatted.content, "表示");
    // 250 バイトを超える内容は、Shift_JIS の 2 バイトの文字の途中で切らずに分ける（1 文字目で奇数にずらす）
    formatted.content = format!("x{}", "表示".repeat(100));
    formatted.spans = spans(&formatted.content);
    let outline = layer(&drawing, "外形");
    let mut road = outline.clone();
    road.name = "道路".to_owned();
    let mincho = drawing
        .text_styles
        .iter()
        .enumerate()
        .find(|(_, style)| style.name == "明朝")
        .unwrap();
    let mut gothic = mincho.1.clone();
    gothic.name = "ゴシック".to_owned();
    let (result, found) = apply(
        &mut file,
        vec![
            replace(text_seen, text.clone()),
            replace(mtext_seen, mtext.clone()),
            Edit::ReplaceLayer {
                target: layer_seen(outline),
                layer: Box::new(road),
            },
            Edit::ReplaceTextStyle {
                target: Seen {
                    id: id::text_style(mincho.1, mincho.0),
                    hash: Hash::text_style(mincho.1),
                },
                style: Box::new(gothic),
            },
        ],
    );
    assert!(result.is_ok(), "{found:?}");
    assert_eq!(found, []);

    let (after, written) = reread(&file);
    let road = layer(&after, "道路");
    assert!(after.layers.iter().all(|layer| layer.name != "外形"));
    assert!(
        after
            .text_styles
            .iter()
            .any(|style| style.name == "ゴシック")
    );
    let elements = &after.spaces[0].elements;
    let Shape::Text(single) = &elements[0].shape else {
        panic!("{:?}", elements[0].shape);
    };
    assert_eq!(single.content, "通り芯 X1");
    let Shape::Formatted(formatted) = &elements[1].shape else {
        panic!("{:?}", elements[1].shape);
    };
    assert_eq!(formatted.content, format!("x{}", "表示".repeat(100)));
    assert!(elements[..2].iter().all(|element| element.layer == road.id));
    // 文字・画層の名前・今の画層・文字スタイルの名前は Shift_JIS で書く
    assert!(contains(&written, b"  1\n\x92\xca\x82\xe8\x90\x63 X1\n"));
    assert!(contains(&written, b"$CLAYER\n  8\n\x93\xb9\x98\x48\n"));
    assert!(contains(&written, b"  2\n\x93\xb9\x98\x48\n"));
    assert!(contains(
        &written,
        b"  7\n\x83\x53\x83\x56\x83\x62\x83\x4e\n"
    ));
    assert!(!contains(&written, OUTLINE) && !contains(&written, MINCHO));
    // 区切り（3）は 250 バイト以内で、偶数のバイトで終わる（x と 2 バイトの文字 124 個）
    assert!(contains(
        &written,
        &[b"\n  3\nx", &SHOW.repeat(62)[..], b"\n  1\n"].concat()
    ));
}

#[test]
fn names_that_differ_in_a_trail_byte_are_different_names() {
    let mut file = testkit::read::<Dxf>(&src("ANSI_932")).0.unwrap();
    let drawing = lift(&file);
    let a = layer(&drawing, "ア");
    let mut renamed = a.clone();
    renamed.name = "イ".to_owned();
    let (result, found) = apply(
        &mut file,
        vec![Edit::ReplaceLayer {
            target: layer_seen(a),
            layer: Box::new(renamed),
        }],
    );
    assert!(result.is_ok(), "{found:?}");
    let (after, written) = reread(&file);
    let elements = &after.spaces[0].elements;
    // ヂ の線は、名前を変えた画層に移らない
    assert_eq!(elements[2].layer, layer(&after, "イ").id);
    assert_eq!(elements[3].layer, layer(&after, "ヂ").id);
    assert!(contains(&written, b"  8\n\x83\x61\n"));

    // ヂ を、もうある イ の名前にはできない
    let drawing = after;
    let di = layer(&drawing, "ヂ");
    let mut same = di.clone();
    same.name = "イ".to_owned();
    let (result, found) = apply(
        &mut file,
        vec![Edit::ReplaceLayer {
            target: layer_seen(di),
            layer: Box::new(same),
        }],
    );
    assert_eq!(result.unwrap_err(), Error::Rejected);
    assert_eq!(found, [codes::CHANGE_EDIT_UNSUPPORTED]);
}

#[test]
fn characters_outside_the_code_page_change_nothing() {
    let src = src("ANSI_932");
    let mut file = testkit::read::<Dxf>(&src).0.unwrap();
    let drawing = lift(&file);
    let (text, seen) = element(&drawing, 0);
    // é は Shift_JIS にない。¥ と − は、符号化すると \ と － に戻るので書かない
    for content in ["café", "¥100", "1−2"] {
        let mut changed = text.clone();
        let Shape::Text(single) = &mut changed.shape else {
            panic!("{:?}", changed.shape);
        };
        single.content = content.to_owned();
        let (result, found) = apply(&mut file, vec![replace(seen.clone(), changed)]);
        assert_eq!(result.unwrap_err(), Error::Rejected, "{content}");
        assert_eq!(
            found,
            [
                codes::CHANGE_EDIT_UNSUPPORTED,
                codes::CHANGE_CHARACTER_UNSUPPORTED
            ],
            "{content}"
        );
    }
    let outline = layer(&drawing, "外形");
    let mut euro = outline.clone();
    euro.name = "€".to_owned();
    // 書ける変更と一緒でも、どれも当てない
    let mut changed = text.clone();
    if let Shape::Text(single) = &mut changed.shape {
        single.content = "通り芯".to_owned();
    }
    let (result, found) = apply(
        &mut file,
        vec![
            replace(seen, changed),
            Edit::ReplaceLayer {
                target: layer_seen(outline),
                layer: Box::new(euro),
            },
        ],
    );
    assert_eq!(result.unwrap_err(), Error::Rejected);
    assert_eq!(
        found,
        [
            codes::CHANGE_EDIT_UNSUPPORTED,
            codes::CHANGE_CHARACTER_UNSUPPORTED
        ]
    );
    assert_eq!(testkit::write::<Dxf>(&file).unwrap(), src);
}

#[test]
fn other_code_pages_take_only_ascii() {
    let src = src("ANSI_1252");
    let mut file = testkit::read::<Dxf>(&src).0.unwrap();
    let drawing = lift(&file);
    // 対応表のないコード・ページの ASCII でない名前の画層は読めないので、0 だけがある
    let zero = layer(&drawing, "0");
    let mut renamed = zero.clone();
    renamed.name = "図面".to_owned();
    let (result, found) = apply(
        &mut file,
        vec![Edit::AddLayer {
            layer: Box::new(renamed),
        }],
    );
    assert_eq!(result.unwrap_err(), Error::Rejected);
    assert_eq!(
        found,
        [
            codes::CHANGE_EDIT_UNSUPPORTED,
            codes::CHANGE_CHARACTER_UNSUPPORTED
        ]
    );
    assert_eq!(testkit::write::<Dxf>(&file).unwrap(), src);
}
