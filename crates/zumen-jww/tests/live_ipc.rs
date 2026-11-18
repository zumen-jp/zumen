//! 常駐 IPC の文字の読み取りと置き換え（`drawing/live/ipc.rs`、#878）。
//! JWW の公開サンプルには文字がないので、`tests/common` で文字を持つ JWW を組み立てて確かめる。

#![cfg(feature = "drawing")]

mod common;

use core::f64::consts::PI;

use common::{Archive, Spec};
use zumen_base::{Budget, CancelToken, Cx, Diagnostics};
use zumen_drawing::{Changes, Edit};
use zumen_jww::JwwLive;

/// 線・円・円弧・点・文字「図面」と、ブロック「部品」を持つ JWW（`tests/live.rs` と同じ組み立て）
fn base_src() -> Vec<u8> {
    let mut spec = Spec::new(700);
    let mut layers = [0; 16];
    layers[0] = 3;
    layers[1] = 2;
    spec.groups[0] = (3, 100.0, layers);
    spec.max_draw_width = -300;
    spec.pens[2] = 0x0000_00FF;
    spec.printer_widths[2] = 25;
    let mut archive = Archive::new(700);
    archive.layer = 0;
    archive.group = 0;
    archive.pen_width = 0;
    archive.count(5);
    archive.line();
    archive.tag("CDataEnko");
    archive.base(1, 1);
    archive.reals(&[1.0, 2.0, 5.0, 0.0, 2.0 * PI, 0.0, 1.0]);
    archive.out.extend(1u32.to_le_bytes());
    archive.tag("CDataEnko");
    archive.base(1, 1);
    archive.reals(&[0.0, 0.0, 5.0, 0.0, PI / 2.0, 0.0, 1.0]);
    archive.out.extend(0u32.to_le_bytes());
    archive.tag("CDataTen");
    archive.base(1, 2);
    archive.reals(&[1.0, 1.0]);
    archive.out.extend(0u32.to_le_bytes());
    archive.tag("CDataMoji");
    archive.base(1, 1);
    archive.text_body("図面");
    archive.count(1);
    archive.block("部品");
    archive.out.extend(0u32.to_le_bytes());
    let mut src = spec.build();
    src.extend(&archive.out);
    src
}

/// 文字の行の内容だけを替え、保存して開き直しても同じ文字で、取り消すと元のバイト列に戻る
#[test]
fn display_retexted_replaces_the_content_of_text_rows() {
    let mut budget = Budget::default();
    let mut diag = Diagnostics::new();
    let cancel = CancelToken::new();
    let cx = &mut Cx::new(&mut budget, &mut diag, &cancel);
    let src = base_src();
    let mut live = JwwLive::open(&src, cx).unwrap();
    let mut changed = 0;
    for row in live.display_order(cx).unwrap() {
        let (kind, content) = live.display_text(row, cx).unwrap().unwrap();
        let Some(element) = live.display_retexted(row, "改め α", cx).unwrap() else {
            assert_eq!(kind, 0);
            continue;
        };
        assert_eq!((kind, content.as_str()), (1, "図面"));
        let target = live.seen_of_row(row, cx).unwrap().unwrap();
        let changes = Changes {
            document: None,
            edits: vec![Edit::Replace {
                target,
                element: Box::new(element),
            }],
        };
        live.apply(&changes, cx).unwrap();
        assert_eq!(
            live.display_text(row, cx).unwrap().unwrap(),
            (1, "改め α".to_owned())
        );
        changed += 1;
    }
    assert_eq!(changed, 1);
    let saved = live.save(cx).unwrap();
    let reopened = JwwLive::open(&saved, cx).unwrap();
    let texts = |live: &JwwLive, cx: &mut Cx<'_>| -> Vec<(u32, String)> {
        live.display_order(cx)
            .unwrap()
            .into_iter()
            .map(|row| live.display_text(row, cx).unwrap().unwrap())
            .collect()
    };
    assert_eq!(texts(&live, cx), texts(&reopened, cx));
    assert!(live.undo(cx).unwrap());
    assert_eq!(live.save(cx).unwrap(), src);
}
