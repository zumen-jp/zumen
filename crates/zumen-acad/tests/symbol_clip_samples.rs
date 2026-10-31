//! 試料の DXF・DWG で、記号の切り抜き（XCLIP）を移す（決定 0149）。
//! 試料は公開サンプルの `samples/` と、受け取った図面を置く `ZUMEN_SAMPLES` のフォルダーから集める。
#![cfg(feature = "drawing")]

mod support;

use std::path::{Path, PathBuf};
use zumen_acad::{Dwg, Dxf};
use zumen_base::{Budget, CancelToken, Cx, Diagnostics, codes};
use zumen_drawing::{Clip, Drawing2D, Shape};
use zumen_sdk::{Codec, Lift};

/// フォルダーの中の、ASCII の DXF と DWG
#[cfg(test)]
fn drawings() -> Vec<PathBuf> {
    let mut found: Vec<_> = support::sample_files()
        .into_iter()
        .filter(|path| path.extension().is_some_and(|e| e == "dxf" || e == "dwg"))
        .filter(|path| !path.to_string_lossy().ends_with("_binary.dxf"))
        .collect();
    found.sort();
    found
}

#[cfg(test)]
fn lift(path: &Path) -> Option<(Drawing2D, Diagnostics)> {
    let src = std::fs::read(path).unwrap();
    let mut diag = Diagnostics::new();
    let mut budget = Budget::default();
    let cancel = CancelToken::new();
    let mut cx = Cx::new(&mut budget, &mut diag, &cancel);
    let drawing = if path.extension().is_some_and(|e| e == "dwg") {
        Dwg::read(&src, &mut cx).ok()?.lift(&mut cx).ok()?
    } else {
        Dxf::read(&src, &mut cx).ok()?.lift(&mut cx).ok()?
    };
    Some((drawing, diag))
}

/// 図面の中の、切り抜いた記号の名前と切り抜き
fn clips(drawing: &Drawing2D) -> Vec<(String, Clip)> {
    let spaces = drawing
        .spaces
        .iter()
        .flat_map(|space| &space.elements)
        .chain(
            drawing
                .symbols
                .iter()
                .flat_map(|symbol| &symbol.space.elements),
        );
    spaces
        .filter_map(|element| match &element.shape {
            Shape::Symbol(symbol) => symbol
                .clip
                .clone()
                .map(|clip| (symbol.key.name.clone(), clip)),
            _ => None,
        })
        .collect()
}

/// 試料の `sample_AC10*` の図面（R12 の AC1009 を除く）は、`my_block` の配置を 1 つ、長方形で切り抜く（XCLIP）。
/// どの版の DWG・DXF でも、境界の 2 つの角は記号の定義の空間で同じ点になり、定義の中身（X が 0〜163）の
/// 右側に重なる。拡張辞書の `ACAD_FILTER` は、移さなかった項目として診断しない
#[test]
fn block_clips_become_the_clip_of_the_symbol() {
    let mut checked = 0;
    for path in drawings() {
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        if !name.starts_with("sample_AC10") || name.starts_with("sample_AC1009") {
            continue;
        }
        let Some((drawing, diag)) = lift(&path) else {
            continue;
        };
        let found = clips(&drawing);
        let [(block, clip)] = found.as_slice() else {
            panic!("{name} {found:?}");
        };
        assert_eq!(block, "my_block", "{name}");
        assert!(!clip.inverted, "{name}");
        let expected = [
            [91.336673034354, 28.465595538105],
            [161.882362107315, 112.309658290608],
        ];
        assert_eq!(clip.vertices.len(), 2, "{name}");
        for (vertex, expected) in clip.vertices.iter().zip(expected) {
            for (a, b) in vertex.iter().zip(expected) {
                assert!((a - b).abs() < 1e-9, "{name} {vertex:?}");
            }
        }
        let codes: Vec<_> = diag.iter().map(|d| d.code()).collect();
        assert!(!codes.contains(&codes::DXF_CLIP_DROPPED), "{name}");
        let filter = diag.iter().any(|d| {
            d.code() == codes::DXF_EXTENSION_DICTIONARY_DROPPED
                && format!("{:?}", d.params()).contains("ACAD_FILTER")
        });
        assert!(!filter, "{name}");
        checked += 1;
    }
    // ZUMEN_SAMPLES がなければ 0
    assert!(checked == 0 || checked >= 11, "{checked}");
}

/// AutoCAD で作った `zumen-xclip-*` の図面（xtask/autocad/samples.lsp）は、`CLIPPED` を 3 つ置いて切り抜く。
/// 1 つ目は 30° 回して 2 倍にした配置を長方形で、2 つ目は 2 倍にした配置を多角形で、3 つ目は 2 倍にした配置を
/// 反転した長方形で切り抜く（決定 0208）。境界は、どれも挿入の逆の変換で記号の定義の空間に写る
#[test]
fn autocad_clips_including_an_inverted_one_become_the_clip_of_the_symbol() {
    // 30° の余弦と正弦
    let (cos, sin) = (0.866_025_403_784_438_6, 0.5);
    let turned = |[x, y]: [f64; 2]| [(x * cos + y * sin) / 2.0, (y * cos - x * sin) / 2.0];
    let expected = [
        (
            [[10.0, 10.0], [50.0, 10.0], [50.0, 40.0], [10.0, 40.0]]
                .map(turned)
                .to_vec(),
            false,
        ),
        (
            vec![[5.0, 2.5], [25.0, 2.5], [35.0, 17.5], [10.0, 17.5]],
            false,
        ),
        (vec![[10.0, 5.0], [30.0, 15.0]], true),
    ];
    let mut checked = 0;
    for path in drawings() {
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        if !name.starts_with("zumen-xclip-") {
            continue;
        }
        let (drawing, diag) = lift(&path).unwrap();
        let found = clips(&drawing);
        assert_eq!(found.len(), expected.len(), "{name} {found:?}");
        for ((block, clip), (vertices, inverted)) in found.iter().zip(&expected) {
            assert_eq!(block, "CLIPPED", "{name}");
            assert_eq!(clip.inverted, *inverted, "{name} {clip:?}");
            assert_eq!(clip.vertices.len(), vertices.len(), "{name} {clip:?}");
            for (vertex, expected) in clip.vertices.iter().zip(vertices) {
                for (a, b) in vertex.iter().zip(expected) {
                    assert!((a - b).abs() < 1e-9, "{name} {vertex:?} {expected:?}");
                }
            }
        }
        let codes: Vec<_> = diag.iter().map(|d| d.code()).collect();
        assert!(!codes.contains(&codes::DXF_CLIP_DROPPED), "{name}");
        checked += 1;
    }
    // R2013 と R2018 の DWG と DXF
    assert_eq!(checked, 4);
}

/// 多角形の写しがなければ、反転を知らないソフトが境界を変えたものとして、SPATIAL_FILTER の境界（外枠から元の
/// 境界を切り欠いた 11 点の多角形）をそのまま、内側を隠さない切り抜きにする（決定 0208）
#[test]
fn inverted_clips_without_the_copy_of_the_boundary_keep_the_boundary() {
    let path =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../samples/dwg/zumen-xclip-r2018_ascii.dxf");
    let src = std::fs::read(&path).unwrap();
    let text = String::from_utf8(src).unwrap();
    let edited = text.replace(
        "ACAD_INVERTEDCLIP_ROUNDTRIP_COMPARE",
        "ACAD_INVERTEDCLIP_ROUNDTRIP_OTHER",
    );
    assert_ne!(edited, text);
    let mut diag = Diagnostics::new();
    let mut budget = Budget::default();
    let cancel = CancelToken::new();
    let mut cx = Cx::new(&mut budget, &mut diag, &cancel);
    let drawing = Dxf::read(edited.as_bytes(), &mut cx)
        .unwrap()
        .lift(&mut cx)
        .unwrap();
    let found = clips(&drawing);
    let (_, clip) = &found[2];
    assert!(!clip.inverted, "{clip:?}");
    assert_eq!(clip.vertices.len(), 11, "{clip:?}");
}
