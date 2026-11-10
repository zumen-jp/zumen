//! UTF-16 の符号化と原本範囲。UTF-8 に直した位置を公開しない（#468）。

#![cfg(test)]

use zumen_base::{Budget, CancelToken, Diagnostics, Kind, Limit, Limits, Stop, codes};
use zumen_sdk::{Cx, Error};
use zumen_xml::{Document, Encoding, Node};

fn encode(value: &str, little: bool, bom: bool) -> Vec<u8> {
    let mut bytes = Vec::new();
    if bom {
        bytes.extend_from_slice(if little { &[0xff, 0xfe] } else { &[0xfe, 0xff] });
    }
    for value in value.encode_utf16() {
        bytes.extend_from_slice(&if little {
            value.to_le_bytes()
        } else {
            value.to_be_bytes()
        });
    }
    bytes
}

fn read(src: &[u8], limits: Limits) -> (Result<Document, Error>, Diagnostics, Budget) {
    let mut diag = Diagnostics::new();
    let mut budget = Budget::new(limits);
    let cancel = CancelToken::new();
    let result = zumen_xml::read(src, &mut Cx::new(&mut budget, &mut diag, &cancel));
    (result, diag, budget)
}

fn offset(text: &str, token: &str) -> usize {
    let at = text.find(token).unwrap();
    2 + text.get(..at).unwrap().encode_utf16().count() * 2
}

#[test]
fn rejects_a_second_bom_but_keeps_feff_in_content() {
    for little in [true, false] {
        let src = encode("\u{feff}<r/>", little, true);
        let (result, diag, _) = read(&src, Limits::default());
        assert!(matches!(result, Err(Error::Invalid)), "{diag:?}");
        let diagnostic = diag.iter().next().unwrap();
        assert_eq!(diagnostic.code(), codes::XML_DOCUMENT_INVALID);
        assert_eq!(
            diagnostic.places(),
            &[zumen_base::Ref::at_bytes(2, src.len() as u64)]
        );

        let src = encode("<r>\u{feff}本文\u{feff}</r>", little, true);
        let (result, diag, _) = read(&src, Limits::default());
        let doc = result.unwrap();
        assert!(diag.is_empty(), "{diag:?}");
        let root = doc.element(doc.root()).unwrap();
        let Node::Text(range) = &root.children()[0] else {
            panic!()
        };
        assert_eq!(doc.text(range.clone()), "\u{feff}本文\u{feff}");
    }
    let (result, _, _) = read("\u{feff}\u{feff}<r/>".as_bytes(), Limits::default());
    assert!(matches!(result, Err(Error::Invalid)));
}

#[test]
fn preserves_original_bytes_names_namespaces_text_and_diagnostics() {
    let text = "<?xml version='1.0' encoding='UTF-16'?><p:r xmlns:p='urn:test' a='日😀\r\n&amp;'><p:c/>A<![CDATA[本😀\r\n]]>&external;</p:r>";
    for little in [true, false] {
        let src = encode(text, little, true);
        let (result, diag, _) = read(&src, Limits::default());
        let doc = result.unwrap();
        assert_eq!(doc.src(), src);
        assert_eq!(
            doc.encoding(),
            if little {
                Encoding::Utf16Le
            } else {
                Encoding::Utf16Be
            }
        );
        let root = doc.element(doc.root()).unwrap();
        assert_eq!(root.span(), offset(text, "<p:r")..src.len());
        assert_eq!(doc.local_name_text(root.name()), "r");
        assert_eq!(doc.local_name(root.name()), encode("r", little, false));
        assert_eq!(doc.text(root.name().raw()), "p:r");
        assert_eq!(doc.text(root.name().prefix().unwrap()), "p");
        assert!(doc.is(root.name(), Some("urn:test"), b"r"));
        let attribute = doc.attribute(root, None, b"a").unwrap();
        assert_eq!(doc.value(attribute), "日😀 &");
        assert_eq!(doc.text(attribute.name().raw()), "a");
        let child = doc.element(1).unwrap();
        assert_eq!(child.parent(), Some(0));
        assert!(doc.is(child.name(), Some("urn:test"), b"c"));
        let cdata = root
            .children()
            .iter()
            .find_map(|node| match node {
                Node::CData(range) => Some(range.clone()),
                _ => None,
            })
            .unwrap();
        assert_eq!(doc.cdata(cdata), "本😀\n");
        assert_eq!(diag.iter().count(), 1);
        let diagnostic = diag.iter().next().unwrap();
        assert_eq!(diagnostic.code(), codes::XML_ENTITY_UNSUPPORTED);
        let bytes = diagnostic.places()[0].bytes().unwrap();
        assert_eq!(
            (bytes.start, bytes.end),
            (
                offset(text, "&external;") as u64,
                (offset(text, "&external;") + "&external;".len() * 2) as u64
            )
        );
    }
}

#[test]
fn accepts_bom_or_explicit_matching_endian_declarations() {
    for little in [true, false] {
        for text in [
            "<r/>",
            "<?xml version='1.0'?><r/>",
            "<?xml version='1.0' encoding='UTF-16'?><r/>",
        ] {
            let (result, diag, _) = read(&encode(text, little, true), Limits::default());
            assert!(result.is_ok(), "{text}: {diag:?}");
            assert!(diag.is_empty());
        }
        let label = if little { "UTF-16LE" } else { "UTF-16BE" };
        let text = format!("<?xml version='1.0' encoding='{label}'?><r/>");
        let (result, diag, _) = read(&encode(&text, little, false), Limits::default());
        assert!(result.is_ok(), "{diag:?}");
        assert!(diag.is_empty());
    }
}

#[test]
fn mismatching_or_missing_declarations_and_broken_units_are_invalid() {
    for little in [true, false] {
        for (text, bom) in [
            ("<?xml version='1.0' encoding='UTF-16'?><r/>", false),
            ("<?xml version='1.0'?><r/>", false),
            ("<?xml version='1.0' encoding='UTF-8'?><r/>", true),
            ("<?xml version='1.0' encoding='UTF-16LE'?><r/>", true),
            ("<?xml version='1.0' encoding='UTF-16BE'?><r/>", true),
        ] {
            let (result, diag, _) = read(&encode(text, little, bom), Limits::default());
            assert_eq!(result, Err(Error::Invalid), "{text}: {diag:?}");
            assert!(diag.iter().any(|d| d.code() == codes::XML_DOCUMENT_INVALID));
        }
        let label = if little { "UTF-16BE" } else { "UTF-16LE" };
        let text = format!("<?xml version='1.0' encoding='{label}'?><r/>");
        assert_eq!(
            read(&encode(&text, little, false), Limits::default()).0,
            Err(Error::Invalid)
        );
        for units in [[0xd800, 0x0041], [0xdc00, 0xd800], [0, 0x41]] {
            let mut src = encode("<r>", little, true);
            let at = src.len();
            for unit in units {
                src.extend_from_slice(&if little {
                    u16::to_le_bytes(unit)
                } else {
                    u16::to_be_bytes(unit)
                });
            }
            let (result, diag, _) = read(&src, Limits::default());
            assert_eq!(result, Err(Error::Invalid));
            assert_eq!(
                diag.iter().next().unwrap().places()[0]
                    .bytes()
                    .unwrap()
                    .start,
                at as u64
            );
        }
        let mut src = encode("<r/>", little, true);
        src.pop();
        assert_eq!(read(&src, Limits::default()).0, Err(Error::Invalid));
        for text in ["<·x/>", "<\u{f0000}/>", "<a:b:c/>", "<r a:='x'/>"] {
            assert_eq!(
                read(&encode(text, little, true), Limits::default()).0,
                Err(Error::Invalid)
            );
        }
    }
}

#[test]
fn conversion_and_mapping_stop_at_exact_budget_boundaries() {
    let src = encode(
        "<p:r xmlns:p='urn:test' a='日本😀'><p:c/>文字</p:r>",
        true,
        true,
    );
    let (result, diag, baseline) = read(&src, Limits::default());
    let document = result.unwrap();
    assert!(diag.is_empty());
    for kind in Kind::ALL {
        if kind == Kind::Depth {
            continue;
        }
        let used = baseline.used(kind);
        let (result, diag, exact) = read(&src, Limits::default().with(kind, Limit::Finite(used)));
        assert_eq!(result.unwrap(), document);
        assert!(diag.is_empty());
        for other in Kind::ALL {
            assert_eq!(exact.used(other), baseline.used(other));
        }
        if used > 0 {
            let (result, _, _) = read(&src, Limits::default().with(kind, Limit::Finite(used - 1)));
            assert!(
                matches!(result, Err(Error::Stopped(Stop::Exceeded(exceeded))) if exceeded.kind == kind),
                "{kind:?}: {result:?}"
            );
        }
    }
}
