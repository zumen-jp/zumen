//! open の文字コード指定を、属性の編集と ifcZIP の書き戻しにも保つ（#540）。
#![cfg(feature = "bim")]
#![cfg(test)]

use zumen_abi1::{Session, Status};
use zumen_base::CancelToken;
use zumen_json::Value;

fn call(session: &mut Session, request: &str) -> Value {
    let reply = session.call(request.as_bytes(), &CancelToken::new());
    assert_eq!(reply.status, Status::Ok, "{request}: {:?}", reply.response);
    zumen_json::parse(&reply.response).unwrap()
}

fn field<'a>(value: &'a Value, pointer: &str) -> &'a str {
    let Some(Value::String(text)) = value.pointer(pointer) else {
        panic!("{pointer}: {value:?}");
    };
    text
}

fn source() -> Vec<u8> {
    br"ISO-10303-21;
HEADER;
FILE_DESCRIPTION((''),'2;1');
FILE_NAME('','',(''),(''),'','','');
FILE_SCHEMA(('IFC4'));
ENDSEC;
DATA;
#1=IFCWALL('0000000000000000000001',$,'\X\B0',$,$,$,$,$,.NOTDEFINED.);
ENDSEC;
END-ISO-10303-21;
"
    .to_vec()
}

#[test]
fn explicit_encoding_survives_attribute_edits_in_spf_and_zip() {
    for zipped in [false, true] {
        for (encoding, expected) in [("iso_8859_1", "°"), ("shift_jis", "ｰ")] {
            let source = source();
            let bytes = if zipped {
                zumen_zip::write(&[("model.ifc", &source)]).unwrap()
            } else {
                source
            };
            let mut session = Session::new(b"").unwrap();
            session.push(b"input", &bytes).unwrap();
            call(
                &mut session,
                &format!(
                    r#"{{"v":1,"op":"open","input":"input","ifc_text_encoding":"{encoding}"}}"#
                ),
            );
            let elements =
                r#"{"v":1,"op":"building_elements","doc":"input","components":["identity"]}"#;
            let before = call(&mut session, elements);
            assert_eq!(field(&before, "/result/elements/0/identity/name"), expected);
            let id = field(&before, "/result/elements/0/id");
            let hash = field(&before, "/result/elements/0/hash");
            call(
                &mut session,
                &format!(
                    r#"{{"v":1,"op":"building_apply","doc":"input","edits":[{{"kind":"attribute","id":"{id}","hash":"{hash}","attribute":"description","value":"更新"}}]}}"#
                ),
            );
            let written = session.take(0).unwrap();
            assert_eq!(written.starts_with(b"PK"), zipped);
            let after = call(&mut session, elements);
            assert_eq!(field(&after, "/result/elements/0/id"), id);
            assert_eq!(field(&after, "/result/elements/0/identity/name"), expected);
            assert_eq!(
                field(&after, "/result/elements/0/identity/description"),
                "更新"
            );
            assert_ne!(field(&after, "/result/elements/0/hash"), hash);
            session.push(b"written", &written).unwrap();
            call(
                &mut session,
                &format!(
                    r#"{{"v":1,"op":"open","input":"written","ifc_text_encoding":"{encoding}"}}"#
                ),
            );
            let reopened = call(&mut session, &elements.replace("input", "written"));
            assert_eq!(after.pointer("/result"), reopened.pointer("/result"));
        }
    }
}
