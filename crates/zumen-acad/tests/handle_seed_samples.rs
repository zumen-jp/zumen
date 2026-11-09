//! AutoCAD で作った DWG に、HANDSEED の大きさ（バイト）が増える数の線を足して書き戻す（#701）。
#![cfg(feature = "drawing")]

use std::path::Path;
use zumen_acad::{Dwg, DwgFile};
use zumen_base::{Budget, CancelToken, Cx, Diagnostics};
use zumen_drawing::{Changes, Drawing2D, Edit, Element, ModelPoint, Placed, Shape, id};
use zumen_sdk::{Apply, Codec, Lift};

#[cfg(test)]
fn read(src: &[u8]) -> DwgFile {
    let mut diag = Diagnostics::new();
    let mut budget = Budget::default();
    Dwg::read(
        src,
        &mut Cx::new(&mut budget, &mut diag, &CancelToken::new()),
    )
    .unwrap()
}

#[cfg(test)]
fn lift(file: &DwgFile) -> Drawing2D {
    let mut diag = Diagnostics::new();
    let mut budget = Budget::default();
    file.lift(&mut Cx::new(&mut budget, &mut diag, &CancelToken::new()))
        .unwrap()
}

/// 実寸の空間に `element` を足す変更
#[cfg(test)]
fn add(element: Element<ModelPoint>) -> Edit {
    Edit::Add {
        place: "space/0".to_owned(),
        element: Box::new(Placed::Model(element)),
    }
}

/// 実寸の空間の要素の ID（決定 0139）
#[cfg(test)]
fn ids(drawing: &Drawing2D) -> Vec<String> {
    let space = &drawing.spaces[0];
    let place = id::space(space.id);
    let mut namer = id::Namer::new(&place);
    space
        .elements
        .iter()
        .enumerate()
        .map(|(index, element)| namer.next(index, element))
        .collect()
}

/// 公開サンプルの R2018 に、HANDSEED が 0xFFFF をまたいで 3 バイトになる数の線を、1 回の apply で足す。
/// 書き戻したファイルを読み直すと、足した線と HANDSEED が合い、元の要素の ID と形は変わらない
#[test]
fn lines_crossing_two_byte_handles_are_written_back_to_autocad_dwgs() {
    // R2000・R2004 の書き直しは、合成の DWG で確かめる（tests/dwg.rs）。ここでは、#701 で断っていた数（64,353 本）を足す
    for version in ["r2018"] {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../samples/dwg")
            .join(format!("zumen-layout-{version}.dwg"));
        let src = std::fs::read(&path).unwrap();
        let mut file = read(&src);
        let seed = file.handle_seed().unwrap();
        assert!(seed < 0x1_0000, "{version} {seed:#x}");
        let before = lift(&file);
        let layer = before.spaces[0].elements[0].layer;
        let template = before.spaces[0].elements[0].style;
        // HANDSEED を 0x10000（3 バイト）に進める数
        let count = 0x1_0000 - seed;
        let line = |i: u64| Element {
            source: zumen_base::Ref::default(),
            layer,
            style: template,
            shape: Shape::Line {
                start: ModelPoint {
                    space: before.spaces[0].id,
                    x: 0.0,
                    y: i as f64,
                },
                end: ModelPoint {
                    space: before.spaces[0].id,
                    x: 10.0,
                    y: i as f64,
                },
            },
        };
        let edits = (0..count).map(|i| add(line(i))).collect();
        let mut diag = Diagnostics::new();
        let mut budget = Budget::default();
        let cancel = CancelToken::new();
        let mut cx = Cx::new(&mut budget, &mut diag, &cancel);
        let result = file.apply(
            &Changes {
                document: None,
                edits,
            },
            &mut cx,
        );
        assert!(
            result.is_ok(),
            "{version}: {:?}",
            diag.iter().map(|d| d.code()).collect::<Vec<_>>()
        );
        let mut out = Vec::new();
        Dwg::write(&file, &mut out, &mut cx).unwrap();

        let again = read(&out);
        assert_eq!(again.handle_seed(), Some(seed + count), "{version}");
        let after = lift(&again);
        let (old, new) = after.spaces[0]
            .elements
            .split_at(before.spaces[0].elements.len());
        // 図形の並びをたどるハンドルを直したレコードは、置く所（バイト範囲）だけが変わる
        let without_bytes = |elements: &[Element<ModelPoint>]| {
            elements
                .iter()
                .map(|e| {
                    let mut e = e.clone();
                    let key = e.source.key();
                    e.source = zumen_base::Ref::default();
                    (key, e)
                })
                .collect::<Vec<_>>()
        };
        assert_eq!(
            without_bytes(old),
            without_bytes(&before.spaces[0].elements),
            "{version}"
        );
        assert_eq!(ids(&after)[..old.len()], ids(&before)[..], "{version}");
        assert_eq!(new.len() as u64, count, "{version}");
        for (i, element) in new.iter().enumerate() {
            assert_eq!(element.shape, line(i as u64).shape, "{version} {i}");
        }
        assert_eq!(count, 64_353);
    }
}
