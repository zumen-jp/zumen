//! 工事写真（JPEG）の Exif と XMP の読みを Pillow 13.0.0 と照らす（#505）。
//!
//! テストの中で作る JPEG と公開サンプルを、別のプロセスで動く `xtask/verify/photo/read.py` に渡す。Exif は
//! Pillow が読んだタグの値、XMP は Pillow が APP1 から取り出したパケットを Python の Expat で読んだ値と照らす。
//! 外部ツールは同梱しない。

#![cfg(test)]

use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};

use zumen_base::{Budget, CancelToken, Code, Cx, Diagnostics, codes};
use zumen_photo::{Photo, read};

mod common;
use common::{V, segment, tiff};

// コアの決定性とは別に、テストのツールの起動条件を読む。
#[allow(clippy::disallowed_methods, clippy::print_stderr)]
fn oracle() -> Option<(PathBuf, PathBuf)> {
    let explicit = std::env::var_os("ZUMEN_PHOTO_PYTHON");
    let python = explicit
        .clone()
        .map(PathBuf::from)
        .unwrap_or_else(|| "python".into());
    let script = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../xtask/verify/photo/read.py");
    let result = Command::new(&python)
        .arg("-I")
        .arg(&script)
        .arg("--check")
        .output();
    let missing = match &result {
        Ok(output) if output.status.success() => {
            assert_eq!(
                String::from_utf8_lossy(&output.stdout).trim(),
                "pillow\t13.0.0"
            );
            return Some((python, script));
        }
        Ok(output) => output.status.code() == Some(77),
        Err(error) => error.kind() == std::io::ErrorKind::NotFound,
    };
    assert!(missing, "Pillow の起動に失敗: {result:?}");
    assert!(
        explicit.is_none() && std::env::var_os("ZUMEN_PHOTO_REQUIRED").is_none(),
        "Pillow が必要だが、入っていない"
    );
    eprintln!("Pillow がないため外部の照合を飛ばす（xtask/verify/photo/README.md）");
    None
}

#[allow(clippy::disallowed_methods)]
fn external(oracle: &(PathBuf, PathBuf), bytes: &[u8]) -> String {
    let mut child = Command::new(&oracle.0)
        .arg("-I")
        .arg("-X")
        .arg("utf8")
        .arg(&oracle.1)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(bytes).unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "Pillow が読めなかった: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap()
}

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

/// `-` は値がないこと
fn optional(value: &str) -> Option<&str> {
    (value != "-").then_some(value)
}

fn real(value: &str) -> Option<f64> {
    optional(value).map(|v| v.parse().unwrap())
}

fn close(core: Option<f64>, pillow: Option<f64>, what: &str) {
    match (core, pillow) {
        (None, None) => {}
        (Some(a), Some(b)) => assert!(
            (a - b).abs() <= 1e-12 * b.abs().max(1.0),
            "{what}: {a} と {b}"
        ),
        _ => panic!("{what}: {core:?} と {pillow:?}"),
    }
}

fn unescape(value: &str) -> String {
    let mut out = String::new();
    let mut chars = value.chars();
    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.next() {
                Some('n') => out.push('\n'),
                Some('t') => out.push('\t'),
                Some(other) => out.push(other),
                None => {}
            }
        } else {
            out.push(c);
        }
    }
    out
}

/// コアの読みと Pillow の読みを照らし、照らした XMP の性質の数を返す
fn compare(name: &str, photo: &Photo, pillow: &str) -> usize {
    let mut xmp = Vec::new();
    let mut rows = 0;
    for line in pillow.lines() {
        let fields: Vec<&str> = line.split('\t').collect();
        let [key, values @ ..] = fields.as_slice() else {
            panic!("{name}: 空の行");
        };
        if *key != "xmp" {
            rows += 1;
        }
        match (*key, values) {
            ("size", [width, height]) => assert_eq!(
                (photo.width, photo.height),
                (Some(width.parse().unwrap()), Some(height.parse().unwrap())),
                "{name}: 画素数"
            ),
            ("orientation", [value]) => assert_eq!(
                photo.orientation,
                optional(value).map(|v| v.parse().unwrap()),
                "{name}: 向き"
            ),
            ("taken", [value]) => {
                assert_eq!(photo.taken.as_deref(), optional(value), "{name}: 日時");
            }
            ("make", [value]) => {
                assert_eq!(photo.make.as_deref(), optional(value), "{name}: 作り手");
            }
            ("model", [value]) => {
                assert_eq!(photo.model.as_deref(), optional(value), "{name}: 型");
            }
            ("position", [latitude, longitude]) => {
                close(photo.position.map(|p| p.latitude), real(latitude), name);
                close(photo.position.map(|p| p.longitude), real(longitude), name);
            }
            ("altitude", [value]) => close(photo.altitude, real(value), name),
            ("direction", [value]) => close(photo.direction, real(value), name),
            ("direction-reference", [value]) => assert_eq!(
                photo.direction_reference.as_deref(),
                optional(value),
                "{name}: 向きの基準"
            ),
            ("datum", [value]) => {
                assert_eq!(photo.datum.as_deref(), optional(value), "{name}: 測地系");
            }
            ("xmp", [namespace, local, value]) => {
                xmp.push((namespace.to_string(), local.to_string(), unescape(value)));
            }
            _ => panic!("{name}: 知らない行 {line:?}"),
        }
    }
    assert_eq!(rows, 10, "{name}: Pillow の行が足りない");
    let core: Vec<(String, String, String)> = photo
        .xmp
        .iter()
        .map(|p| (p.namespace.clone(), p.name.clone(), p.value.clone()))
        .collect();
    assert_eq!(core, xmp, "{name}: XMP");
    xmp.len()
}

const XMP_HEAD: &str = "http://ns.adobe.com/xap/1.0/\0";

/// 配列・入れ子・空白を持つ XMP
const XMP: &str = r#"<?xpacket begin="" id="W5M0MpCehiHzreSzNTczkc9d"?>
<x:xmpmeta xmlns:x="adobe:ns:meta/"><rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#">
<rdf:Description rdf:about="" xmlns:xmp="http://ns.adobe.com/xap/1.0/" xmlns:dc="http://purl.org/dc/elements/1.1/"
  xmlns:photoshop="http://ns.adobe.com/photoshop/1.0/" xmp:CreatorTool=" 工事写真の台帳 " xmp:Rating="4">
 <dc:title><rdf:Alt><rdf:li xml:lang="x-default"> 基礎の配筋 </rdf:li><rdf:li xml:lang="en">rebar</rdf:li></rdf:Alt></dc:title>
 <dc:subject><rdf:Bag><rdf:li>工種A</rdf:li><rdf:li>工種B</rdf:li></rdf:Bag></dc:subject>
 <dc:creator><rdf:Seq><rdf:li><rdf:Description/></rdf:li><rdf:li>2 番目</rdf:li></rdf:Seq></dc:creator>
 <dc:rights><rdf:Alt/></dc:rights>
 <photoshop:City>
   東京都
 </photoshop:City>
 <photoshop:Category/>
</rdf:Description>
<rdf:Description rdf:about="" xmlns:zumen="urn:zumen:test" zumen:場所="北側"><zumen:測点>No.3+12.5</zumen:測点></rdf:Description>
</rdf:RDF></x:xmpmeta>
<?xpacket end="w"?>"#;

fn jpeg(ifds: &[Vec<(u16, V)>], little: bool, xmp: Option<&str>) -> Vec<u8> {
    let mut out = vec![0xFF, 0xD8];
    out.extend(segment(0xE0, b"JFIF\0\x01\x01\0\0\x01\0\x01\0\0"));
    if !ifds.is_empty() {
        let mut exif = b"Exif\0\0".to_vec();
        exif.extend(tiff(ifds, little));
        out.extend(segment(0xE1, &exif));
    }
    if let Some(xmp) = xmp {
        let mut packet = XMP_HEAD.as_bytes().to_vec();
        packet.extend(xmp.as_bytes());
        out.extend(segment(0xE1, &packet));
    }
    out.extend(segment(0xC0, &[8, 0x01, 0xE0, 0x02, 0x80, 1, 1, 0x11, 0]));
    out.extend(segment(0xDA, &[1, 1, 0, 0, 0x3F, 0]));
    out.extend([0x12, 0x34, 0xFF, 0xD9]);
    out
}

/// 名前・作る JPEG・コアが返す診断
type Case = (&'static str, Vec<u8>, Vec<Code>);

fn cases() -> Vec<Case> {
    vec![
        (
            "北東・海面より下・時差つき（II）",
            jpeg(
                &[
                    vec![
                        (0x010F, V::Ascii("zumen")),
                        (0x0110, V::Ascii("試験機 ")),
                        (0x0112, V::Short(6)),
                        (0x8769, V::Pointer(1)),
                        (0x8825, V::Pointer(2)),
                    ],
                    vec![
                        (0x9003, V::Ascii("2026:10:31 09:15:30")),
                        (0x9011, V::Ascii("+09:00")),
                    ],
                    vec![
                        (0x0001, V::Ascii("N")),
                        (0x0002, V::Rationals(vec![(35, 1), (40, 1), (3013, 100)])),
                        (0x0003, V::Ascii("E")),
                        (0x0004, V::Rationals(vec![(139, 1), (45, 1), (7, 3)])),
                        (0x0005, V::Byte(1)),
                        (0x0006, V::Rationals(vec![(125, 10)])),
                        (0x0010, V::Ascii("T")),
                        (0x0011, V::Rationals(vec![(2289, 10)])),
                        (0x0012, V::Ascii("WGS-84")),
                    ],
                ],
                true,
                Some(XMP),
            ),
            vec![],
        ),
        (
            "南西・時差なし・磁北（MM）",
            jpeg(
                &[
                    vec![
                        (0x010F, V::Ascii("ZUMEN")),
                        (0x0112, V::Short(3)),
                        (0x8825, V::Pointer(2)),
                        (0x8769, V::Pointer(1)),
                    ],
                    vec![(0x9003, V::Ascii("1999:12:31 23:59:59"))],
                    vec![
                        (0x0001, V::Ascii("S")),
                        (0x0002, V::Rationals(vec![(33, 1), (52, 1), (1, 7)])),
                        (0x0003, V::Ascii("W")),
                        (0x0004, V::Rationals(vec![(70, 1), (39, 1), (4859, 100)])),
                        (0x0005, V::Byte(0)),
                        (0x0006, V::Rationals(vec![(69620, 10)])),
                        (0x0010, V::Ascii("M")),
                        (0x0011, V::Rationals(vec![(0, 1)])),
                    ],
                ],
                false,
                None,
            ),
            vec![],
        ),
        (
            "緯度だけ・UTF-8 でない型",
            jpeg(
                &[
                    vec![(0x0110, V::Bytes(b"\xff\xfe\0")), (0x8825, V::Pointer(1))],
                    vec![
                        (0x0001, V::Ascii("N")),
                        (0x0002, V::Rationals(vec![(35, 1), (0, 1), (0, 1)])),
                    ],
                ],
                true,
                None,
            ),
            vec![codes::JPEG_EXIF_INVALID],
        ),
        ("XMP だけ", jpeg(&[], false, Some(XMP)), vec![]),
    ]
}

#[test]
fn synthetic_jpegs_read_like_pillow() {
    // Pillow がなくても、コアの読みと診断は確かめる
    let mut read_cases = Vec::new();
    for (name, src, expected) in cases() {
        let (photo, diag) = with_cx(|cx| read(&src, cx));
        assert_eq!(diag, expected, "{name}");
        read_cases.push((name, photo.unwrap(), src));
    }
    let first = &read_cases[0].1;
    assert_eq!(first.taken.as_deref(), Some("2026-10-31T09:15:30+09:00"));
    assert_eq!(first.model.as_deref(), Some("試験機"));
    assert_eq!(first.altitude, Some(-12.5));
    assert_eq!(
        first
            .xmp
            .iter()
            .map(|p| (p.name.as_str(), p.value.as_str()))
            .collect::<Vec<_>>(),
        [
            ("CreatorTool", " 工事写真の台帳 "),
            ("Rating", "4"),
            ("title", "基礎の配筋"),
            ("subject", "工種A"),
            ("creator", "2 番目"),
            ("City", "東京都"),
            ("Category", ""),
            ("場所", "北側"),
            ("測点", "No.3+12.5"),
        ]
    );
    let third = &read_cases[2].1;
    assert_eq!((third.position, third.model.as_deref()), (None, None));

    let Some(oracle) = oracle() else {
        return;
    };
    for (name, photo, src) in &read_cases {
        compare(name, photo, &external(&oracle, src));
    }
}

const SAMPLE: &[u8] =
    include_bytes!("../../../samples/jpeg/commons-aichi-ceramic-museum-2018-005.jpg");

#[test]
fn public_sample_reads_like_pillow() {
    let (photo, diag) = with_cx(|cx| read(SAMPLE, cx));
    assert!(diag.is_empty(), "{diag:?}");
    let photo = photo.unwrap();
    assert_eq!((photo.width, photo.height), (Some(2028), Some(2027)));
    assert_eq!(photo.orientation, None);
    assert_eq!(photo.taken.as_deref(), Some("2018-11-10T12:43:48"));
    assert_eq!(
        (photo.make.as_deref(), photo.model.as_deref()),
        (Some("Apple"), Some("iPhone 6s"))
    );
    let position = photo.position.unwrap();
    assert!((position.latitude - 35.185_436_111_111_11).abs() < 1e-12);
    assert!((position.longitude - 137.098_191_638_888_9).abs() < 1e-12);
    // Wikimedia Commons の説明のページの向き（97.04 度）と同じ
    assert!((photo.direction.unwrap() - 97.040_271_301_398_9).abs() < 1e-12);
    assert_eq!(photo.direction_reference.as_deref(), Some("T"));
    assert_eq!(photo.datum, None);
    // GIMP の属性 10 と、入れ子の履歴（属性だけの rdf:li）の空の値。空の配列は返さない
    assert_eq!(photo.xmp.len(), 11);
    assert_eq!(
        photo
            .xmp
            .last()
            .map(|p| (p.name.as_str(), p.value.as_str())),
        Some(("History", ""))
    );

    let Some(oracle) = oracle() else {
        return;
    };
    assert_eq!(
        compare("公開サンプル", &photo, &external(&oracle, SAMPLE)),
        11
    );
}
