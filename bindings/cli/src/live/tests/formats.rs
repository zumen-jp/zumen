//! JWW・SXF の文書を常駐 IPC で開き、用紙の座標の表示で選んで動かす。

use std::collections::BTreeSet;

use zumen_drawing::Affine;
use zumen_live::RowId;

use super::super::document::{Document, RowEdit, conjugate};
use super::super::host::Host;
use super::super::wire::{self, Command, Status, Words};
use super::{call, rectangle, transform};

const JWW: &[u8] = include_bytes!("../../../../../samples/jww/ezjww-geometry.jww");
const SFC: &[u8] = include_bytes!("../../../../../samples/sxf/ezsxf-writer-all-features.sfc");
const EVERYWHERE: [f64; 4] = [-1e300, -1e300, 1e300, 1e300];
/// 用紙の外へ遠く離す移動。動かした行だけが、移動先の矩形に掛かる
const FAR: f64 = 1.0e6;

fn open_bytes(host: &mut Host, bytes: &[u8]) {
    let mut data = 1_u32.to_le_bytes().to_vec();
    data.extend_from_slice(&0_u32.to_le_bytes());
    data.extend_from_slice(bytes);
    let reply = call(host, Command::Open, 1, wire::NONE, wire::NONE, 1, &data);
    assert_eq!(reply.status, Status::Ok);
    assert_eq!(reply.display_status, Status::Ok);
    assert_eq!(reply.revision, 0);
}

fn select(host: &mut Host, id: u64, revision: u64, rect: [f64; 4]) -> Vec<u32> {
    let reply = call(
        host,
        Command::Select,
        id,
        revision,
        wire::NONE,
        0,
        &rectangle(rect),
    );
    assert_eq!(reply.status, Status::Ok);
    let mut words = Words(&reply.data);
    let count = words.u32().unwrap();
    let rows: Vec<u32> = (0..count).map(|_| words.u32().unwrap()).collect();
    assert!(words.0.is_empty());
    rows
}

/// 表示の座標で選んだ行を遠くへ動かすと、その行だけが移動先で選ばれ、元に戻すと元の場所に戻る
fn moves_a_selected_row_in_display_coordinates(bytes: &[u8]) {
    let mut host = Host::default();
    open_bytes(&mut host, bytes);
    let rows = select(&mut host, 2, 0, EVERYWHERE);
    assert!(!rows.is_empty());
    // 形式に書き戻せない要素（画層のない SFC の要素など）は断られる。断られても版は進まない
    let row = rows
        .iter()
        .copied()
        .find(|&row| {
            let moved = call(
                &mut host,
                Command::Transform,
                3,
                0,
                0,
                1,
                &transform(&[row], Affine::translation(FAR, FAR)),
            );
            assert!(matches!(moved.status, Status::Ok | Status::Rejected));
            assert_eq!(moved.revision, u64::from(moved.status == Status::Ok));
            moved.status == Status::Ok
        })
        .unwrap();
    let far = [FAR * 0.5, FAR * 0.5, FAR * 2.0, FAR * 2.0];
    assert_eq!(select(&mut host, 4, 1, far), [row]);
    let undone = call(&mut host, Command::Undo, 5, 1, wire::NONE, 1, &[]);
    assert_eq!(undone.status, Status::Ok);
    assert!(select(&mut host, 6, 2, far).is_empty());
    let mut again = select(&mut host, 7, 2, EVERYWHERE);
    again.sort_unstable();
    let mut rows = rows;
    rows.sort_unstable();
    assert_eq!(again, rows);
}

#[test]
fn jww_rows_move_in_paper_coordinates() {
    moves_a_selected_row_in_display_coordinates(JWW);
}

#[test]
fn sfc_rows_move_in_paper_coordinates() {
    moves_a_selected_row_in_display_coordinates(SFC);
}

/// ビューに置いていない実寸の空間の要素は、表示の座標が決まらないので、動かさずに断る
#[test]
fn sfc_rows_without_a_view_are_rejected_without_changes() {
    let mut host = Host::default();
    open_bytes(&mut host, SFC);
    let shown: BTreeSet<u32> = select(&mut host, 2, 0, EVERYWHERE).into_iter().collect();
    let before = call(&mut host, Command::Save, 3, 0, wire::NONE, 0, &[]).data;
    let mut rejected = 0;
    for row in 0..4096_u32 {
        let target = call(
            &mut host,
            Command::Target,
            4,
            0,
            wire::NONE,
            0,
            &row.to_le_bytes(),
        );
        if target.status != Status::Ok || shown.contains(&row) {
            continue;
        }
        let reply = call(
            &mut host,
            Command::Transform,
            5,
            0,
            wire::NONE,
            1,
            &transform(&[row], Affine::translation(1.0, 2.0)),
        );
        assert_eq!(reply.status, Status::Rejected, "row {row}");
        assert_eq!(reply.revision, 0);
        assert!(reply.diagnostics > 0);
        rejected += 1;
    }
    assert!(rejected > 0);
    assert_eq!(
        call(&mut host, Command::Save, 6, 0, wire::NONE, 0, &[]).data,
        before
    );
}

/// 表示の座標の変換を行の座標に直すと、行の座標で当ててから写したものと、写してから当てたものが一致する
#[test]
fn conjugated_transforms_commute_with_the_sheet() {
    let sheet = Affine {
        m: [0.01, 0.0, 10.0, 0.0, 0.01, 20.0],
    };
    let display = Affine {
        m: [0.0, -1.0, 3.0, 1.0, 0.0, -4.0],
    };
    let row = conjugate(sheet, display).unwrap();
    for (x, y) in [(0.0, 0.0), (100.0, -250.0), (-7.5, 3.25)] {
        let (sx, sy) = sheet.apply(x, y);
        let expected = display.apply(sx, sy);
        let (rx, ry) = row.apply(x, y);
        let actual = sheet.apply(rx, ry);
        assert!(
            (actual.0 - expected.0).abs() < 1e-9,
            "{actual:?} {expected:?}"
        );
        assert!(
            (actual.1 - expected.1).abs() < 1e-9,
            "{actual:?} {expected:?}"
        );
    }
    let translated = conjugate(sheet, Affine::translation(1.0, 2.0)).unwrap();
    for (actual, expected) in translated
        .m
        .into_iter()
        .zip([1.0, 0.0, 100.0, 0.0, 1.0, 200.0])
    {
        assert!((actual - expected).abs() < 1e-9, "{:?}", translated.m);
    }
    assert!(conjugate(Affine { m: [0.0; 6] }, display).is_none());
}

/// 画層の表の項目（値・フラグ・RGB・色の符号・名前）
fn layers(host: &mut Host, id: u64, revision: u64) -> Vec<(u32, u32, u32, u32, String)> {
    let reply = call(host, Command::Layers, id, revision, wire::NONE, 0, &[]);
    assert_eq!(reply.status, Status::Ok);
    assert_eq!(reply.data_kind, 6);
    let mut words = Words(&reply.data);
    let count = words.u32().unwrap();
    assert_eq!(u64::from(count), reply.result);
    let mut out = Vec::new();
    for _ in 0..count {
        let [value, flags, rgb, code, length] = [(); 5].map(|()| words.u32().unwrap());
        let length = length as usize;
        let padded = length.next_multiple_of(4);
        let name = String::from_utf8(words.0[..length].to_vec()).unwrap();
        assert!(words.0[length..padded].iter().all(|&byte| byte == 0));
        words.0 = &words.0[padded..];
        out.push((value, flags, rgb, code, name));
    }
    assert!(words.0.is_empty());
    out
}

/// 表示のスタイルの画層の値と、その画層
type LayerPairs = Vec<(u32, zumen_drawing::Layer)>;

/// 画層の表は、文書エンジンの画層の組を写し、名前・表示の可否・色を運ぶ
#[test]
fn layers_carry_the_display_layer_table() {
    let mut budget = zumen_base::Budget::default();
    let mut diag = zumen_base::Diagnostics::new();
    let cancel = zumen_base::CancelToken::new();
    let cx = &mut zumen_base::Cx::new(&mut budget, &mut diag, &cancel);
    let engines: [(&[u8], LayerPairs); 3] = [
        (
            super::DXF,
            zumen_acad::AcadLive::open(super::DXF, cx)
                .unwrap()
                .display_layers(cx)
                .unwrap(),
        ),
        (
            JWW,
            zumen_jww::JwwLive::open(JWW, cx)
                .unwrap()
                .display_layers(cx)
                .unwrap(),
        ),
        (
            SFC,
            zumen_sxf::SxfLive::open(SFC, cx)
                .unwrap()
                .display_layers(cx)
                .unwrap(),
        ),
    ];
    for (bytes, expected) in engines {
        let mut host = Host::default();
        open_bytes(&mut host, bytes);
        let actual = layers(&mut host, 2, 0);
        assert_eq!(actual.len(), expected.len());
        assert!(!actual.is_empty());
        for ((value, flags, rgb, code, name), (expected_value, layer)) in
            actual.iter().zip(&expected)
        {
            assert_eq!(value, expected_value);
            assert_eq!(name, &layer.name);
            assert_eq!(flags & 1 != 0, layer.state.is_some());
            if let Some(state) = layer.state {
                assert_eq!(flags & 2 != 0, state.visible);
                assert_eq!(flags & 16 != 0, state.frozen);
            }
            let color = layer.style.and_then(|style| style.color);
            assert_eq!(flags & 32 != 0, color.and_then(|color| color.rgb).is_some());
            assert_eq!(
                flags & 64 != 0,
                color.and_then(|color| color.code).is_some()
            );
            if let Some(found) = color.and_then(|color| color.code) {
                assert_eq!(code & 0x00FF_FFFF, found.value);
            }
            if let Some([r, g, b]) = color.and_then(|color| color.rgb) {
                assert_eq!(
                    *rgb,
                    (u32::from(r) << 16) | (u32::from(g) << 8) | u32::from(b)
                );
            }
        }
        // 前提の版が違えば、表を返さない
        let stale = call(&mut host, Command::Layers, 3, 9, wire::NONE, 0, &[]);
        assert_eq!(stale.status, Status::Stale);
        assert!(stale.data.is_empty());
    }
}

/// 描く順は、文書エンジンの `display_order` の行を、件数と行番号で運ぶ
#[test]
fn order_carries_the_draw_order() {
    let mut budget = zumen_base::Budget::default();
    let mut diag = zumen_base::Diagnostics::new();
    let cancel = zumen_base::CancelToken::new();
    let cx = &mut zumen_base::Cx::new(&mut budget, &mut diag, &cancel);
    let engines: [(&[u8], Vec<RowId>); 3] = [
        (
            super::DXF,
            zumen_acad::AcadLive::open(super::DXF, cx)
                .unwrap()
                .display_order(cx)
                .unwrap(),
        ),
        (
            JWW,
            zumen_jww::JwwLive::open(JWW, cx)
                .unwrap()
                .display_order(cx)
                .unwrap(),
        ),
        (
            SFC,
            zumen_sxf::SxfLive::open(SFC, cx)
                .unwrap()
                .display_order(cx)
                .unwrap(),
        ),
    ];
    for (bytes, expected) in engines {
        let mut host = Host::default();
        open_bytes(&mut host, bytes);
        let reply = call(&mut host, Command::Order, 2, 0, wire::NONE, 0, &[]);
        assert_eq!(reply.status, Status::Ok);
        assert_eq!(reply.data_kind, 7);
        let mut words = Words(&reply.data);
        let count = words.u32().unwrap();
        assert_eq!(u64::from(count), reply.result);
        let rows: Vec<u32> = (0..count).map(|_| words.u32().unwrap()).collect();
        assert!(words.0.is_empty());
        assert!(!rows.is_empty());
        assert_eq!(rows, expected.iter().map(|row| row.0).collect::<Vec<_>>());
        // 前提の版が違えば、順を返さない
        let stale = call(&mut host, Command::Order, 3, 9, wire::NONE, 0, &[]);
        assert_eq!(stale.status, Status::Stale);
        assert!(stale.data.is_empty());
    }
}

fn rows_body(rows: &[u32]) -> Vec<u8> {
    let mut bytes = (rows.len() as u32).to_le_bytes().to_vec();
    bytes.extend_from_slice(&0_u32.to_le_bytes());
    for row in rows {
        bytes.extend_from_slice(&row.to_le_bytes());
    }
    bytes
}

/// 選んだ行を除くと選ばれず・対象を引けず、取り消すと元の行に戻る。形式に書き戻せない行は断られ、版は進まない
#[test]
fn remove_drops_selected_rows_and_undo_restores_them() {
    for bytes in [super::DXF, JWW, SFC] {
        let mut host = Host::default();
        open_bytes(&mut host, bytes);
        let rows = select(&mut host, 2, 0, EVERYWHERE);
        let row = rows
            .iter()
            .copied()
            .find(|&row| {
                let removed = call(&mut host, Command::Remove, 3, 0, 0, 1, &rows_body(&[row]));
                assert!(matches!(removed.status, Status::Ok | Status::Rejected));
                assert_eq!(removed.revision, u64::from(removed.status == Status::Ok));
                if removed.status == Status::Ok {
                    assert_eq!(removed.result, 1);
                    // 表示の確認を付けた要求には、表示の差分を返す
                    assert_eq!(removed.data_kind, 1);
                }
                removed.status == Status::Ok
            })
            .unwrap();
        let left = select(&mut host, 4, 1, EVERYWHERE);
        assert!(!left.contains(&row));
        assert_eq!(left.len() + 1, rows.len());
        let target = call(
            &mut host,
            Command::Target,
            5,
            1,
            wire::NONE,
            0,
            &row.to_le_bytes(),
        );
        assert_eq!(target.status, Status::Missing);
        // 除いた行は、もう一度は除けない
        let again = call(
            &mut host,
            Command::Remove,
            6,
            1,
            wire::NONE,
            0,
            &rows_body(&[row]),
        );
        assert_eq!(again.status, Status::Missing);
        assert_eq!(again.revision, 1);
        let undone = call(&mut host, Command::Undo, 7, 1, wire::NONE, 0, &[]);
        assert_eq!(undone.status, Status::Ok);
        let mut back = select(&mut host, 8, 2, EVERYWHERE);
        back.sort_unstable();
        let mut rows = rows;
        rows.sort_unstable();
        assert_eq!(back, rows);
    }
}

/// 写しの行は、当てる前の表の末尾から対象の順に並び、`Applied` の ID と一致する
#[test]
fn copies_are_appended_in_target_order() {
    let mut budget = zumen_base::Budget::default();
    let mut diag = zumen_base::Diagnostics::new();
    let cancel = zumen_base::CancelToken::new();
    let cx = &mut zumen_base::Cx::new(&mut budget, &mut diag, &cancel);
    let mut document = Document::open(super::DXF, 1, cx).unwrap();
    let targets = [RowId(1), RowId(0)];
    assert_copies(&mut document, &targets, cx);
    // ハンドルを持つ公開サンプルの行を 1 つずつ写す（ハンドルのない R12 の DXF は写せない）
    let mut copied = 0;
    for bytes in [
        &include_bytes!("../../../../../samples/dwg/zumen-layout-r2018.dwg")[..],
        &include_bytes!("../../../../../samples/dxf/text_alignments.dxf")[..],
        &include_bytes!("../../../../../samples/dxf/ezdxf-mtext-columns-r2007.dxf")[..],
    ] {
        let mut document = Document::open(bytes, 1, cx).unwrap();
        for index in 0..document.row_count() {
            let row = RowId::from_index(index).unwrap();
            if document.target(row, cx).is_err() {
                continue;
            }
            let mut diag = zumen_base::Diagnostics::new();
            let mut budget = zumen_base::Budget::default();
            let inner = &mut zumen_base::Cx::new(&mut budget, &mut diag, &cancel);
            if assert_copies(&mut document, &[row], inner) {
                copied += 1;
            }
        }
    }
    assert!(copied > 10, "{copied}");
}

/// 写せたら、足した行が `Applied` と対象の順に一致することを確かめる。断られたら表の行は増えない
fn assert_copies(document: &mut Document, targets: &[RowId], cx: &mut zumen_base::Cx<'_>) -> bool {
    let before = document.row_count();
    let Ok(applied) = document.edit(
        document.revision(),
        targets,
        RowEdit::Copy(Affine::translation(FAR, FAR)),
        cx,
    ) else {
        assert_eq!(document.row_count(), before);
        return false;
    };
    assert_eq!(document.row_count(), before + targets.len());
    assert_eq!(applied.len(), targets.len());
    for (index, applied) in applied.iter().enumerate() {
        let row = RowId::from_index(before + index).unwrap();
        let seen = document.target(row, cx).unwrap();
        assert_eq!(applied.id.as_deref(), Some(seen.id.as_str()));
        assert_eq!(applied.hash, Some(seen.hash));
    }
    true
}

/// Copy は写しの行を返し、写しは移動先で選ばれる。元の行は残る。写せない形式は断られ、版は進まない
#[test]
fn copy_carries_the_new_rows() {
    let mut host = Host::default();
    open_bytes(&mut host, super::DXF);
    let reply = call(
        &mut host,
        Command::Copy,
        2,
        0,
        wire::NONE,
        0,
        &transform(&[0, 1], Affine::translation(FAR, FAR)),
    );
    assert_eq!(reply.status, Status::Ok);
    assert_eq!(reply.revision, 1);
    assert_eq!(reply.data_kind, 5);
    assert_eq!(reply.result, 2);
    let mut words = Words(&reply.data);
    assert_eq!(words.u32(), Some(2));
    let copies = [words.u32().unwrap(), words.u32().unwrap()];
    assert!(words.0.is_empty());
    assert_eq!(copies, [2, 3]);
    let far = [FAR * 0.5, FAR * 0.5, FAR * 2.0, FAR * 2.0];
    assert_eq!(select(&mut host, 3, 1, far), copies);
    assert_eq!(select(&mut host, 4, 1, EVERYWHERE), [0, 1, 2, 3]);
    let undone = call(&mut host, Command::Undo, 5, 1, wire::NONE, 0, &[]);
    assert_eq!(undone.status, Status::Ok);
    assert_eq!(select(&mut host, 6, 2, EVERYWHERE), [0, 1]);
    // JWW・SXF の文書エンジンは、まだ写せない
    for bytes in [JWW, SFC] {
        let mut host = Host::default();
        open_bytes(&mut host, bytes);
        let rows = select(&mut host, 2, 0, EVERYWHERE);
        let reply = call(
            &mut host,
            Command::Copy,
            3,
            0,
            wire::NONE,
            0,
            &transform(&rows[..1], Affine::translation(FAR, FAR)),
        );
        assert_eq!(reply.status, Status::Rejected);
        assert_eq!(reply.revision, 0);
        assert!(reply.data.is_empty());
    }
}
