//! Shift_JIS の文字境界と名前。原本のバイト位置を保つ（#824）。

#![cfg(test)]

use zumen_base::{Budget, CancelToken, Diagnostics, Kind, Limit, Limits, Stop, Value, codes};
use zumen_sdk::{Cx, Error};
use zumen_xml::{Document, Encoding, Node};

fn encode(text: &str) -> Vec<u8> {
    let (bytes, _, error) = encoding_rs::SHIFT_JIS.encode(text);
    assert!(!error);
    bytes.into_owned()
}

fn read(src: &[u8], limits: Limits) -> (Result<Document, Error>, Diagnostics, Budget) {
    let mut budget = Budget::new(limits);
    let mut diag = Diagnostics::new();
    let cancel = CancelToken::new();
    let result = zumen_xml::read(src, &mut Cx::new(&mut budget, &mut diag, &cancel));
    (result, diag, budget)
}

#[test]
fn reads_names_attributes_prefixes_and_doctype_at_original_byte_positions() {
    // Python の cp932 と独立に確認した符号。2 バイト目は [・]・バックスラッシュ。
    assert_eq!(
        encode("ゼゾソー"),
        [0x83, 0x5b, 0x83, 0x5d, 0x83, 0x5c, 0x81, 0x5b]
    );
    let text = "<?xml version='1.0' encoding='Shift_JIS'?><!DOCTYPE ソ:ボーリング情報 SYSTEM 'sample.dtd'><ソ:ボーリング情報 xmlns:ソ='urn:根' ソ:ゾ='表' plain='a'><ソ:ゼー>文字<![CDATA[表]]></ソ:ゼー></ソ:ボーリング情報>";
    let src = encode(text);
    let (result, diag, _) = read(&src, Limits::default());
    let doc = result.unwrap();
    assert_eq!(doc.src(), src);
    assert_eq!(doc.encoding(), Encoding::ShiftJis);
    assert_eq!(
        diag.iter().map(|d| d.code()).collect::<Vec<_>>(),
        [codes::XML_DTD_DROPPED]
    );
    let root = doc.element(doc.root()).unwrap();
    assert_eq!(doc.local_name_text(root.name()), "ボーリング情報");
    assert_eq!(doc.bytes(root.name().raw()), encode("ソ:ボーリング情報"));
    assert_eq!(doc.local_name(root.name()), encode("ボーリング情報"));
    assert_eq!(doc.parsed_local_name(root.name()), encode("ボーリング情報"));
    assert_eq!(doc.bytes(root.name().prefix().unwrap()), encode("ソ"));
    assert_eq!(doc.namespace(root.name()).as_deref(), Some("urn:根"));
    let start = encode(
        text.get(..text.find("<ソ:ボーリング情報").unwrap())
            .unwrap(),
    )
    .len();
    assert_eq!(root.span(), start..src.len());
    assert_eq!(
        doc.bytes(doc.doctype().unwrap()),
        encode("<!DOCTYPE ソ:ボーリング情報 SYSTEM 'sample.dtd'>")
    );
    let attribute = root
        .attributes()
        .iter()
        .find(|a| doc.local_name_text(a.name()) == "ゾ")
        .unwrap();
    assert_eq!(doc.value(attribute), "表");
    assert_eq!(doc.namespace(attribute.name()).as_deref(), Some("urn:根"));
    assert_eq!(doc.value(doc.attribute(root, None, b"plain").unwrap()), "a");
    let child = doc.element(1).unwrap();
    assert_eq!(doc.local_name_text(child.name()), "ゼー");
    assert_eq!(doc.bytes(child.name().raw()), encode("ソ:ゼー"));
    assert_eq!(child.parent(), Some(0));
    let [Node::Text(text), Node::CData(cdata)] = child.children() else {
        panic!()
    };
    assert_eq!(doc.text(text.clone()), "文字");
    assert_eq!(doc.cdata(cdata.clone()), "表");
}

#[test]
fn missing_prefix_diagnostics_keep_the_decoded_name_and_original_range() {
    let src = encode("<?xml version='1.0' encoding='windows-31j'?><ソ:ゼー/>");
    let (result, diag, _) = read(&src, Limits::default());
    let doc = result.unwrap();
    let diagnostic = diag.iter().next().unwrap();
    assert_eq!(diagnostic.code(), codes::XML_NAMESPACE_MISSING);
    assert_eq!(
        diagnostic
            .params()
            .iter()
            .find(|(key, _)| *key == "prefix")
            .map(|(_, value)| value),
        Some(&Value::Text("ソ".into()))
    );
    let root = doc.element(doc.root()).unwrap();
    assert_eq!(
        diagnostic.places()[0].bytes().unwrap().start,
        root.name().raw().start as u64
    );
    assert_eq!(
        diagnostic.places()[0].bytes().unwrap().end,
        root.name().raw().end as u64
    );
}

#[test]
fn rejects_bad_encoded_names_and_separately_rejects_invalid_qnames() {
    let declaration = b"<?xml version='1.0' encoding='Shift_JIS'?><";
    for name in [
        &[0x81][..],
        &[0x81, 0x7f],
        &[0x81, 0xad],
        &[0xfd],
        &[0xff],
        &[0x81, b'>'],
    ] {
        let mut src = declaration.to_vec();
        src.extend_from_slice(name);
        src.extend_from_slice(b"/>");
        let (result, diag, _) = read(&src, Limits::default());
        assert!(matches!(result, Err(Error::Invalid)), "{name:?}: {diag:?}");
        assert_eq!(
            diag.iter().next().unwrap().code(),
            codes::XML_DOCUMENT_INVALID
        );
    }
    // 0x8140 は有効な符号の U+3000 だが、QName の文字ではない。
    for text in ["<　/>", "<a:　/>", "<a:b:c/>", "<a ゾ:='x'/>"] {
        let src = encode(&format!("<?xml version='1.0' encoding='Shift_JIS'?>{text}"));
        assert!(matches!(
            read(&src, Limits::default()).0,
            Err(Error::Invalid)
        ));
    }
}

#[test]
fn names_and_doctype_stop_at_all_budget_boundaries() {
    let src = encode(
        "<?xml version='1.0' encoding='Shift_JIS'?><!DOCTYPE ボーリング情報 SYSTEM 'x.dtd'><ボーリング情報 ゾ='表'><ゼー/></ボーリング情報>",
    );
    let (result, _, baseline) = read(&src, Limits::default());
    let doc = result.unwrap();
    for kind in Kind::ALL {
        if kind == Kind::Depth {
            continue;
        }
        let used = baseline.used(kind);
        let (result, _, exact) = read(&src, Limits::default().with(kind, Limit::Finite(used)));
        assert_eq!(result.unwrap(), doc);
        for other in Kind::ALL {
            assert_eq!(exact.used(other), baseline.used(other));
        }
        if used != 0 {
            assert!(
                matches!(read(&src, Limits::default().with(kind, Limit::Finite(used - 1))).0,
                Err(Error::Stopped(Stop::Exceeded(e))) if e.kind == kind)
            );
        }
    }
    assert!(
        read(&src, Limits::default().with(Kind::Depth, Limit::Finite(2)))
            .0
            .is_ok()
    );
    assert!(
        matches!(read(&src, Limits::default().with(Kind::Depth, Limit::Finite(1))).0,
        Err(Error::Stopped(Stop::Exceeded(e))) if e.kind == Kind::Depth)
    );
}

#[test]
fn signals_stop_before_reading_and_values_keep_the_existing_replacement_policy() {
    let mut src = encode("<?xml version='1.0' encoding='Shift_JIS'?><ゼー a='");
    src.push(0x81);
    src.extend_from_slice(b"'>");
    src.push(0x81);
    src.extend_from_slice(&encode("</ゼー>"));
    let doc = read(&src, Limits::default()).0.unwrap();
    let root = doc.element(doc.root()).unwrap();
    assert_eq!(doc.value(&root.attributes()[0]), "\u{fffd}");
    let Node::Text(text) = &root.children()[0] else {
        panic!()
    };
    assert_eq!(doc.text(text.clone()), "\u{fffd}");
    for signal in [Stop::Cancelled, Stop::TimedOut] {
        let cancel = CancelToken::new();
        match signal {
            Stop::Cancelled => cancel.cancel(),
            Stop::TimedOut => cancel.expire(),
            _ => {}
        }
        let mut budget = Budget::default();
        let mut diag = Diagnostics::new();
        assert_eq!(
            zumen_xml::read(&src, &mut Cx::new(&mut budget, &mut diag, &cancel)),
            Err(Error::Stopped(signal))
        );
        assert!(Kind::ALL.iter().all(|kind| budget.used(*kind) == 0));
    }
}
