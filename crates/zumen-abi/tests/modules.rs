//! wasm のモジュールを分けて作る（決定 0013・0221）。表は schema/boundary.toml から生成する。
//!
//! モジュールのテストは、そのモジュールの feature だけを入れて流す:
//! `cargo test -p zumen-abi --no-default-features --features drawing --test modules`（bim・civil・pointcloud・plugin も
//! 同じ）。

use zumen_abi1::Session;
use zumen_abi1::abi::{DETECTION, MODULES, Status, has_format, has_op};
use zumen_acad::{Dwg, Dxf};
use zumen_base::CancelToken;
use zumen_citygml::CityGml;
use zumen_e57::E57;
use zumen_fgd::Fgd;
use zumen_ifc::{Ifc, IfcZip};
use zumen_jww::Jww;
use zumen_landxml::LandXml;
use zumen_las::Las;
use zumen_ply::Ply;
use zumen_sdk::Codec;
use zumen_sima::Sima;
use zumen_sxf::Sxf;
use zumen_tiff::Tiff;

const DXF: &[u8] = b"0\nSECTION\n2\nENTITIES\n0\nENDSEC\n0\nEOF\n";
const IFC: &[u8] = b"ISO-10303-21;\nHEADER;\nFILE_DESCRIPTION((''),'2;1');\n\
FILE_NAME('','',(''),(''),'','','');\nFILE_SCHEMA(('IFC4'));\nENDSEC;\nDATA;\n\
#1=IFCPROJECT('0000000000000000000001',$,'Project',$,$,$,$,$,$);\nENDSEC;\nEND-ISO-10303-21;\n";

#[cfg(test)]
fn enabled(name: &str) -> bool {
    match name {
        "drawing" => cfg!(feature = "drawing"),
        "bim" => cfg!(feature = "bim"),
        "civil" => cfg!(feature = "civil"),
        "pointcloud" => cfg!(feature = "pointcloud"),
        "plugin" => cfg!(feature = "plugin"),
        other => panic!("知らないモジュール {other}"),
    }
}

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
        ("e57", E57::SIGNATURES),
        // 図面モデルのバイナリ（決定 0247・0250）
        (
            "drawing",
            &[zumen_sdk::Signature::Bytes {
                offset: 0,
                bytes: &zumen_drawing::binary::MAGIC,
            }],
        ),
        ("dxf", Dxf::SIGNATURES),
        ("dwg", Dwg::SIGNATURES),
        ("sxf", Sxf::SIGNATURES),
        ("jww", Jww::SIGNATURES),
        ("sima", Sima::SIGNATURES),
        ("tiff", Tiff::SIGNATURES),
        ("landxml", LandXml::SIGNATURES),
        ("citygml", CityGml::SIGNATURES),
        ("fgd", Fgd::SIGNATURES),
        // ZIP に入れた SXF（src/packed.rs の読み取りと同じ名前の終わり）
        (
            "sxf",
            &[zumen_sdk::Signature::ZipEntry {
                suffixes: &[b".p21", b".sfc"],
            }],
        ),
        // 基盤地図情報の配布のアーカイブ（決定 0251）
        ("fgd", zumen_fgd::ARCHIVE),
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
                // フィールドを読む前に断る
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

#[test]
fn lists_only_the_formats_of_the_modules_it_has() {
    let mut session = Session::new(b"").unwrap();
    let (status, capabilities) = call(&mut session, r#"{"v":1,"op":"capabilities"}"#);
    assert_eq!(status, Status::Ok);
    for module in MODULES {
        for format in module.formats {
            let expected = MODULES
                .iter()
                .any(|m| enabled(m.name) && m.formats.contains(format));
            assert_eq!(has_format(format), expected, "{format}");
        }
    }
    // 図面・土木地理・点群の形式（決定 0253）。J-LandXML と TIFF は 3 つのモジュールが持つ
    for format in [
        "dxf", "sima", "tiff", "landxml", "citygml", "fgd", "extdm", "geojson", "las", "ply",
        "xyz", "e57",
    ] {
        assert_eq!(
            capabilities.contains(&format!(r#""id":"{format}""#)),
            has_format(format),
            "{format}"
        );
    }
    assert_eq!(
        has_format("landxml"),
        enabled("drawing") || enabled("civil") || enabled("pointcloud")
    );
    assert_eq!(
        has_format("tiff"),
        enabled("drawing") || enabled("civil") || enabled("pointcloud")
    );
    assert_eq!(has_format("sima"), enabled("drawing") || enabled("civil"));
}

#[test]
fn names_extended_dm_and_xyz_only_in_their_modules() {
    for (format, module) in [
        ("extdm", "civil"),
        ("geojson", "civil"),
        ("xyz", "pointcloud"),
    ] {
        let mut session = Session::new(b"").unwrap();
        session.push(b"a.dat", b"").unwrap();
        let (_, response) = call(
            &mut session,
            &format!(r#"{{"v":1,"op":"open","input":"a.dat","format":"{format}"}}"#),
        );
        // 持たないモジュールでは、名指した形式を値の誤りとして断る
        assert_eq!(
            response.contains(r#""code":"call.value.invalid""#)
                && response.contains(r#""/format""#),
            !enabled(module),
            "{format}: {response}"
        );
    }
}

#[test]
fn opens_sima_in_drawing_and_civil_modules_and_rejects_elsewhere() {
    let sim = b"G00,01,\x8b\xab\x8a\x95,EPSG:6677,\r\nA00,\r\nA01,1,P1,10.0,20.0,1.0,\r\nA99,\r\n";
    let mut session = Session::new(b"").unwrap();
    session.push(b"a.sim", sim).unwrap();
    let (status, response) = call(&mut session, r#"{"v":1,"op":"open","input":"a.sim"}"#);
    if cfg!(any(feature = "drawing", feature = "civil")) {
        assert_eq!(status, Status::Ok, "{response}");
        assert!(response.contains(r#""format":"sima""#), "{response}");
        if cfg!(feature = "civil") {
            let (geo_status, geo_resp) = call(&mut session, r#"{"v":1,"op":"geo","doc":"a.sim"}"#);
            assert_eq!(geo_status, Status::Ok, "{geo_resp}");
            let (conv_status, conv_resp) = call(
                &mut session,
                r#"{"v":1,"op":"convert","doc":"a.sim","to":"b.sim","format":"sima"}"#,
            );
            assert_eq!(conv_status, Status::Ok, "{conv_resp}");
        }
    } else {
        assert_eq!(status, Status::Failed, "{response}");
        assert!(
            response.contains(r#""code":"call.format.unsupported""#),
            "{response}"
        );
    }
}

#[test]
fn opens_tiff_in_drawing_civil_and_pointcloud_modules_and_rejects_elsewhere() {
    // 1x1 8-bit グレースケールの最小クラシック TIFF (LE)
    let mut tiff = Vec::from(&b"II*\x00\x0c\x00\x00\x00\x80\x00\x00\x00"[..]);
    let entries: &[(u16, u16, u32, u32)] = &[
        (256, 4, 1, 1), // ImageWidth = 1
        (257, 4, 1, 1), // ImageLength = 1
        (258, 3, 1, 8), // BitsPerSample = 8
        (259, 3, 1, 1), // Compression = 1
        (262, 3, 1, 1), // Photometric = 1
        (273, 4, 1, 8), // StripOffsets = 8
        (277, 3, 1, 1), // SamplesPerPixel = 1
        (278, 4, 1, 1), // RowsPerStrip = 1
        (279, 4, 1, 1), // StripByteCounts = 1
    ];
    tiff.extend_from_slice(&(entries.len() as u16).to_le_bytes());
    for &(tag, ftype, count, val) in entries {
        tiff.extend_from_slice(&tag.to_le_bytes());
        tiff.extend_from_slice(&ftype.to_le_bytes());
        tiff.extend_from_slice(&count.to_le_bytes());
        tiff.extend_from_slice(&val.to_le_bytes());
    }
    tiff.extend_from_slice(&0u32.to_le_bytes());

    let mut session = Session::new(b"").unwrap();
    session.push(b"a.tif", &tiff).unwrap();
    let (status, response) = call(&mut session, r#"{"v":1,"op":"open","input":"a.tif"}"#);
    if cfg!(any(
        feature = "drawing",
        feature = "civil",
        feature = "pointcloud"
    )) {
        assert_eq!(status, Status::Ok, "{response}");
        assert!(response.contains(r#""format":"tiff""#), "{response}");
        if cfg!(feature = "drawing") {
            let (r_status, r_resp) = call(&mut session, r#"{"v":1,"op":"raster","doc":"a.tif"}"#);
            assert_eq!(r_status, Status::Ok, "{r_resp}");
        }
        if cfg!(feature = "civil") {
            let (g_status, g_resp) = call(&mut session, r#"{"v":1,"op":"geo","doc":"a.tif"}"#);
            assert_eq!(g_status, Status::Ok, "{g_resp}");
        }
        if cfg!(feature = "pointcloud") {
            // DEM でない TIFF は pointcloud では call.document.unsupported になる
            let (p_status, p_resp) =
                call(&mut session, r#"{"v":1,"op":"pointcloud","doc":"a.tif"}"#);
            assert_eq!(p_status, Status::Failed, "{p_resp}");
            assert!(
                p_resp.contains(r#""code":"call.document.unsupported""#),
                "{p_resp}"
            );
        }
    } else {
        assert_eq!(status, Status::Failed, "{response}");
        assert!(
            response.contains(r#""code":"call.format.unsupported""#),
            "{response}"
        );
    }
}
