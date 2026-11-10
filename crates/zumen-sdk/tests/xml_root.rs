//! XML の署名は文字コードの宣言の正否や本文を検証せず、原本の最初の開始タグで判定する。

use zumen_sdk::Signature;

const NAMESPACE: &str = "http://www.landxml.org/schema/LandXML-1.2";
const LANDXML: Signature = Signature::XmlRoot {
    name: b"LandXML",
    namespace: Some(NAMESPACE.as_bytes()),
};

fn utf16(text: &str, little: bool, bom: bool) -> Vec<u8> {
    let mut bytes = Vec::new();
    if bom {
        bytes.extend(if little { [0xff, 0xfe] } else { [0xfe, 0xff] });
    }
    for unit in text.encode_utf16() {
        bytes.extend(if little {
            unit.to_le_bytes()
        } else {
            unit.to_be_bytes()
        });
    }
    bytes
}

#[test]
fn utf16_signatures_skip_the_prolog_and_resolve_the_root_prefix() {
    for little in [false, true] {
        for bom in [false, true] {
            let label = if bom {
                "UTF-16"
            } else if little {
                "UTF-16LE"
            } else {
                "UTF-16BE"
            };
            // 非 ASCII の下位バイトが :・<・>・引用符でも、区切りとして扱わない。
            let text = format!(
                "<?xml version='1.0' encoding='{label}'?>\n<!-- 表示😀 -->\n\
                 <!DOCTYPE x [<!ENTITY e \"]>\">]>\n<?pi data?>\n\
                 <ĺ😀:LandXML a='ļľĢ' xmlns:ĺ😀='{NAMESPACE}' xmlns:x='other'>"
            );
            assert!(LANDXML.matches(&utf16(&text, little, bom)));
            assert!(LANDXML.matches(&utf16(
                &format!("<LandXML xmlns='{NAMESPACE}'/>"),
                little,
                true,
            )));
            // 宣言の不整合は L0 の検査に任せ、モジュールを選べるようにする。
            assert!(LANDXML.matches(&utf16(
                &format!("<?xml version='1.0' encoding='UTF-8'?><LandXML xmlns='{NAMESPACE}'>"),
                little,
                bom,
            )));
        }
    }
}

#[test]
fn the_complete_start_tag_is_required_but_the_following_body_is_not_read() {
    let text = format!("<LandXML xmlns='{NAMESPACE}'>");
    for little in [false, true] {
        let mut bytes = utf16(&text, little, true);
        for end in 0..bytes.len() {
            assert!(!LANDXML.matches(&bytes[..end]), "{little} {end}");
        }
        assert!(LANDXML.matches(&bytes));
        // 孤立した上位サロゲートと、切れたコード単位が後ろにあっても、開始タグの判定は変えない。
        bytes.extend(if little { [0, 0xd8] } else { [0xd8, 0] });
        bytes.push(0xff);
        assert!(LANDXML.matches(&bytes));
    }
}

#[test]
fn utf16_names_and_namespaces_are_compared_without_losing_unicode() {
    let unicode = Signature::XmlRoot {
        name: "図😀".as_bytes(),
        namespace: Some("urn:測😀".as_bytes()),
    };
    let any = Signature::XmlRoot {
        name: b"LandXML",
        namespace: None,
    };
    for little in [false, true] {
        assert!(unicode.matches(&utf16("<測😀:図😀 xmlns:測😀='urn:測😀'/>", little, true)));
        assert!(any.matches(&utf16("<LandXML/>", little, true)));
        for text in [
            format!("<LandXMLx xmlns='{NAMESPACE}'>"),
            String::from("<LandXML xmlns='urn:other'>"),
            String::from("<LandXML>"),
            format!("<p:LandXML xmlns='{NAMESPACE}'>"),
            String::from("<!-- <LandXML> "),
            String::from("text<LandXML>"),
        ] {
            assert!(!LANDXML.matches(&utf16(&text, little, true)));
        }
        let mut broken_name = utf16("<", little, true);
        broken_name.extend(if little { [0, 0xd8] } else { [0xd8, 0] });
        broken_name.extend(utf16("/>", little, false));
        assert!(!unicode.matches(&broken_name));
    }
}

#[test]
fn utf32_signatures_and_incomplete_units_do_not_match() {
    for prefix in [
        [0, 0, 0xfe, 0xff],
        [0xff, 0xfe, 0, 0],
        [0, 0, 0xff, 0xfe],
        [0xfe, 0xff, 0, 0],
        [0, 0, 0, 0x3c],
        [0x3c, 0, 0, 0],
        [0, 0, 0x3c, 0],
        [0, 0x3c, 0, 0],
    ] {
        let mut bytes = prefix.to_vec();
        bytes.extend(utf16(
            &format!("<LandXML xmlns='{NAMESPACE}'>"),
            true,
            false,
        ));
        assert!(!LANDXML.matches(&bytes));
    }
    for bytes in [&b"\xff"[..], b"\xff\xfe", b"\xff\xfe<", b"\xfe\xff\0"] {
        assert!(!LANDXML.matches(bytes));
    }
}

#[test]
fn signatures_keep_byte_encodings_and_do_not_limit_the_header_length() {
    let mut bytes = b"<?xml version='1.0' encoding='Shift_JIS'?><LandXML a='".to_vec();
    bytes.extend([0x95, 0x5c, 0x8e, 0xa6]);
    bytes.extend(format!("' xmlns='{NAMESPACE}'>").as_bytes());
    assert!(LANDXML.matches(&bytes));
    let text = format!(
        "<!-- {} --><LandXML a='{}' xmlns='{NAMESPACE}'>",
        "x".repeat(65_536),
        "y".repeat(65_536),
    );
    assert!(LANDXML.matches(text.as_bytes()));
    for little in [false, true] {
        assert!(LANDXML.matches(&utf16(&text, little, true)));
    }
}
