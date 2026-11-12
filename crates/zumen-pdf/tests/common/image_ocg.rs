//! #520: 画像の OC とフォームの文脈を照合する最小の合成 PDF。
//!
//! OCG は Shown=3、Other=4、Hidden=5。OCMD=6、未登録 OCG=7。
//! Im/Same は画像8、Other は画像9、Plain は OC のない画像13、Mask はステンシル16。
//! Fm=10 は画像8、Grouped=11 は自身に OCG3、Rebound=12 は同じ資源名から画像9を引く。
//! PlainA=14/PlainB=15 は OC のない画像13を別の OCG の中で呼ぶ。
//! Broken=17 は OCG4 の BDC を閉じないフォーム。

#![allow(dead_code)]

use super::common::{Document, Page, stream};

pub struct Fixture {
    pub bytes: Vec<u8>,
    pub content_object: u32,
}

const GROUPS: &str = "/OCProperties << /OCGs [3 0 R 4 0 R 5 0 R] /D << >> >>";
const OFF: &str = "/OCProperties << /OCGs [3 0 R 4 0 R 5 0 R] /D << /OFF [5 0 R] >> >>";
const PROPERTIES: &str =
    "/Properties << /a 3 0 R /b 4 0 R /off 5 0 R /md 6 0 R /unregistered 7 0 R /missing 99 0 R >>";

pub fn make(content: &str, image_optional: &str) -> Fixture {
    build(content, image_optional, false, false, None)
}

pub fn with_off(content: &str, image_optional: &str) -> Fixture {
    build(content, image_optional, true, false, None)
}

pub fn anonymous(content: &str) -> Fixture {
    build(content, "/OC 3 0 R", false, true, None)
}

pub fn with_form(content: &str, image_optional: &str, form_content: &str) -> Fixture {
    build(content, image_optional, false, false, Some(form_content))
}

fn image(optional: &str) -> Vec<u8> {
    stream(
        &format!(
            "/Type /XObject /Subtype /Image /Width 1 /Height 1 /ColorSpace /DeviceGray /BitsPerComponent 8 {optional}"
        ),
        [0],
    )
}

fn form(optional: &str, image: u32, content: &str) -> Vec<u8> {
    stream(
        &format!(
            "/Type /XObject /Subtype /Form /BBox [0 0 100 100] /Resources << /XObject << /I {image} 0 R >> {PROPERTIES} >> {optional}"
        ),
        content,
    )
}

fn build(
    content: &str,
    image_optional: &str,
    off: bool,
    anonymous: bool,
    form_content: Option<&str>,
) -> Fixture {
    // 直接記述の stream は、資源の辞書を走査できる文字だけで組む。
    let inline = format!(
        "<< /Type /XObject /Subtype /Image /Width 1 /Height 1 /ColorSpace /DeviceGray /BitsPerComponent 8 /Filter /ASCIIHexDecode /Length 4 {image_optional} >>\nstream\n0 0 \nendstream"
    );
    let image_entry = if anonymous { inline.as_str() } else { "8 0 R" };
    let resources = format!(
        "/XObject << /Im {image_entry} /Same 8 0 R /Other 9 0 R /Fm 10 0 R /Grouped 11 0 R /Rebound 12 0 R /Plain 13 0 R /PlainA 14 0 R /PlainB 15 0 R /Mask 16 0 R /Broken 17 0 R >> {PROPERTIES}"
    );
    let mut document = Document::new(vec![Page::new(content).resources(&resources)]);
    document.objects = vec![
        b"<< /Type /OCG /Name (Shown) >>".to_vec(),
        b"<< /Type /OCG /Name (Other) >>".to_vec(),
        b"<< /Type /OCG /Name (Hidden) >>".to_vec(),
        b"<< /Type /OCMD /OCGs [3 0 R 4 0 R] /P /AnyOn >>".to_vec(),
        b"<< /Type /OCG /Name (Unregistered) >>".to_vec(),
        image(image_optional),
        image("/OC 4 0 R"),
        form("", 8, form_content.unwrap_or("/I Do")),
        form("/OC 3 0 R", 8, "/I Do"),
        form("", 9, "/I Do"),
        image(""),
        form("/OC 3 0 R", 13, "/I Do"),
        form("/OC 4 0 R", 13, "/I Do"),
        stream(
            "/Type /XObject /Subtype /Image /Width 1 /Height 1 /ImageMask true /BitsPerComponent 1 /OC 3 0 R",
            [0],
        ),
        form("", 8, "/OC /b BDC /I Do"),
    ];
    document.catalog = if off { OFF } else { GROUPS };
    Fixture {
        // 3〜17 が共通の資源、18 がページ、19 が内容。
        content_object: 19,
        bytes: document.bytes(),
    }
}
