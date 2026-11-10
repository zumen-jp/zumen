//! e8d389a3 と現在の UTF-8・Shift_JIS の公開値と予算を、同じソースで比べる。

use zumen_base::{Budget, CancelToken, Diagnostics, Kind, Value};
use zumen_landxml::LandXml;
use zumen_sdk::{Codec, Cx, FileModel, Lift};
use zumen_xml::{Document, Name, Node};

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn name(doc: &Document, name: &Name) -> String {
    format!(
        "{:?} {:?} {:?} {:?} {} {:?}",
        name.raw(),
        name.local(),
        name.prefix(),
        name.namespace(),
        hex(doc.local_name(name)),
        doc.namespace(name)
    )
}

fn document(doc: &Document, out: &mut String) {
    out.push_str(&format!(
        "document {:?} {} {:?}\n",
        doc.encoding(),
        hex(doc.src()),
        doc.doctype()
    ));
    for (index, element) in doc.elements().iter().enumerate() {
        out.push_str(&format!(
            "element {index} {:?} {:?} {}\n",
            element.span(),
            element.parent(),
            name(doc, element.name())
        ));
        for attr in element.attributes() {
            out.push_str(&format!(
                "attribute {} {:?} {:?}\n",
                name(doc, attr.name()),
                attr.raw(),
                doc.value(attr)
            ));
        }
        for node in element.children() {
            match node {
                Node::Element(index) => out.push_str(&format!("child {index}\n")),
                Node::Text(range) => {
                    out.push_str(&format!("text {range:?} {:?}\n", doc.text(range.clone())))
                }
                Node::CData(range) => {
                    out.push_str(&format!("cdata {range:?} {:?}\n", doc.cdata(range.clone())))
                }
            }
        }
    }
}

fn context(diag: &Diagnostics, budget: &Budget, out: &mut String) {
    for diagnostic in diag.iter() {
        out.push_str(&format!(
            "diagnostic {} {} {:?} {:?} {:?} {:?}\n",
            diagnostic.code().name(),
            diagnostic.count(),
            diagnostic.severity(),
            diagnostic.fidelity(),
            diagnostic.places(),
            diagnostic.spec()
        ));
        for (name, value) in diagnostic.params() {
            match value {
                Value::Float(value) => {
                    out.push_str(&format!("param {name} float {:016x}\n", value.to_bits()))
                }
                value => out.push_str(&format!("param {name} {value:?}\n")),
            }
        }
    }
    for kind in Kind::ALL {
        out.push_str(&format!("budget {} {}\n", kind.name(), budget.used(kind)));
    }
}

/// 新しい UTF-16 の API を使わず、固定の旧文字コードの入力だけを読む。
pub fn snapshot() -> Result<String, String> {
    let utf8 = "<!DOCTYPE p:r><p:r xmlns:p='urn:root' a='日本&#x1F600;'>日😀<p:子/><![CDATA[本\r\n]]>&unresolved;</p:r>";
    let sjis = b"<?xml version='1.0' encoding='Shift_JIS'?><p:r xmlns:p='urn:r' a='\x93\xfa\x96\x7b'>\x93\xfa<p:\x93\xfa/><![CDATA[\x96\x7b\r\n]]></p:r>";
    let land = "<LandXML xmlns='http://www.landxml.org/schema/LandXML-1.2' version='1.2'><CoordinateSystem horizontalDatum='JGD2011' verticalDatum='T.P' horizontalCoordinateSystemName='9(X,Y)'/><Alignments><Alignment name='本線' staStart='1000' length='10'><CoordGeom><Line length='10'><Start>0 0</Start><End>10 0</End></Line></CoordGeom></Alignment></Alignments></LandXML>";
    let mut out = String::new();
    for (label, bytes) in [("utf8", utf8.as_bytes()), ("shift-jis", &sjis[..])] {
        let mut budget = Budget::default();
        let mut diag = Diagnostics::new();
        let cancel = CancelToken::new();
        let doc = zumen_xml::read(bytes, &mut Cx::new(&mut budget, &mut diag, &cancel))
            .map_err(|e| format!("{label}: {e:?}"))?;
        out.push_str(&format!("case {label}\n"));
        document(&doc, &mut out);
        context(&diag, &budget, &mut out);
    }
    let mut budget = Budget::default();
    let mut diag = Diagnostics::new();
    let cancel = CancelToken::new();
    let mut cx = Cx::new(&mut budget, &mut diag, &cancel);
    let file = LandXml::read(land.as_bytes(), &mut cx).map_err(|e| format!("landxml: {e:?}"))?;
    let model = file.lift(&mut cx).map_err(|e| format!("landxml: {e:?}"))?;
    let mut written = Vec::new();
    LandXml::write(&file, &mut written, &mut cx).map_err(|e| format!("landxml: {e:?}"))?;
    out.push_str("case landxml\n");
    document(file.document(), &mut out);
    out.push_str(&format!("model {model:?}\nwritten {}\n", hex(&written)));
    for index in 0..file.record_count() {
        let record = file
            .record(index)
            .ok_or_else(|| "レコードがない".to_string())?;
        out.push_str(&format!("record {} {:?}\n", hex(record.kind), record.bytes));
    }
    context(&diag, &budget, &mut out);
    Ok(out)
}
