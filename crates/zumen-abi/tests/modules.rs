//! wasm のモジュールを分けて作る（決定 0013・0221）。表は schema/boundary.toml から生成する。
//!
//! モジュールの試験は、そのモジュールの feature だけを入れて流す:
//! `cargo test -p zumen-abi --no-default-features --features drawing --test modules`（bim も同じ）。

use zumen_abi1::Session;
use zumen_abi1::abi::{DETECTION, MODULES, Status, has_op};
use zumen_acad::{Dwg, Dxf};
use zumen_base::CancelToken;
use zumen_ifc::{Ifc, IfcZip};
use zumen_jww::Jww;
use zumen_landxml::LandXml;
use zumen_las::Las;
use zumen_ply::Ply;
use zumen_sdk::Codec;
use zumen_sxf::Sxf;

const DXF: &[u8] = b"0\nSECTION\n2\nENTITIES\n0\nENDSEC\n0\nEOF\n";
const IFC: &[u8] = b"ISO-10303-21;\nHEADER;\nFILE_DESCRIPTION((''),'2;1');\n\
FILE_NAME('','',(''),(''),'','','');\nFILE_SCHEMA(('IFC4'));\nENDSEC;\nDATA;\n\
#1=IFCPROJECT('0000000000000000000001',$,'Project',$,$,$,$,$,$);\nENDSEC;\nEND-ISO-10303-21;\n";

fn call(session: &mut Session, request: &str) -> (Status, String) {
    let reply = session.call(request.as_bytes(), &CancelToken::new());
    (
        reply.status,
        String::from_utf8(reply.response).unwrap_or_default(),
    )
}

#[test]
fn detection_follows_the_codecs_in_the_order_open_tries() {
    let expected = [
        ("ifc", Ifc::SIGNATURES),
        ("las", <Las as Codec>::SIGNATURES),
        ("ply", Ply::SIGNATURES),
        ("dxf", Dxf::SIGNATURES),
        ("dwg", Dwg::SIGNATURES),
        ("sxf", Sxf::SIGNATURES),
        ("jww", Jww::SIGNATURES),
        ("landxml", LandXml::SIGNATURES),
        ("ifc", IfcZip::SIGNATURES),
    ];
    assert_eq!(DETECTION.len(), expected.len());
    for (detection, (format, rules)) in DETECTION.iter().zip(expected) {
        assert_eq!(detection.format, format);
        assert_eq!(detection.rules, rules, "{format}");
        let module = MODULES.iter().find(|m| m.name == detection.module).unwrap();
        assert!(module.formats.contains(&format), "{format}");
    }
}

#[test]
fn answers_only_the_ops_of_the_modules_it_has() {
    let enabled = |name: &str| match name {
        "drawing" => cfg!(feature = "drawing"),
        "bim" => cfg!(feature = "bim"),
        other => panic!("知らないモジュール {other}"),
    };
    let mut session = Session::new(b"").unwrap();
    let (status, capabilities) = call(&mut session, r#"{"v":1,"op":"capabilities"}"#);
    assert_eq!(status, Status::Ok);
    for module in MODULES {
        for op in module.ops {
            let expected = MODULES
                .iter()
                .any(|m| enabled(m.name) && m.ops.contains(op));
            assert_eq!(has_op(op), expected, "{op}");
            assert_eq!(
                capabilities.contains(&format!(r#""{op}""#)),
                expected,
                "{op}"
            );
            if !expected {
                // 欄を読む前に断る
                let (status, response) = call(&mut session, &format!(r#"{{"v":1,"op":"{op}"}}"#));
                assert_eq!(status, Status::Failed, "{op}");
                assert!(
                    response.contains(r#""code":"call.op.unsupported""#),
                    "{op}: {response}"
                );
            }
        }
    }
}

#[test]
fn opens_only_the_formats_of_the_modules_it_has() {
    let mut session = Session::new(b"").unwrap();
    session.push(b"a.dxf", DXF).unwrap();
    session.push(b"a.ifc", IFC).unwrap();
    let (status, response) = call(&mut session, r#"{"v":1,"op":"open","input":"a.dxf"}"#);
    if cfg!(feature = "drawing") {
        assert_eq!(status, Status::Ok, "{response}");
    } else {
        assert!(
            response.contains(r#""code":"call.format.unsupported""#),
            "{response}"
        );
    }
    let (status, response) = call(&mut session, r#"{"v":1,"op":"open","input":"a.ifc"}"#);
    if cfg!(feature = "bim") {
        assert_eq!(status, Status::Ok, "{response}");
        assert!(response.contains(r#""format":"ifc""#), "{response}");
    } else {
        // 図面のモジュールでは、同じ始まりの SXF として読み、スキーマが合わずに失敗する
        assert_eq!(status, Status::Failed, "{response}");
    }
}
