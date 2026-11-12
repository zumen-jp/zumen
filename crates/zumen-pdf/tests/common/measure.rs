//! #520: 同じ紙の要素を保った VP/Measure の合成入力。

#![allow(dead_code)]

use super::common::Builder;

pub const PAGE: &str =
    "0.5 w 12 14 m 40 30 l S BT /F 8 Tf 20 60 Td (Plan) Tj ET q 10 0 0 10 70 80 cm /Im Do Q";

pub struct Viewport {
    pub bbox: Option<String>,
    pub name: Option<String>,
    pub measure: Option<String>,
    pub extra: String,
}

impl Viewport {
    pub fn new(measure: &str) -> Self {
        Self {
            bbox: Some("[0 0 144 144]".to_owned()),
            name: Some("(Plan)".to_owned()),
            measure: Some(measure.to_owned()),
            extra: String::new(),
        }
    }
    pub fn bbox(mut self, bbox: &str) -> Self {
        self.bbox = Some(bbox.to_owned());
        self
    }
    pub fn name(mut self, name: &str) -> Self {
        self.name = Some(name.to_owned());
        self
    }
    pub fn without_bbox(mut self) -> Self {
        self.bbox = None;
        self
    }
    pub fn without_measure(mut self) -> Self {
        self.measure = None;
        self
    }
    pub fn extra(mut self, extra: &str) -> Self {
        self.extra = extra.to_owned();
        self
    }
}

pub struct Fixture {
    pub bytes: Vec<u8>,
    /// VP のキーだけを同じ長さの未知キーへ変える。全レコードとバイト位置は変えない。
    pub without_viewports: Vec<u8>,
    pub content_object: u32,
    pub viewport_objects: Vec<u32>,
}

pub fn rl(x: &str, y: Option<&str>, extra: &str) -> String {
    rl_units("(m)", x, y.map(|factor| ("(m)", factor)), extra)
}

/// U は PDF の文字列の字句で渡す。未知のラベルや UTF-16 も、そのまま組み込める。
pub fn rl_units(x_label: &str, x: &str, y: Option<(&str, &str)>, extra: &str) -> String {
    let y = y.map_or_else(String::new, |(label, factor)| {
        format!("/Y [<< /Type /NumberFormat /U {label} /C {factor} >>]")
    });
    format!(
        "<< /Type /Measure /Subtype /RL /R (source scale text) /X [<< /Type /NumberFormat /U {x_label} /C {x} >>] {y} /D [<< /U (m) /C 1 >>] /A [<< /U (m2) /C 1 >>] {extra} >>"
    )
}

pub fn full_formats() -> String {
    r"<< /Type /Measure /Subtype /RL /R <FEFF7E2E5C3A>
      /X [<< /Type /NumberFormat /U <FEFF5C3A> /C 2 /F /F /D 16 /FD true /RT (\054) /RD (.) /PS (<) /SS (>) /O /P /Private << /Kept true >> >> << /U (in) /C 12 >>]
      /Y [<< /U (palm) /C 3 >>] /CYX 0.5
      /D [<< /U (ft) /C 1 >> << /U (in) /C 12 >>]
      /A [<< /U (square ft) /C 0.25 >>] /T [<< /U (degree) /C 1 >>] /S [<< /U (rise) /C 1 >>]
      /O [12.5 -3.25] >>".to_owned()
}

pub fn overflow() -> Fixture {
    let large = format!("1{}", "0".repeat(307));
    let small = format!("0.{}1", "0".repeat(306));
    make(&[Viewport::new(&rl(&large, Some(&small), ""))])
}

pub const TWENTY_DIGITS: &str = "0.12345678901234567890";
pub const PRECISE_SMALL: &str = "0.001390000000000000000000000000";

pub fn tiny_decimal() -> String {
    format!("0.{}1", "0".repeat(306))
}

pub fn direct_tiny_factor() -> Fixture {
    let tiny = tiny_decimal();
    make(&[Viewport::new(&rl(
        "1",
        Some(&tiny),
        &format!("/O [{tiny} {TWENTY_DIGITS}]"),
    ))])
}

pub fn referenced_tiny_factor() -> Fixture {
    with_objects(
        PAGE,
        "",
        &[
            tiny_decimal().into_bytes(),
            b"<< /Type /NumberFormat /U (m) /C 5 0 R >>".to_vec(),
            format!("<< /Type /NumberFormat /U (m) /C {PRECISE_SMALL} >>").into_bytes(),
            format!("[5 0 R {PRECISE_SMALL}]").into_bytes(),
        ],
        &[Viewport::new(&format!(
            "<< /Type /Measure /Subtype /RL /X [6 0 R] /Y [7 0 R] /CYX {TWENTY_DIGITS} /O 8 0 R /D [<< /U (m) /C 1 >>] /A [<< /U (m2) /C 1 >>] >>"
        ))],
    )
}

pub fn decimal_twenty_digits() -> Fixture {
    make(&[Viewport::new(&rl(
        PRECISE_SMALL,
        Some(TWENTY_DIGITS),
        &format!("/CYX {PRECISE_SMALL} /O [{TWENTY_DIGITS} -{PRECISE_SMALL}]"),
    ))])
}

pub fn make(viewports: &[Viewport]) -> Fixture {
    with_page(PAGE, "", viewports)
}

pub fn with_page(page: &str, extra: &str, viewports: &[Viewport]) -> Fixture {
    with_objects(page, extra, &[], viewports)
}

/// 追加オブジェクトは 5 番から。Viewport はその後に置く。
pub fn with_objects(
    page: &str,
    extra: &str,
    objects: &[Vec<u8>],
    viewports: &[Viewport],
) -> Fixture {
    build(page, extra, objects, viewports, None)
}

pub fn with_vp_value(value: &str) -> Fixture {
    build(PAGE, "", &[], &[], Some(value))
}

fn build(
    page: &str,
    extra: &str,
    objects: &[Vec<u8>],
    viewports: &[Viewport],
    raw_vp: Option<&str>,
) -> Fixture {
    let first_viewport = 5 + objects.len();
    let page_object = first_viewport + viewports.len();
    let content_object = page_object + 1;
    let mut builder = Builder::default();
    builder.object("<< /Type /Catalog /Pages 2 0 R >>");
    builder.object(format!(
        "<< /Type /Pages /Kids [{page_object} 0 R] /Count 1 >>"
    ));
    builder.object("<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>");
    builder.stream("/Type /XObject /Subtype /Image /Width 1 /Height 1 /ColorSpace /DeviceGray /BitsPerComponent 8", [0]);
    for object in objects {
        builder.object(object);
    }
    for viewport in viewports {
        let bbox = viewport
            .bbox
            .as_ref()
            .map_or_else(String::new, |value| format!("/BBox {value}"));
        let name = viewport
            .name
            .as_ref()
            .map_or_else(String::new, |value| format!("/Name {value}"));
        let measure = viewport
            .measure
            .as_ref()
            .map_or_else(String::new, |value| format!("/Measure {value}"));
        builder.object(format!(
            "<< /Type /Viewport {bbox} {name} {measure} {} >>",
            viewport.extra
        ));
    }
    let references: Vec<_> = (first_viewport..page_object)
        .map(|id| format!("{id} 0 R"))
        .collect();
    let list = format!("[{}]", references.join(" "));
    let vp = raw_vp.unwrap_or(&list);
    builder.object(format!("<< /Type /Page /Parent 2 0 R /MediaBox [0 0 200 200] /Contents {content_object} 0 R /Resources << /Font << /F 3 0 R >> /XObject << /Im 4 0 R >> >> /VP {vp} {extra} >>"));
    builder.stream("", page);
    let bytes = builder.bytes(1);
    let mut without_viewports = bytes.clone();
    if let Some(index) = without_viewports
        .windows(4)
        .position(|part| part == b"/VP ")
    {
        without_viewports[index..index + 3].copy_from_slice(b"/ZZ");
    }
    Fixture {
        bytes,
        without_viewports,
        content_object: u32::try_from(content_object).unwrap_or(u32::MAX),
        viewport_objects: (first_viewport..page_object)
            .map(|id| u32::try_from(id).unwrap_or(u32::MAX))
            .collect(),
    }
}

pub struct Case {
    pub name: &'static str,
    pub fixture: Fixture,
}

pub fn cases() -> Vec<Case> {
    vec![
        Case {
            name: "isotropic",
            fixture: make(&[Viewport::new(&rl("1", None, ""))]),
        },
        Case {
            name: "anisotropic",
            fixture: make(&[Viewport::new(&rl("2", Some("1"), ""))]),
        },
        Case {
            name: "negative",
            fixture: make(&[Viewport::new(&rl("-2", Some("-1"), ""))]),
        },
        Case {
            name: "cyx-explicit",
            fixture: make(&[Viewport::new(&rl("2", Some("0.5"), "/CYX 2"))]),
        },
        Case {
            name: "y-absent-stray-cyx",
            fixture: make(&[Viewport::new(&rl("1", None, "/CYX 9"))]),
        },
        Case {
            name: "unknown-unit",
            fixture: make(&[Viewport::new(&rl_units("(cubit)", "1", None, ""))]),
        },
        Case {
            name: "unknown-units-cyx",
            fixture: make(&[Viewport::new(&rl_units(
                "(span)",
                "2",
                Some(("(palm)", "0.5")),
                "/CYX 2",
            ))]),
        },
        Case {
            name: "origin-kept",
            fixture: make(&[Viewport::new(&rl("1", None, "/O [300 400]"))]),
        },
        Case {
            name: "rotated-crop-user-unit",
            fixture: with_page(
                PAGE,
                "/CropBox [10 20 110 120] /Rotate 90 /UserUnit 2",
                &[Viewport::new(&rl("2", Some("1"), "")).bbox("[10 20 110 120]")],
            ),
        },
        Case {
            name: "all-number-formats",
            fixture: make(&[Viewport::new(&full_formats()).name("<FEFF5E73976256F3>")]),
        },
        Case {
            name: "overlapping-viewports",
            fixture: make(&[
                Viewport::new(&rl("1", None, "")),
                Viewport::new(&rl("2", None, "")),
            ]),
        },
        Case {
            name: "referenced-formats",
            fixture: with_objects(
                PAGE,
                "",
                &[
                    b"<< /Type /NumberFormat /U (m) /C 1 >>".to_vec(),
                    b"<< /Type /Measure /Subtype /RL /R (source) /X [5 0 R] /D [] /A [] >>"
                        .to_vec(),
                ],
                &[Viewport::new("6 0 R")],
            ),
        },
        Case {
            name: "zero-c",
            fixture: make(&[Viewport::new(&rl("0", None, ""))]),
        },
        Case {
            name: "zero-cyx",
            fixture: make(&[Viewport::new(&rl("1", Some("1"), "/CYX 0"))]),
        },
        Case {
            name: "null-measure",
            fixture: make(&[Viewport::new("null")]),
        },
        Case {
            name: "unresolved-measure",
            fixture: make(&[Viewport::new("99 0 R")]),
        },
        Case {
            name: "wrong-measure-type",
            fixture: make(&[Viewport::new("17")]),
        },
        Case {
            name: "geo",
            fixture: make(&[Viewport::new(
                "<< /Type /Measure /Subtype /GEO /GPTS [0 0 1 1] /LPTS [0 0 1 1] /GCS << /Type /GEOGCS >> >>",
            )]),
        },
        Case {
            name: "wrong-bbox-count",
            fixture: make(&[Viewport::new(&rl("1", None, "")).bbox("[0 0 144]")]),
        },
        Case {
            name: "vp-value-type",
            fixture: with_vp_value("17"),
        },
        Case {
            name: "vp-null-before-valid",
            fixture: with_vp_value(&format!(
                "[null << /Type /Viewport /BBox [0 0 144 144] /Measure {} >>]",
                rl("1", None, "")
            )),
        },
        Case {
            name: "no-measure",
            fixture: make(&[Viewport::new("null").without_measure()]),
        },
        Case {
            name: "known-unit-spelling",
            fixture: make(&[Viewport::new(&rl_units("( Feet )", "1", None, ""))]),
        },
        Case {
            name: "measure-after-unmeasured-viewport",
            fixture: make(&[
                Viewport::new("null").without_measure(),
                Viewport::new(&rl("1", None, "")),
            ]),
        },
        Case {
            name: "reversed-bbox",
            fixture: make(&[Viewport::new(&rl("1", None, "")).bbox("[144 144 0 0]")]),
        },
        Case {
            name: "measurement-overflow",
            fixture: overflow(),
        },
        Case {
            name: "direct-tiny-factor",
            fixture: direct_tiny_factor(),
        },
        Case {
            name: "ref-tiny-factor",
            fixture: referenced_tiny_factor(),
        },
        Case {
            name: "decimal-twenty-digits",
            fixture: decimal_twenty_digits(),
        },
    ]
}
