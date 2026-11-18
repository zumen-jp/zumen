//! JWW・SXF の文書を常駐 IPC で開き、用紙の座標の表示で選んで動かす。

use std::collections::BTreeSet;

use zumen_drawing::Affine;

use super::super::document::conjugate;
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
