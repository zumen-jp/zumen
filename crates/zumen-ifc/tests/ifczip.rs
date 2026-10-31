//! ifcZIP の読み書き（決定 0211）。書庫は、試験の中で圧縮せずに組み立てる。

use zumen_base::crc::crc32;
use zumen_base::{Budget, CancelToken, Code, Cx, Diagnostics, Kind, Limit, Limits, codes};
use zumen_ifc::{IfcZip, IfcZipFile};
use zumen_sdk::{Codec, Error, testkit};

const IFC: &str = "ISO-10303-21;\nHEADER;\nFILE_DESCRIPTION((''),'2;1');\n\
FILE_NAME('','',(''),(''),'','','');\nFILE_SCHEMA(('IFC4'));\nENDSEC;\nDATA;\n\
#1=IFCPROJECT('0YvctVUKr0kugbFTf53O9L',$,'Project',$,$,$,$,$,$);\n\
#2=IFCRELAGGREGATES('1YvctVUKr0kugbFTf53O9L',$,$,$,#1,(#3));\n\
#3=IFCSITE('2YvctVUKr0kugbFTf53O9L',$,'Site',$,$,$,$,$,.ELEMENT.,$,$,$,$,$);\n\
ENDSEC;\nEND-ISO-10303-21;\n";

/// 圧縮しない項目の書庫
fn archive(entries: &[(&str, &[u8])]) -> Vec<u8> {
    let mut out = Vec::new();
    let mut central = Vec::new();
    for (name, data) in entries {
        let offset = out.len() as u32;
        let header = |signature: u32| {
            let mut h = signature.to_le_bytes().to_vec();
            if signature == 0x0201_4b50 {
                h.extend_from_slice(&20u16.to_le_bytes());
            }
            h.extend_from_slice(&20u16.to_le_bytes());
            h.extend_from_slice(&(1u16 << 11).to_le_bytes());
            h.extend_from_slice(&0u16.to_le_bytes());
            h.extend_from_slice(&0u32.to_le_bytes());
            h.extend_from_slice(&crc32(data).to_le_bytes());
            h.extend_from_slice(&(data.len() as u32).to_le_bytes());
            h.extend_from_slice(&(data.len() as u32).to_le_bytes());
            h.extend_from_slice(&(name.len() as u16).to_le_bytes());
            h.extend_from_slice(&0u16.to_le_bytes());
            h
        };
        out.extend(header(0x0403_4b50));
        out.extend_from_slice(name.as_bytes());
        out.extend_from_slice(data);
        central.extend(header(0x0201_4b50));
        central.extend_from_slice(&[0; 6]);
        central.extend_from_slice(&0u32.to_le_bytes());
        central.extend_from_slice(&offset.to_le_bytes());
        central.extend_from_slice(name.as_bytes());
    }
    let start = out.len() as u32;
    out.extend_from_slice(&central);
    out.extend_from_slice(&0x0605_4b50u32.to_le_bytes());
    out.extend_from_slice(&[0; 4]);
    out.extend_from_slice(&(entries.len() as u16).to_le_bytes());
    out.extend_from_slice(&(entries.len() as u16).to_le_bytes());
    out.extend_from_slice(&(central.len() as u32).to_le_bytes());
    out.extend_from_slice(&start.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes());
    out
}

fn read_with(src: &[u8], limits: Limits) -> (Result<IfcZipFile, Error>, Diagnostics) {
    let mut budget = Budget::new(limits);
    let mut diag = Diagnostics::new();
    let cancel = CancelToken::new();
    let result = IfcZip::read(src, &mut Cx::new(&mut budget, &mut diag, &cancel));
    (result, diag)
}

fn found(diag: &Diagnostics) -> Vec<Code> {
    diag.iter().map(|d| d.code()).collect()
}

#[test]
fn reads_the_ifc_in_the_archive_and_writes_the_same_bytes() {
    let src = archive(&[
        ("docs/", b""),
        ("docs/readme.txt", b"not ifc"),
        ("model/Model.IFC", IFC.as_bytes()),
    ]);
    let (file, diag) = read_with(&src, Limits::default());
    let file = file.unwrap();
    assert!(diag.is_empty(), "{diag:?}");
    assert_eq!(file.name(), "model/Model.IFC");
    assert_eq!(file.ifc().schema(), b"IFC4");
    assert_eq!(file.ifc().ids().collect::<Vec<_>>(), [1, 2, 3]);
    assert_eq!(
        file.ifc().references(file.ifc().instance(2).unwrap()),
        [1, 3]
    );
    testkit::roundtrip::<IfcZip>(&src).unwrap();
    testkit::records_in_order::<IfcZip>(&src).unwrap();
    testkit::deterministic::<IfcZip>(&src).unwrap();
    assert!(IfcZip::SIGNATURES.iter().all(|s| s.matches(&src)));
}

#[test]
fn needs_exactly_one_ifc() {
    let none = archive(&[("readme.txt", b"x"), ("model.ifc/", b"")]);
    let (result, diag) = read_with(&none, Limits::default());
    assert!(matches!(result, Err(Error::Invalid)));
    assert_eq!(found(&diag), [codes::IFC_ENTRY_MISSING]);

    let two = archive(&[("a.ifc", IFC.as_bytes()), ("b.ifc", IFC.as_bytes())]);
    let (result, diag) = read_with(&two, Limits::default());
    assert!(matches!(result, Err(Error::Unsupported)));
    assert_eq!(found(&diag), [codes::IFC_ENTRY_UNSUPPORTED]);

    // 中が IFC でなければ、IFC-SPF と同じく読まない
    let sxf = IFC.replace("IFC4", "ASSOCIATIVE_DRAUGHTING");
    let (result, diag) = read_with(&archive(&[("a.ifc", sxf.as_bytes())]), Limits::default());
    assert!(matches!(result, Err(Error::Unsupported)));
    assert_eq!(found(&diag), [codes::IFC_SCHEMA_UNSUPPORTED]);

    // 書庫として壊れている
    let (result, diag) = read_with(b"PK\x03\x04", Limits::default());
    assert!(matches!(result, Err(Error::Invalid)));
    assert_eq!(found(&diag), [codes::ZIP_ARCHIVE_INVALID]);
}

#[test]
fn limits_the_expanded_size() {
    let src = archive(&[("model.ifc", IFC.as_bytes())]);
    let limit = IFC.len() as u64 - 1;
    let (result, diag) = read_with(
        &src,
        Limits::default().with(Kind::DecodedBytes, Limit::Finite(limit)),
    );
    assert!(matches!(result, Err(Error::Stopped(_))));
    assert_eq!(found(&diag), [codes::BUDGET_DECODED_BYTES_EXCEEDED]);
    let (result, _) = read_with(
        &src,
        Limits::default().with(Kind::DecodedBytes, Limit::Finite(limit + 1)),
    );
    assert!(result.is_ok());
}
