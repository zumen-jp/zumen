//! 試験の中で作った成果品を読む（決定 0241、#383）。要領の作成例は転載が禁じられているので、要素の名前は
//! 試験のために決めたもの。

use zumen_base::{Budget, CancelToken, Code, Cx, Diagnostics, codes};
use zumen_delivery::{Package, Record, read, read_zip};

const INDEX: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE constdata SYSTEM "INDE_C05.DTD">
<constdata DTD_version="05">
  <基礎情報>
    <メディア番号>1</メディア番号>
    <図面フォルダー名>DRAWING</図面フォルダー名>
    <適用要領基準>土木 202603-01</適用要領基準>
    <版>4.0</版>
  </基礎情報>
</constdata>
"#;

/// Shift_JIS の管理ファイル。「図面情報」「図面ファイル名」を Shift_JIS で書く
fn drawing() -> Vec<u8> {
    let info = [0x90, 0x7D, 0x96, 0xCA, 0x8F, 0xEE, 0x95, 0xF1];
    let file = [
        0x90, 0x7D, 0x96, 0xCA, 0x83, 0x74, 0x83, 0x40, 0x83, 0x43, 0x83, 0x8B, 0x96, 0xBC,
    ];
    let mut out = b"<?xml version=\"1.0\" encoding=\"Shift_JIS\"?>\n<!DOCTYPE drawingdata SYSTEM \"DRAW05.DTD\">\n<drawingdata>".to_vec();
    for name in ["D0PL001Z.P21", "D0PL002Z.P21"] {
        out.extend_from_slice(b"<");
        out.extend_from_slice(&info);
        out.extend_from_slice(b"><");
        out.extend_from_slice(&file);
        out.extend_from_slice(b">");
        out.extend_from_slice(name.as_bytes());
        out.extend_from_slice(b"</");
        out.extend_from_slice(&file);
        out.extend_from_slice(b"></");
        out.extend_from_slice(&info);
        out.extend_from_slice(b">");
    }
    out.extend_from_slice(b"</drawingdata>\n");
    out
}

#[cfg(test)]
fn with_cx<T>(f: impl FnOnce(&mut Cx<'_>) -> T) -> (T, Vec<Code>) {
    let mut budget = Budget::default();
    let mut diag = Diagnostics::new();
    let cancel = CancelToken::new();
    let out = {
        let mut cx = Cx::new(&mut budget, &mut diag, &cancel);
        f(&mut cx)
    };
    (out, diag.iter().map(|d| d.code()).collect())
}

#[cfg(test)]
fn find<'r>(record: &'r Record, name: &str) -> Vec<&'r Record> {
    let mut found: Vec<&Record> = record.children.iter().filter(|r| r.name == name).collect();
    for child in &record.children {
        found.extend(find(child, name));
    }
    found
}

#[cfg(test)]
fn files(drawing: &[u8]) -> Vec<(&'static str, Vec<u8>)> {
    vec![
        ("INDEX_C.XML", INDEX.as_bytes().to_vec()),
        ("DRAWING/DRAWING.XML", drawing.to_vec()),
        ("DRAWING/D0PL001Z.P21", b"ISO-10303-21;".to_vec()),
        ("OTHERS/ORG001/NOTE.XML", b"<a/>".to_vec()),
        ("BROKEN.XML", b"<a><b></a>".to_vec()),
    ]
}

#[test]
fn management_files_are_read_and_their_file_names_resolved() {
    let drawing = drawing();
    let mut inputs = files(&drawing);
    inputs.push(("../evil.txt", Vec::new()));
    let borrowed: Vec<(&str, &[u8])> = inputs.iter().map(|(n, b)| (*n, b.as_slice())).collect();
    let (package, diag) = with_cx(|cx| read(&borrowed, cx));
    let package: Package = package.unwrap();
    assert!(diag.contains(&codes::DELIVERY_NAME_UNSAFE), "{diag:?}");
    assert!(diag.contains(&codes::DELIVERY_FILE_INVALID), "{diag:?}");
    assert!(!diag.contains(&codes::XML_DTD_DROPPED), "{diag:?}");
    // 名前の順。使えない名前は除く
    let names: Vec<_> = package.files.iter().map(|f| f.name.as_str()).collect();
    assert_eq!(
        names,
        [
            "BROKEN.XML",
            "DRAWING/D0PL001Z.P21",
            "DRAWING/DRAWING.XML",
            "INDEX_C.XML",
            "OTHERS/ORG001/NOTE.XML"
        ]
    );
    // 2 段より深い XML は管理ファイルにしない。読めない管理ファイルは除く
    let managed: Vec<_> = package.managed.iter().map(|m| m.name.as_str()).collect();
    assert_eq!(managed, ["DRAWING/DRAWING.XML", "INDEX_C.XML"]);
    let index = &package.managed[1];
    assert_eq!(index.doctype.as_deref(), Some("INDE_C05.DTD"));
    assert_eq!(index.root.name, "constdata");
    let standard = find(&index.root, "適用要領基準");
    assert_eq!(standard[0].value.as_deref(), Some("土木 202603-01"));
    assert_eq!(standard[0].file, None);
    // 数の値はファイルの名前にしない
    assert_eq!(find(&index.root, "版")[0].file, None);
    // Shift_JIS の管理ファイルの名前は、管理ファイルのフォルダーから解く
    let drawing = &package.managed[0];
    assert_eq!(drawing.doctype.as_deref(), Some("DRAW05.DTD"));
    let links: Vec<_> = find(&drawing.root, "図面ファイル名")
        .iter()
        .map(|r| r.file.clone().map(|l| (l.name, l.found)))
        .collect();
    assert_eq!(
        links,
        [
            Some(("DRAWING/D0PL001Z.P21".to_string(), true)),
            Some(("DRAWING/D0PL002Z.P21".to_string(), false)),
        ]
    );
}

#[test]
fn a_zip_package_reads_the_same() {
    let drawing = drawing();
    let inputs = files(&drawing);
    let borrowed: Vec<(&str, &[u8])> = inputs.iter().map(|(n, b)| (*n, b.as_slice())).collect();
    let (expected, _) = with_cx(|cx| read(&borrowed, cx));
    let zip = zumen_zip::write(&borrowed).unwrap();
    let (found, diag) = with_cx(|cx| read_zip(&zip, cx));
    assert_eq!(found.unwrap(), expected.unwrap());
    assert!(diag.contains(&codes::DELIVERY_FILE_INVALID), "{diag:?}");
}
