//! XML（L0）の fuzz の target（#501）。要素・属性・文字・名前空間を読み、文字と属性の値を戻す。要素の入れ子が予算の
//! 深さで止まり、文書型宣言の実体（外部の実体と、実体を重ねて膨らむ文書）を戻さないことを確かめる。

#![no_main]

use libfuzzer_sys::fuzz_target;
use zumen_xml::Node;

fuzz_target!(|data: &[u8]| {
    let read = zumen_sdk::testkit::survives_with(data, |src, cx| {
        let doc = zumen_xml::read(src, cx)?;
        // 戻した文字と属性の値のバイト数。実体を戻さないので、読めない 1 バイトを置き換えの文字（3 バイト）にする
        // ときの 3 倍を超えない
        let mut decoded = 0;
        for element in doc.elements() {
            let _ = doc.local_name_text(element.name());
            let _ = doc.namespace(element.name());
            for attribute in element.attributes() {
                let _ = doc.local_name_text(attribute.name());
                let _ = doc.namespace(attribute.name());
                decoded += doc.value(attribute).len();
            }
            for node in element.children() {
                decoded += match node {
                    Node::Element(_) => 0,
                    Node::Text(range) => doc.text(range.clone()).len(),
                    Node::CData(range) => doc.cdata(range.clone()).len(),
                };
            }
        }
        assert!(
            decoded <= 3 * src.len(),
            "{} バイトの文書から、{decoded} バイトの文字を戻した",
            src.len()
        );
        Ok(())
    });
    if let Err(message) = read {
        panic!("{message}");
    }
});
