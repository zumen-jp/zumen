//! XML の L0 の読み取り（決定 0209）。入力は、どれも試験の中で作る。

use zumen_base::{
    Budget, CancelToken, Code, Cx, Diagnostics, Kind, Limit, Limits, Ref, Stop, codes,
};
use zumen_sdk::Error;
use zumen_xml::{Document, Encoding, Namespace, Node};

fn read_with(src: &[u8], limits: Limits) -> (Result<Document, Error>, Diagnostics) {
    let mut budget = Budget::new(limits);
    let mut diag = Diagnostics::new();
    let cancel = CancelToken::new();
    let result = zumen_xml::read(src, &mut Cx::new(&mut budget, &mut diag, &cancel));
    (result, diag)
}

fn read(src: &[u8]) -> (Result<Document, Error>, Diagnostics) {
    read_with(src, Limits::default())
}

fn found(diag: &Diagnostics) -> Vec<Code> {
    diag.iter().map(|d| d.code()).collect()
}

const DOC: &[u8] = br#"<?xml version="1.0" encoding="UTF-8"?>
<!-- before -->
<r:root xmlns:r="urn:root" xmlns="urn:default" r:id="1" plain='a &amp; b'>
  <item kind="x">one &lt;two&gt; &#x41;&#66;</item>
  <inner xmlns="">
    <bare/>
  </inner>
  <![CDATA[<raw> &amp;]]>
  <?pi ignored?>
</r:root>
<!-- after -->
"#;

#[test]
fn reads_elements_attributes_and_text_with_byte_ranges() {
    let (doc, diag) = read(DOC);
    let doc = doc.unwrap();
    assert!(diag.is_empty(), "{diag:?}");
    assert_eq!(doc.encoding(), Encoding::Utf8);
    let names: Vec<_> = doc
        .elements()
        .iter()
        .map(|e| String::from_utf8_lossy(doc.bytes(e.name().raw())).into_owned())
        .collect();
    assert_eq!(names, ["r:root", "item", "inner", "bare"]);
    let root = doc.element(doc.root()).unwrap();
    assert!(doc.bytes(root.span()).starts_with(b"<r:root "));
    assert!(doc.bytes(root.span()).ends_with(b"</r:root>"));
    assert_eq!(root.parent(), None);
    assert_eq!(doc.local_name(root.name()), b"root");
    assert_eq!(doc.bytes(root.name().prefix().unwrap()), b"r");

    // 属性は書いた順。値は参照を戻して読む
    let plain = doc.attribute(root, None, b"plain").unwrap();
    assert_eq!(doc.bytes(plain.raw()), b"a &amp; b");
    assert_eq!(doc.value(plain), "a & b");
    let id = doc.attribute(root, Some("urn:root"), b"id").unwrap();
    assert_eq!(doc.value(id), "1");
    assert_eq!(root.attributes().len(), 4);

    // 中身。注記と処理命令は入れない
    let item = doc.element(1).unwrap();
    assert_eq!(item.parent(), Some(0));
    let Some(Node::Text(text)) = item.children().first() else {
        panic!("{item:?}");
    };
    assert_eq!(doc.text(text.clone()), "one <two> AB");
    let cdata: Vec<_> = root
        .children()
        .iter()
        .filter_map(|node| match node {
            Node::CData(range) => Some(doc.cdata(range.clone())),
            _ => None,
        })
        .collect();
    assert_eq!(cdata, ["<raw> &amp;"]);
    let children: Vec<_> = root
        .children()
        .iter()
        .filter_map(|node| match node {
            Node::Element(index) => Some(*index),
            _ => None,
        })
        .collect();
    assert_eq!(children, [1, 2]);
}

#[test]
fn resolves_namespaces_by_scope() {
    let (doc, _) = read(DOC);
    let doc = doc.unwrap();
    let ns = |index: usize| doc.namespace(doc.element(index).unwrap().name());
    assert_eq!(ns(0).as_deref(), Some("urn:root"));
    // 接頭辞のない要素は、既定の名前空間に入る。`xmlns=""` で外す
    assert_eq!(ns(1).as_deref(), Some("urn:default"));
    assert_eq!(ns(2), None);
    assert_eq!(ns(3), None);
    // 接頭辞のない属性は、名前空間に入らない。宣言の属性は xmlns の名前空間
    let root = doc.element(0).unwrap();
    let kind = doc.attribute(doc.element(1).unwrap(), None, b"kind");
    assert!(kind.is_some());
    let declarations = root
        .attributes()
        .iter()
        .filter(|a| a.name().namespace() == Some(&Namespace::Xmlns))
        .count();
    assert_eq!(declarations, 2);
    assert!(doc.is(root.name(), Some("urn:root"), b"root"));
    assert!(!doc.is(root.name(), Some("urn:default"), b"root"));

    // 宣言されていない接頭辞は報告し、名前空間のない名前にする
    let (doc, diag) = read(b"<a:x xml:lang='ja'><a:y/></a:x>");
    let doc = doc.unwrap();
    assert_eq!(found(&diag), [codes::XML_NAMESPACE_MISSING]);
    assert_eq!(diag.iter().next().unwrap().count(), 2);
    assert_eq!(doc.namespace(doc.element(0).unwrap().name()), None);
    let lang = doc.element(0).unwrap().attributes().first().unwrap();
    assert_eq!(lang.name().namespace(), Some(&Namespace::Xml));
}

#[test]
fn normalizes_line_ends_and_attribute_spaces() {
    let (doc, _) = read(b"<a v=\"x\r\ny\tz&#10;\">l1\r\nl2\rl3</a>");
    let doc = doc.unwrap();
    let a = doc.element(0).unwrap();
    assert_eq!(doc.value(&a.attributes()[0]), "x y z\n");
    let Some(Node::Text(text)) = a.children().first() else {
        panic!("{a:?}");
    };
    assert_eq!(doc.text(text.clone()), "l1\nl2\nl3");
}

#[test]
fn reads_shift_jis_documents_from_the_original_bytes() {
    // 「表示」は 0x95 0x5C 0x8E 0xA6。2 バイト目の 0x5C（\）も、マークアップとは取り違えない
    let mut src = b"<?xml version=\"1.0\" encoding=\"Shift_JIS\"?><a t=\"".to_vec();
    src.extend_from_slice(&[0x95, 0x5c, 0x8e, 0xa6]);
    src.extend_from_slice(b"\">");
    src.extend_from_slice(&[0x95, 0x5c, 0x8e, 0xa6]);
    src.extend_from_slice(b" &amp;</a>");
    let (doc, diag) = read(&src);
    let doc = doc.unwrap();
    assert!(diag.is_empty());
    assert_eq!(doc.encoding(), Encoding::ShiftJis);
    let a = doc.element(0).unwrap();
    assert_eq!(doc.value(&a.attributes()[0]), "表示");
    let Some(Node::Text(text)) = a.children().first() else {
        panic!("{a:?}");
    };
    assert_eq!(doc.text(text.clone()), "表示 &");
    // 別名も同じ文字コード
    let (doc, _) = read(b"<?xml version='1.0' encoding='windows-31j'?><a/>");
    assert_eq!(doc.unwrap().encoding(), Encoding::ShiftJis);
}

#[test]
fn other_encodings_are_not_read() {
    for (src, encoding) in [
        (
            &b"<?xml version=\"1.0\" encoding=\"EUC-JP\"?><a/>"[..],
            "EUC-JP",
        ),
        (b"\xff\xfe<\0a\0/\0>\0", "UTF-16"),
        (b"<\0a\0/\0>\0", "UTF-16"),
        (
            b"\xef\xbb\xbf<?xml version=\"1.0\" encoding=\"Shift_JIS\"?><a/>",
            "Shift_JIS",
        ),
    ] {
        let (result, diag) = read(src);
        assert!(matches!(result, Err(Error::Unsupported)), "{src:?}");
        assert_eq!(found(&diag), [codes::XML_ENCODING_UNSUPPORTED]);
        assert_eq!(
            diag.iter().next().unwrap().param("encoding"),
            Some(&zumen_base::Value::Text(encoding.to_owned()))
        );
    }
    // 宣言のない文書と UTF-8 の印は UTF-8
    let (doc, _) = read(b"\xef\xbb\xbf<a/>");
    assert_eq!(doc.unwrap().encoding(), Encoding::Utf8);
}

#[test]
fn does_not_process_the_dtd() {
    let src = br#"<?xml version="1.0"?>
<!DOCTYPE a SYSTEM "http://example.invalid/a.dtd" [
  <!ENTITY e "expanded">
  <!ENTITY x SYSTEM "file:///etc/passwd">
  <!-- ] > -->
]>
<a>&e; &x; &lt;</a>"#;
    let (doc, diag) = read(src);
    let doc = doc.unwrap();
    let mut codes_found = found(&diag);
    codes_found.sort_by_key(|code| code.name());
    // 実体は名前ごとに分けて報告する
    assert_eq!(
        codes_found,
        [
            codes::XML_DTD_DROPPED,
            codes::XML_ENTITY_UNSUPPORTED,
            codes::XML_ENTITY_UNSUPPORTED
        ]
    );
    let names: Vec<_> = diag
        .iter()
        .filter(|d| d.code() == codes::XML_ENTITY_UNSUPPORTED)
        .filter_map(|d| d.param("name").cloned())
        .collect();
    assert_eq!(
        names,
        [
            zumen_base::Value::Text("e".to_owned()),
            zumen_base::Value::Text("x".to_owned())
        ]
    );
    assert!(doc.bytes(doc.doctype().unwrap()).ends_with(b"]>"));
    // 宣言した実体は戻さず、そのままの文字にする
    let a = doc.element(0).unwrap();
    let Some(Node::Text(text)) = a.children().first() else {
        panic!("{a:?}");
    };
    assert_eq!(doc.text(text.clone()), "&e; &x; <");
}

#[test]
fn broken_documents_are_invalid_at_the_place() {
    for (src, place) in [
        (&b"<a><b></a>"[..], &b"</a>"[..]),
        (b"<a>", b"<a>"),
        (b"<a/><b/>", b"<b/>"),
        (b"<a/>text", b"text"),
        (b"<a x='1' x='2'/>", b"x"),
        (b"<a x=1/>", b"x=1"),
        (b"<a x='<'/>", b"x='<'"),
        (b"<a>&#xZZ;</a>", b"&#xZZ;"),
        (b"<a>&amp</a>", b"&amp"),
        (b"<a>&#0;</a>", b"&#0;"),
        (b"<a:b:c/>", b"a:b:c"),
        (b"<a xmlns:p=''/>", b"xmlns:p"),
        (b"<!-- open", b"<!-- open"),
        (b"", b""),
        (b"  ", b""),
        (b"<?xml encoding='UTF-8'?><a/>", b"<?xml encoding='UTF-8'?>"),
    ] {
        let (result, diag) = read(src);
        assert!(matches!(result, Err(Error::Invalid)), "{src:?}");
        let d = diag.iter().next().unwrap();
        assert_eq!(d.code(), codes::XML_DOCUMENT_INVALID, "{src:?}");
        let bytes = d.places().first().and_then(Ref::bytes).unwrap();
        assert_eq!(
            &src[bytes.start as usize..bytes.end as usize],
            place,
            "{}",
            String::from_utf8_lossy(src)
        );
    }
}

#[test]
fn budgets_stop_at_elements_and_depth() {
    let (result, diag) = read_with(
        b"<a><b/><c/><d/></a>",
        Limits::default().with(Kind::Records, Limit::Finite(2)),
    );
    assert!(matches!(result, Err(Error::Stopped(Stop::Exceeded(_)))));
    let stop = diag.iter().next().unwrap();
    assert_eq!(stop.code(), codes::BUDGET_RECORDS_EXCEEDED);
    let place = stop.places().first().and_then(Ref::bytes).unwrap();
    assert_eq!(place.start, 7);

    let deep = format!("{}{}", "<a>".repeat(10), "</a>".repeat(10));
    let (result, diag) = read_with(
        deep.as_bytes(),
        Limits::default().with(Kind::Depth, Limit::Finite(5)),
    );
    assert!(matches!(result, Err(Error::Stopped(_))));
    assert_eq!(found(&diag), [codes::BUDGET_DEPTH_EXCEEDED]);
    let (result, _) = read_with(
        deep.as_bytes(),
        Limits::default().with(Kind::Depth, Limit::Finite(10)),
    );
    assert_eq!(result.unwrap().elements().len(), 10);
    // 空要素も、入れ子の 1 段に数えてから戻す
    let siblings = format!("<a>{}</a>", "<b/>".repeat(100));
    let (result, _) = read_with(
        siblings.as_bytes(),
        Limits::default().with(Kind::Depth, Limit::Finite(2)),
    );
    assert_eq!(result.unwrap().elements().len(), 101);

    let (result, diag) = read_with(
        DOC,
        Limits::default().with(Kind::InputBytes, Limit::Finite(10)),
    );
    assert!(matches!(result, Err(Error::Stopped(_))));
    assert_eq!(found(&diag), [codes::BUDGET_INPUT_BYTES_EXCEEDED]);
}

/// どこで切っても、どのバイトを変えても、パニックせずに読むか断る
#[test]
fn truncated_and_mutated_documents_do_not_panic() {
    for end in 0..DOC.len() {
        let _ = read(&DOC[..end]);
    }
    for at in 0..DOC.len() {
        for byte in [b'<', b'>', b'&', b'"', b'\'', b':', b'/', 0x95, 0] {
            let mut src = DOC.to_vec();
            src[at] = byte;
            let _ = read(&src);
        }
    }
}
