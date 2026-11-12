//! #520: 注釈の通常外観。固定の資源と、任意個の AP・注釈を組み合わせる。

#![allow(dead_code)]

use super::common::{Builder, Document, Page, stream};

/// 固定資源の後に置く、最初の外観のオブジェクト番号。
pub const FIRST_AP: usize = 14;
pub const LINE: &str = "0 0 m 10 10 l S";
pub const TEXT_FILL: &str = "BT /F 4 Tf 2 3 Td (A) Tj ET";
pub const TEXT_STROKE: &str = "BT /F 4 Tf 1 Tr 2 3 Td (A) Tj ET";
pub const TEXT_BOTH: &str = "BT /F 4 Tf 2 Tr 2 3 Td (A) Tj ET";
pub const DANGEROUS_TEXT: &str = "BT /Bad 4 Tf 2 3 Td <0001> Tj ET";

pub fn dangerous_font() -> &'static str {
    "<< /Type /Font /Subtype /Type0 /BaseFont /Bad /Encoding /Identity-H /DescendantFonts [<< /Subtype /CIDFontType2 /BaseFont /Bad /CIDSystemInfo << /Registry (Adobe) /Ordering (Japan1) /Supplement 6 >> /W [0 4294967295 500] >>] >>"
}

/// 通常外観。Subtype を省いた形も hayro が Form として解釈する。
pub struct Appearance {
    pub content: String,
    pub bbox: Option<String>,
    pub extra: String,
}

impl Appearance {
    pub fn new(content: &str) -> Self {
        Self {
            content: content.to_owned(),
            bbox: Some("[0 0 10 10]".to_owned()),
            extra: String::new(),
        }
    }

    pub fn bbox(mut self, bbox: &str) -> Self {
        self.bbox = Some(bbox.to_owned());
        self
    }

    pub fn without_bbox(mut self) -> Self {
        self.bbox = None;
        self
    }

    pub fn extra(mut self, extra: &str) -> Self {
        self.extra = extra.to_owned();
        self
    }
}

pub enum Normal {
    Stream(usize),
    States(Vec<(String, usize)>, Option<String>),
    Missing,
    Raw(String),
}

pub struct Annotation {
    pub normal: Normal,
    pub rect: Option<String>,
    pub extra: String,
}

impl Annotation {
    pub fn new(appearance: usize) -> Self {
        Self::normal(Normal::Stream(appearance))
    }

    pub fn states(states: &[(&str, usize)], selected: Option<&str>) -> Self {
        Self::normal(Normal::States(
            states
                .iter()
                .map(|&(name, index)| (name.to_owned(), index))
                .collect(),
            selected.map(str::to_owned),
        ))
    }

    pub fn missing() -> Self {
        Self::normal(Normal::Missing)
    }

    pub fn raw(normal: &str) -> Self {
        Self::normal(Normal::Raw(normal.to_owned()))
    }

    fn normal(normal: Normal) -> Self {
        Self {
            normal,
            rect: Some("[0 0 10 10]".to_owned()),
            extra: String::new(),
        }
    }

    pub fn rect(mut self, rect: &str) -> Self {
        self.rect = Some(rect.to_owned());
        self
    }

    pub fn without_rect(mut self) -> Self {
        self.rect = None;
        self
    }

    pub fn extra(mut self, extra: &str) -> Self {
        self.extra = extra.to_owned();
        self
    }
}

pub struct Fixture {
    pub bytes: Vec<u8>,
    /// Annots のキーだけを同じ長さの未知キーへ変え、全レコードの位置を保った比較用入力。
    pub without_annotations: Vec<u8>,
    pub content_object: u32,
    pub annotation_objects: Vec<u32>,
}

pub fn make(page: &str, appearances: &[Appearance], annotations: &[Annotation]) -> Fixture {
    build(page, "", false, appearances, annotations)
}

pub fn with_page_extra(
    page: &str,
    extra: &str,
    appearances: &[Appearance],
    annotations: &[Annotation],
) -> Fixture {
    build(page, extra, false, appearances, annotations)
}

pub fn with_off(page: &str, appearances: &[Appearance], annotations: &[Annotation]) -> Fixture {
    build(page, "", true, appearances, annotations)
}

/// AP 自身を含む Form の段数。50 段の境界と、その 1 段先を同じ構造で確かめる。
pub fn nested(depth: usize) -> Fixture {
    let appearances: Vec<_> = (0..depth)
        .map(|index| {
            if index + 1 == depth {
                Appearance::new(LINE).extra("/Subtype /Form")
            } else {
                Appearance::new("/Next Do").extra(&format!(
                    "/Subtype /Form /Resources << /XObject << /Next {} 0 R >> >>",
                    FIRST_AP + index + 1
                ))
            }
        })
        .collect();
    make("1 1 m 2 2 l S", &appearances, &[Annotation::new(0)])
}

/// 再帰せず、q/Q のグラフィックス状態だけを深く積む通常外観。
pub fn stacked(depth: usize) -> Fixture {
    let content = format!("{}{LINE}{}", "q ".repeat(depth), " Q".repeat(depth));
    make("", &[Appearance::new(&content)], &[Annotation::new(0)])
}

/// 全ページの既存様式を作る前に AP の様式を挿入しないための、2 ページの入力。
pub fn multiple_pages() -> Fixture {
    let first = "[2 1] 0 d 1 1 m 10 1 l S BT /F 4 Tf 2 20 Td <0001> Tj ET";
    let second = "[3 1] 0 d 1 1 m 10 1 l S BT /F 4 Tf 2 20 Td <0001> Tj ET";
    let mut document = Document::new(vec![
        Page::new(first)
            .resources("/Font << /F 3 0 R >>")
            .extra("/MediaBox [0 0 200 200] /Annots [8 0 R]"),
        Page::new(second)
            .resources("/Font << /F 5 0 R >>")
            .extra("/MediaBox [0 0 200 200]"),
    ]);
    document.objects = vec![
        named_font("PageOne"),
        stream(
            "",
            "begincmap /CMapName /Test def /CMapType 2 def 1 begincodespacerange <0000> <FFFF> endcodespacerange 1 beginbfchar <0001> <0041> endbfchar endcmap",
        ),
        named_font("PageTwo"),
        named_font("Appearance"),
        stream(
            "/BBox [0 0 100 100] /Resources << /Font << /F 6 0 R >> >>",
            "[7 2] 0 d 10 10 m 20 10 l S BT /F 4 Tf 2 20 Td <0001> Tj ET",
        ),
        b"<< /Type /Annot /Subtype /Stamp /Rect [0 0 100 100] /AP << /N 7 0 R >> >>".to_vec(),
    ];
    let bytes = document.bytes();
    let mut without_annotations = bytes.clone();
    if let Some(index) = without_annotations
        .windows(7)
        .position(|bytes| bytes == b"/Annots")
    {
        without_annotations[index..index + 7].copy_from_slice(b"/ZZZZZZ");
    }
    Fixture {
        bytes,
        without_annotations,
        content_object: 10,
        annotation_objects: vec![8],
    }
}

/// PAGE の既存の安全性 guard が働いたとき、注釈だけを描き始めないことを確かめる。
pub fn refused_page() -> Fixture {
    let mut document = Document::new(vec![
        Page::new(DANGEROUS_TEXT)
            .resources("/Font << /Bad 3 0 R >>")
            .extra("/Annots [5 0 R 6 0 R]"),
    ]);
    document.objects = vec![
        dangerous_font().as_bytes().to_vec(),
        stream("/BBox [0 0 10 10]", LINE),
        b"<< /Type /Annot /Subtype /Stamp /Rect [0 0 10 10] /AP << /N 4 0 R >> >>".to_vec(),
        b"<< /Type /Annot /Subtype /Stamp /Rect [20 20 30 30] /AP << /N 4 0 R >> >>".to_vec(),
    ];
    let bytes = document.bytes();
    let mut without_annotations = bytes.clone();
    if let Some(index) = without_annotations
        .windows(7)
        .position(|bytes| bytes == b"/Annots")
    {
        without_annotations[index..index + 7].copy_from_slice(b"/ZZZZZZ");
    }
    Fixture {
        bytes,
        without_annotations,
        content_object: 8,
        annotation_objects: vec![5, 6],
    }
}

fn named_font(name: &str) -> Vec<u8> {
    format!("<< /Type /Font /Subtype /Type0 /BaseFont /{name} /Encoding /Identity-H /DescendantFonts [<< /Type /Font /Subtype /CIDFontType2 /BaseFont /{name} /CIDSystemInfo << /Registry (Adobe) /Ordering (Japan1) /Supplement 6 >> /FontDescriptor << /Type /FontDescriptor /FontName /{name} /Flags 4 /FontBBox [0 -141 1000 859] /ItalicAngle 0 /Ascent 859 /Descent -141 /CapHeight 700 /StemV 80 >> /DW 1000 >>] /ToUnicode 4 0 R >>").into_bytes()
}

#[derive(Clone, Copy, Debug)]
pub enum ScalarCase {
    TransferDirect,
    TransferReference,
    DashReferences(usize),
    UnknownLiteral,
    UnknownHex,
}

/// scalar の値は最大 32 KiB 程度に固定して使い、実際の資源超過を試さない。
/// `length` は元の値のおよそのバイト数。hex の展開後は、その半分になる。
pub fn scalar(case: ScalarCase, length: usize) -> Fixture {
    let hex = format!("<{}>", "00".repeat(length / 2));
    let mut builder = Builder::default();
    builder.object("<< /Type /Catalog /Pages 2 0 R >>");
    builder.object("<< /Type /Pages /Kids [7 0 R] /Count 1 >>");
    builder.object(match case {
        ScalarCase::TransferReference | ScalarCase::DashReferences(_) => hex.as_str(),
        _ => "0",
    });
    let repeats = match case {
        ScalarCase::DashReferences(repeats) => repeats,
        _ => 0,
    };
    builder.object(format!("[{}]", "3 0 R ".repeat(repeats)));
    let (resources, extra, content) = match case {
        ScalarCase::TransferDirect => (
            format!("/Resources << /ExtGState << /T << /TR2 {hex} >> >> >>"),
            String::new(),
            "/T gs 0 0 m 10 10 l S",
        ),
        ScalarCase::TransferReference => (
            "/Resources << /ExtGState << /T << /TR2 3 0 R >> >> >>".to_owned(),
            String::new(),
            "/T gs 0 0 m 10 10 l S",
        ),
        ScalarCase::DashReferences(_) => (
            "/Resources << /ExtGState << /T << /D [4 0 R 0] >> >> >>".to_owned(),
            String::new(),
            "/T gs 0 0 m 10 10 l S",
        ),
        ScalarCase::UnknownLiteral => {
            let mut value = "padding 1 0 R ".repeat(length.div_ceil(14));
            value.truncate(length);
            (String::new(), format!("/Unrelated ({value})"), LINE)
        }
        ScalarCase::UnknownHex => (String::new(), format!("/Unrelated {hex}"), LINE),
    };
    builder.stream(&format!("/BBox [0 0 10 10] {resources} {extra}"), content);
    builder.object("<< /Type /Annot /Subtype /Stamp /Rect [0 0 10 10] /AP << /N 5 0 R >> >>");
    builder.object(
        "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 200 200] /Contents 8 0 R /Annots [6 0 R] >>",
    );
    builder.stream("", "20 20 m 30 30 l S");
    finish_fixture(builder.bytes(1), 8, vec![6])
}

/// 通常 scalar で Annots の型付き反復が終わった後の注釈は、hayro も読まない。
/// Subtype は直接の escaped Name と、同じ Name への参照の両方で試せる。
pub fn annotation_scalars(referenced_subtype: bool, tail: &str) -> Fixture {
    let mut builder = Builder::default();
    builder.object("<< /Type /Catalog /Pages 2 0 R >>");
    builder.object("<< /Type /Pages /Kids [7 0 R] /Count 1 >>");
    builder.object("/St#61mp");
    builder.stream("/BBox [0 0 10 10]", LINE);
    let subtype = if referenced_subtype {
        "3 0 R"
    } else {
        "/St#61mp"
    };
    builder.object(format!(
        "<< /Type /Annot /Subtype {subtype} /Rect [0 0 10 10] /AP << /N 4 0 R >> >>"
    ));
    builder.object("<< /Type /Annot /Subtype /Stamp /Rect [20 20 30 30] /AP << /N 4 0 R >> >>");
    builder.object(format!("<< /Type /Page /Parent 2 0 R /MediaBox [0 0 200 200] /Contents 8 0 R /Annots [5 0 R {tail} 6 0 R] >>"));
    builder.stream("", "20 20 m 30 30 l S");
    finish_fixture(builder.bytes(1), 8, vec![5, 6])
}

fn finish_fixture(bytes: Vec<u8>, content_object: u32, annotation_objects: Vec<u32>) -> Fixture {
    let mut without_annotations = bytes.clone();
    if let Some(index) = without_annotations
        .windows(7)
        .position(|bytes| bytes == b"/Annots")
    {
        without_annotations[index..index + 7].copy_from_slice(b"/ZZZZZZ");
    }
    Fixture {
        bytes,
        without_annotations,
        content_object,
        annotation_objects,
    }
}

fn image(optional: &str) -> Vec<u8> {
    stream(
        &format!(
            "/Type /XObject /Subtype /Image /Width 1 /Height 1 /ColorSpace /DeviceGray /BitsPerComponent 8 {optional}"
        ),
        [0],
    )
}

fn build(
    page: &str,
    page_extra: &str,
    off: bool,
    appearances: &[Appearance],
    annotations: &[Annotation],
) -> Fixture {
    let mut builder = Builder::default();
    let first_annotation = FIRST_AP + appearances.len();
    let page_object = first_annotation + annotations.len();
    let content_object = page_object + 1;
    let config = if off { "/OFF [6 0 R]" } else { "" };
    builder.object(format!("<< /Type /Catalog /Pages 2 0 R /OCProperties << /OCGs [5 0 R 6 0 R] /D << {config} >> >> >>"));
    builder.object(format!(
        "<< /Type /Pages /Kids [{page_object} 0 R] /Count 1 >>"
    ));
    builder.object("<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>");
    builder.object("<< /Type /Font /Subtype /Type1 /BaseFont /Courier >>");
    builder.object("<< /Type /OCG /Name (Shown) >>");
    builder.object("<< /Type /OCG /Name (Other) >>");
    builder.object(image("/OC 5 0 R"));
    builder.object(image("/OC 6 0 R"));
    builder.object(image(""));
    builder.stream("/Type /XObject /Subtype /Form /BBox [0 0 10 10]", "/Im Do");
    builder.stream(
        "/Type /XObject /Subtype /Form /BBox [0 0 10 10] /Resources << /XObject << /Im 8 0 R >> >>",
        "/Im Do",
    );
    builder.stream(
        "/Type /XObject /Subtype /Form /BBox [0 0 10 10]",
        "/OC /b BDC /Im Do",
    );
    builder.stream("/Type /XObject /Subtype /Form /BBox [0 0 10 10] /Resources << /XObject << /Self 13 0 R >> >>", "/Self Do");
    for appearance in appearances {
        let bbox = appearance
            .bbox
            .as_ref()
            .map_or_else(String::new, |bbox| format!("/BBox {bbox}"));
        builder.stream(&format!("{bbox} {}", appearance.extra), &appearance.content);
    }
    for annotation in annotations {
        let mut selected = String::new();
        let ap = match &annotation.normal {
            Normal::Stream(index) => format!("/AP << /N {} 0 R >>", FIRST_AP + index),
            Normal::States(states, state) => {
                let values: Vec<_> = states
                    .iter()
                    .map(|(name, index)| format!("/{name} {} 0 R", FIRST_AP + index))
                    .collect();
                if let Some(state) = state {
                    selected = format!("/AS /{state}");
                }
                format!("/AP << /N << {} >> >>", values.join(" "))
            }
            Normal::Missing => String::new(),
            Normal::Raw(raw) => format!("/AP << /N {raw} >>"),
        };
        let rect = annotation
            .rect
            .as_ref()
            .map_or_else(String::new, |rect| format!("/Rect {rect}"));
        builder.object(format!(
            "<< /Type /Annot /Subtype /Stamp {rect} {ap} {selected} {} >>",
            annotation.extra
        ));
    }
    let references: Vec<_> = (first_annotation..page_object)
        .map(|index| format!("{index} 0 R"))
        .collect();
    let page_dict = format!(
        "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 200 200] /Contents {content_object} 0 R /Resources << /Font << /F 3 0 R /G 4 0 R >> /XObject << /Im 7 0 R /Other 8 0 R /Plain 9 0 R /Fm 10 0 R /Rebound 11 0 R /Broken 12 0 R /Recursive 13 0 R >> /Properties << /a 5 0 R /b 6 0 R >> >> /Annots [{}] {page_extra} >>",
        references.join(" ")
    );
    builder.object(&page_dict);
    builder.stream("", page);
    let bytes = builder.bytes(1);
    // Builder の同じ入力から作り、Annots 以外のキー・値・レコードを動かさない。
    let mut without_annotations = bytes.clone();
    if let Some(index) = without_annotations
        .windows(7)
        .position(|bytes| bytes == b"/Annots")
    {
        without_annotations[index..index + 7].copy_from_slice(b"/ZZZZZZ");
    }
    Fixture {
        bytes,
        without_annotations,
        content_object: u32::try_from(content_object).unwrap_or(u32::MAX),
        annotation_objects: (first_annotation..page_object)
            .map(|index| u32::try_from(index).unwrap_or(u32::MAX))
            .collect(),
    }
}

pub struct Case {
    pub name: &'static str,
    pub fixture: Fixture,
}

/// native/wasm と外部照合でも、そのまま使える代表入力。
pub fn cases() -> Vec<Case> {
    vec![
        Case {
            name: "placed",
            fixture: make(
                "1 1 m 2 2 l S",
                &[Appearance::new(LINE)],
                &[Annotation::new(0).rect("[20 30 60 50]")],
            ),
        },
        Case {
            name: "matrix",
            fixture: make(
                "",
                &[Appearance::new(LINE).extra("/Matrix [0 1 -1 0 10 0]")],
                &[Annotation::new(0).rect("[20 30 60 50]")],
            ),
        },
        Case {
            name: "rotated-page",
            fixture: with_page_extra(
                "",
                "/Rotate 90",
                &[Appearance::new(LINE)],
                &[Annotation::new(0).rect("[20 30 60 50]")],
            ),
        },
        Case {
            name: "repeated",
            fixture: make(
                "",
                &[Appearance::new(LINE)],
                &[Annotation::new(0), Annotation::new(0).rect("[20 30 60 50]")],
            ),
        },
        Case {
            name: "resources",
            fixture: make(
                "/Im Do",
                &[Appearance::new("/Fm Do /Rebound Do")],
                &[Annotation::new(0)],
            ),
        },
        Case {
            name: "phase-image",
            fixture: make(
                "/Plain Do",
                &[Appearance::new("/Plain Do").extra("/OC 6 0 R")],
                &[Annotation::new(0)],
            ),
        },
        Case {
            name: "state-on",
            fixture: make(
                "",
                &[Appearance::new(LINE), Appearance::new("0 10 m 10 0 l S")],
                &[Annotation::states(&[("On", 0), ("Off", 1)], Some("On"))],
            ),
        },
        Case {
            name: "state-off",
            fixture: make(
                "",
                &[Appearance::new(LINE), Appearance::new("0 10 m 10 0 l S")],
                &[Annotation::states(
                    &[("On", 0), ("Off", 1)],
                    Some("Missing"),
                )],
            ),
        },
        Case {
            name: "page-ap-text",
            fixture: make(
                TEXT_FILL,
                &[Appearance::new(TEXT_STROKE)],
                &[Annotation::new(0)],
            ),
        },
        Case {
            name: "ap-ap-text",
            fixture: make(
                "",
                &[Appearance::new(TEXT_FILL), Appearance::new(TEXT_STROKE)],
                &[Annotation::new(0), Annotation::new(1)],
            ),
        },
        Case {
            name: "mode-two",
            fixture: make("", &[Appearance::new(TEXT_BOTH)], &[Annotation::new(0)]),
        },
        Case {
            name: "page-open-mark",
            fixture: make(
                "/OC /b BDC 1 1 m 2 2 l S",
                &[Appearance::new(LINE)],
                &[Annotation::new(0)],
            ),
        },
        Case {
            name: "page-excess-q",
            fixture: make(
                "Q 1 0 0 1 100 0 cm 1 1 m 2 2 l S",
                &[Appearance::new(LINE)],
                &[Annotation::new(0)],
            ),
        },
        Case {
            name: "hidden-annotation",
            fixture: make(
                "1 1 m 2 2 l S",
                &[Appearance::new(LINE)],
                &[Annotation::new(0).extra("/F 2")],
            ),
        },
        Case {
            name: "no-view-annotation",
            fixture: make(
                "1 1 m 2 2 l S",
                &[Appearance::new(LINE)],
                &[Annotation::new(0).extra("/F 32")],
            ),
        },
        Case {
            name: "annotation-oc",
            fixture: make(
                "1 1 m 2 2 l S",
                &[Appearance::new(LINE)],
                &[Annotation::new(0).extra("/OC 5 0 R")],
            ),
        },
        Case {
            name: "mixed-missing",
            fixture: make(
                "1 1 m 2 2 l S",
                &[Appearance::new(LINE)],
                &[Annotation::new(0), Annotation::missing()],
            ),
        },
        Case {
            name: "open-mark",
            fixture: make(
                "1 1 m 2 2 l S",
                &[Appearance::new("/OC /a BDC /Im Do")],
                &[Annotation::new(0)],
            ),
        },
        Case {
            name: "excess-q",
            fixture: make(
                "1 1 m 2 2 l S",
                &[Appearance::new("Q 0 0 m 10 10 l S")],
                &[Annotation::new(0)],
            ),
        },
        Case {
            name: "zero-rect",
            fixture: make(
                "1 1 m 2 2 l S",
                &[Appearance::new(LINE)],
                &[Annotation::new(0).rect("[0 0 0 10]")],
            ),
        },
        Case {
            name: "multiple-pages",
            fixture: multiple_pages(),
        },
        Case {
            name: "depth-50",
            fixture: nested(50),
        },
        Case {
            name: "depth-51",
            fixture: nested(51),
        },
        Case {
            name: "refused-page",
            fixture: refused_page(),
        },
        Case {
            name: "scalar-tr2-direct",
            fixture: scalar(ScalarCase::TransferDirect, 32 * 1024),
        },
        Case {
            name: "scalar-tr2-reference",
            fixture: scalar(ScalarCase::TransferReference, 32 * 1024),
        },
        Case {
            name: "scalar-dash-references",
            fixture: scalar(ScalarCase::DashReferences(8), 32 * 1024),
        },
        Case {
            name: "scalar-unknown-literal",
            fixture: scalar(ScalarCase::UnknownLiteral, 32 * 1024),
        },
        Case {
            name: "scalar-unknown-hex",
            fixture: scalar(ScalarCase::UnknownHex, 32 * 1024),
        },
        Case {
            name: "subtype-escaped-tail-integer",
            fixture: annotation_scalars(false, "17"),
        },
        Case {
            name: "subtype-reference-tail-hex",
            fixture: annotation_scalars(true, "<12ab>"),
        },
        Case {
            name: "stacked-4096",
            fixture: stacked(4096),
        },
    ]
}
