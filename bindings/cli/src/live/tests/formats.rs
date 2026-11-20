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
        &RowEdit::Copy(Affine::translation(FAR, FAR)),
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

/// Copy は写しの行を返し、写しは移動先で選ばれる。元の行は残る。写せない要素は断られ、版は進まない
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
    for bytes in [JWW, SFC] {
        let mut host = Host::default();
        open_bytes(&mut host, bytes);
        let rows = select(&mut host, 2, 0, EVERYWHERE);
        let (source, reply) = rows
            .iter()
            .find_map(|source| {
                let reply = call(
                    &mut host,
                    Command::Copy,
                    3,
                    0,
                    wire::NONE,
                    0,
                    &transform(&[*source], Affine::translation(FAR, FAR)),
                );
                if reply.status == Status::Rejected {
                    assert_eq!(reply.revision, 0);
                    assert!(reply.data.is_empty());
                    return None;
                }
                Some((*source, reply))
            })
            .unwrap();
        assert_eq!(reply.status, Status::Ok);
        assert_eq!(reply.revision, 1);
        assert_eq!(reply.data_kind, 5);
        let mut words = Words(&reply.data);
        assert_eq!(words.u32(), Some(1));
        let copy = words.u32().unwrap();
        assert!(words.0.is_empty());
        assert_eq!(select(&mut host, 4, 1, far), [copy]);
        assert!(select(&mut host, 5, 1, EVERYWHERE).contains(&source));
        let undone = call(&mut host, Command::Undo, 6, 1, wire::NONE, 0, &[]);
        assert_eq!(undone.status, Status::Ok);
        assert_eq!(select(&mut host, 7, 2, EVERYWHERE), rows);
    }
}

/// 画層 0 と A に 1 本ずつ線を置いた R2000 の DXF。R12 の要素はスタイルを変えられない
const R2000: &[u8] = b"0
SECTION
2
HEADER
9
$ACADVER
1
AC1015
9
$INSUNITS
70
4
0
ENDSEC
    0
SECTION
2
TABLES
0
TABLE
2
LAYER
70
2
    0
LAYER
5
10
2
0
70
0
62
7
6
CONTINUOUS
    0
LAYER
5
11
2
A
70
0
62
1
6
CONTINUOUS
0
ENDTAB
0
ENDSEC
    0
SECTION
2
ENTITIES
    0
LINE
5
30
8
0
10
0
20
0
11
1
21
0
    0
LINE
5
31
8
A
10
2
20
0
11
3
21
0
    0
ENDSEC
0
EOF
";

/// Restyle の本文。`color` は（種類, フラグ, 色番号, RGB）、`weight` は（種類, 幅）
fn restyle_body(
    mask: u32,
    layer: u32,
    color: (u32, u32, u32, u32),
    weight: (u32, f64),
    scale: f64,
    rows: &[u32],
) -> Vec<u8> {
    let mut bytes = Vec::new();
    for value in [
        rows.len() as u32,
        mask,
        layer,
        color.0,
        color.1,
        color.2,
        color.3,
        weight.0,
    ] {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    bytes.extend_from_slice(&weight.1.to_bits().to_le_bytes());
    bytes.extend_from_slice(&scale.to_bits().to_le_bytes());
    for row in rows {
        bytes.extend_from_slice(&row.to_le_bytes());
    }
    bytes
}

/// 線種だけを変える Restyle の本文。`kind` は 0 画層・1 ブロック・2 線種の表の値
fn line_type_body(kind: u32, value: u32, rows: &[u32]) -> Vec<u8> {
    let mut bytes = restyle_body(16, 0, (0, 0, 0, 0), (0, 0.0), 0.0, &[]);
    bytes[..4].copy_from_slice(&(rows.len() as u32).to_le_bytes());
    for word in [kind, value] {
        bytes.extend_from_slice(&word.to_le_bytes());
    }
    for row in rows {
        bytes.extend_from_slice(&row.to_le_bytes());
    }
    bytes
}

/// 線種の表を持つ R2004 の DXF
const DXF_TABLES: &[u8] = include_bytes!("../../../../../samples/dxf/text_alignments.dxf");

/// 線種の表の項目（値・フラグ・名前・説明・列）
type LineTypeItem = (u32, u32, String, String, Vec<f64>);

fn line_types(host: &mut Host, id: u64, revision: u64) -> Vec<LineTypeItem> {
    let reply = call(host, Command::LineTypes, id, revision, wire::NONE, 0, &[]);
    assert_eq!(reply.status, Status::Ok);
    assert_eq!(reply.data_kind, 8);
    let mut words = Words(&reply.data);
    let count = words.u32().unwrap();
    assert_eq!(u64::from(count), reply.result);
    let mut out = Vec::new();
    for _ in 0..count {
        let [value, flags, name, description, pattern] = [(); 5].map(|()| words.u32().unwrap());
        let mut text = |length: u32| {
            let length = length as usize;
            let padded = length.next_multiple_of(4);
            let found = String::from_utf8(words.0[..length].to_vec()).unwrap();
            assert!(words.0[length..padded].iter().all(|&byte| byte == 0));
            words.0 = &words.0[padded..];
            found
        };
        let (name, description) = (text(name), text(description));
        let pattern = (0..pattern).map(|_| words.f64().unwrap()).collect();
        out.push((value, flags, name, description, pattern));
    }
    assert!(words.0.is_empty());
    out
}

/// 線種の表は、文書エンジンの `display_line_types` を値・名前・定義で運ぶ
#[test]
fn line_types_carry_the_line_type_table() {
    let mut budget = zumen_base::Budget::default();
    let mut diag = zumen_base::Diagnostics::new();
    let cancel = zumen_base::CancelToken::new();
    let cx = &mut zumen_base::Cx::new(&mut budget, &mut diag, &cancel);
    let engines: [(&[u8], Vec<zumen_drawing::LineType>); 3] = [
        (
            DXF_TABLES,
            zumen_acad::AcadLive::open(DXF_TABLES, cx)
                .unwrap()
                .display_line_types(cx)
                .unwrap(),
        ),
        (
            JWW,
            zumen_jww::JwwLive::open(JWW, cx)
                .unwrap()
                .display_line_types(cx)
                .unwrap(),
        ),
        (
            SFC,
            zumen_sxf::SxfLive::open(SFC, cx)
                .unwrap()
                .display_line_types(cx)
                .unwrap(),
        ),
    ];
    for (bytes, expected) in engines {
        let mut host = Host::default();
        open_bytes(&mut host, bytes);
        let actual = line_types(&mut host, 2, 0);
        assert_eq!(actual.len(), expected.len());
        assert!(!actual.is_empty());
        for ((value, flags, name, description, pattern), line_type) in actual.iter().zip(&expected)
        {
            assert_eq!(*value, line_type.id.0);
            assert_eq!(name, &line_type.name);
            let definition = line_type.definition.as_ref();
            assert_eq!(flags & 1 != 0, definition.is_some());
            assert_eq!(
                flags & 2 != 0,
                definition.is_some_and(|d| d.basis == zumen_drawing::PatternBasis::Paper)
            );
            assert_eq!(
                flags & 4 != 0,
                definition.is_some_and(|d| !d.marks.is_empty())
            );
            assert_eq!(flags >> 3, 0);
            assert_eq!(
                description,
                definition.map_or("", |d| d.description.as_str())
            );
            assert_eq!(
                pattern.as_slice(),
                definition.map_or(&[][..], |d| d.pattern.as_slice())
            );
        }
        // 前提の版が違えば、表を返さない
        let stale = call(&mut host, Command::LineTypes, 3, 9, wire::NONE, 0, &[]);
        assert_eq!(stale.status, Status::Stale);
        assert!(stale.data.is_empty());
    }
}

/// 線種を線種の表の値で変えると、要素のハッシュと保存が変わり、取り消すと戻る。画層に従わせることもできる
#[test]
fn restyle_changes_the_line_type_of_rows() {
    for bytes in [DXF_TABLES, JWW, SFC] {
        let mut host = Host::default();
        open_bytes(&mut host, bytes);
        let original = saved(&mut host, 2, 0);
        let table = line_types(&mut host, 3, 0);
        let rows = select(&mut host, 4, 0, EVERYWHERE);
        let mut changed = None;
        'search: for &row in &rows {
            for (value, ..) in &table {
                let before = target_hash(&mut host, 5, 0, row);
                let reply = call(
                    &mut host,
                    Command::Restyle,
                    6,
                    0,
                    wire::NONE,
                    0,
                    &line_type_body(2, *value, &[row]),
                );
                assert!(
                    matches!(reply.status, Status::Ok | Status::Rejected),
                    "{:?}",
                    reply.status
                );
                if reply.status == Status::Ok {
                    assert_eq!(reply.result, 1);
                    if target_hash(&mut host, 7, 1, row) != before {
                        changed = Some(row);
                        break 'search;
                    }
                    // 開き直して版を 0 に戻す
                    host = Host::default();
                    open_bytes(&mut host, bytes);
                }
            }
        }
        let row = changed.unwrap();
        assert_ne!(saved(&mut host, 9, 1), original);
        let undone = call(&mut host, Command::Undo, 10, 1, wire::NONE, 0, &[]);
        assert_eq!(undone.status, Status::Ok);
        assert_eq!(saved(&mut host, 11, 2), original);
        // 画層に従わせる指定も受ける
        let reply = call(
            &mut host,
            Command::Restyle,
            12,
            2,
            wire::NONE,
            0,
            &line_type_body(0, 0, &[row]),
        );
        assert!(
            matches!(reply.status, Status::Ok | Status::Rejected),
            "{:?}",
            reply.status
        );
    }
}

fn saved(host: &mut Host, id: u64, revision: u64) -> Vec<u8> {
    let reply = call(host, Command::Save, id, revision, wire::NONE, 0, &[]);
    assert_eq!(reply.status, Status::Ok);
    reply.data
}

fn target_hash(host: &mut Host, id: u64, revision: u64, row: u32) -> Vec<u8> {
    let reply = call(
        host,
        Command::Target,
        id,
        revision,
        wire::NONE,
        0,
        &row.to_le_bytes(),
    );
    assert_eq!(reply.status, Status::Ok);
    reply.data[4..20].to_vec()
}

/// 画層を表示の画層の表の値で、色を RGB で変えると、要素のハッシュと保存が変わり、取り消すと戻る
#[test]
fn restyle_changes_the_layer_and_color_of_rows() {
    for bytes in [R2000, JWW, SFC] {
        let mut host = Host::default();
        open_bytes(&mut host, bytes);
        let original = saved(&mut host, 2, 0);
        let table = layers(&mut host, 3, 0);
        let rows = select(&mut host, 4, 0, EVERYWHERE);
        // 書き戻せる行と画層の組を探す。断られても版は進まない
        let mut changed = None;
        'search: for &row in &rows {
            for &(value, ..) in &table {
                let before = target_hash(&mut host, 5, 0, row);
                let reply = call(
                    &mut host,
                    Command::Restyle,
                    6,
                    0,
                    wire::NONE,
                    0,
                    &restyle_body(1, value, (0, 0, 0, 0), (0, 0.0), 0.0, &[row]),
                );
                assert!(
                    matches!(reply.status, Status::Ok | Status::Rejected),
                    "{:?}",
                    reply.status
                );
                if reply.status == Status::Ok {
                    assert_eq!(reply.result, 1);
                    // 同じ画層への変更もあるので、変わった組を探す
                    if target_hash(&mut host, 7, 1, row) != before {
                        changed = Some(row);
                        break 'search;
                    }
                    let undone = call(&mut host, Command::Undo, 8, 1, wire::NONE, 0, &[]);
                    assert_eq!(undone.status, Status::Ok);
                    // 取り消しも版を進めるので、開き直して版を 0 に戻す
                    host = Host::default();
                    open_bytes(&mut host, bytes);
                }
            }
        }
        let row = changed.unwrap();
        assert_ne!(saved(&mut host, 9, 1), original);
        let undone = call(&mut host, Command::Undo, 10, 1, wire::NONE, 0, &[]);
        assert_eq!(undone.status, Status::Ok);
        assert_eq!(saved(&mut host, 11, 2), original);
        // 色を形式の色番号で変える（ACI 3、Jw_cad のペン 2、SXF の既定義色 3）
        let code = if core::ptr::eq(bytes, R2000) {
            1 << 24 | 3
        } else if core::ptr::eq(bytes, JWW) {
            3 << 24 | 2
        } else {
            2 << 24 | 3
        };
        let before = target_hash(&mut host, 12, 2, row);
        let reply = call(
            &mut host,
            Command::Restyle,
            13,
            2,
            wire::NONE,
            1,
            &restyle_body(2, 0, (2, 1, code, 0), (0, 0.0), 0.0, &[row]),
        );
        assert_eq!(reply.status, Status::Ok);
        assert_eq!(reply.data_kind, 1);
        assert_ne!(target_hash(&mut host, 14, 3, row), before);
        if core::ptr::eq(bytes, R2000) {
            // DXF の RGB は R2004 から書けるので、R2000 の文書では断り、版は進まない
            let reply = call(
                &mut host,
                Command::Restyle,
                15,
                3,
                wire::NONE,
                0,
                &restyle_body(2, 0, (2, 2, 0, 0x0012_3456), (0, 0.0), 0.0, &[row]),
            );
            assert_eq!(reply.status, Status::Rejected);
            assert_eq!(reply.revision, 3);
        }
    }
}

/// 形の合わない本文と、表にない画層の値は、編集を始めずに断る
#[test]
fn malformed_restyles_are_refused() {
    let mut host = Host::default();
    open_bytes(&mut host, R2000);
    let original = saved(&mut host, 2, 0);
    let bad = [
        // 変える項目がない・予約ビット
        restyle_body(0, 0, (0, 0, 0, 0), (0, 0.0), 0.0, &[0]),
        restyle_body(32, 0, (0, 0, 0, 0), (0, 0.0), 0.0, &[0]),
        // 線種: 値が続かない・継承に値・未知の種類・表にない値
        restyle_body(16, 0, (0, 0, 0, 0), (0, 0.0), 0.0, &[]),
        line_type_body(0, 1, &[0]),
        line_type_body(1, 1, &[0]),
        line_type_body(3, 0, &[0]),
        line_type_body(2, 9999, &[0]),
        // 使わない値が 0 でない
        restyle_body(2, 5, (0, 0, 0, 0), (0, 0.0), 0.0, &[0]),
        restyle_body(1, 0, (0, 0, 0, 1), (0, 0.0), 0.0, &[0]),
        // 色: 継承に値・フラグなし・未知の体系・RGB の範囲
        restyle_body(2, 0, (0, 1, 0, 0), (0, 0.0), 0.0, &[0]),
        restyle_body(2, 0, (2, 0, 0, 0), (0, 0.0), 0.0, &[0]),
        restyle_body(2, 0, (2, 1, 4 << 24, 0), (0, 0.0), 0.0, &[0]),
        restyle_body(2, 0, (2, 2, 0, 0x0100_0000), (0, 0.0), 0.0, &[0]),
        restyle_body(2, 0, (3, 0, 0, 0), (0, 0.0), 0.0, &[0]),
        // 線幅・線種の倍率
        restyle_body(4, 0, (0, 0, 0, 0), (2, -1.0), 0.0, &[0]),
        restyle_body(4, 0, (0, 0, 0, 0), (2, f64::NAN), 0.0, &[0]),
        restyle_body(4, 0, (0, 0, 0, 0), (4, 0.0), 0.0, &[0]),
        restyle_body(8, 0, (0, 0, 0, 0), (0, 0.0), 0.0, &[0]),
        restyle_body(8, 0, (0, 0, 0, 0), (0, 0.0), f64::INFINITY, &[0]),
        // 表にない画層の値
        restyle_body(1, 9999, (0, 0, 0, 0), (0, 0.0), 0.0, &[0]),
    ];
    for (index, body) in bad.iter().enumerate() {
        let reply = call(&mut host, Command::Restyle, 3, 0, wire::NONE, 0, body);
        assert_eq!(reply.status, Status::Invalid, "{index}");
        assert_eq!(reply.revision, 0, "{index}");
    }
    assert_eq!(saved(&mut host, 4, 0), original);
    // 線幅と線種の倍率は当たる
    let reply = call(
        &mut host,
        Command::Restyle,
        5,
        0,
        wire::NONE,
        0,
        &restyle_body(12, 0, (0, 0, 0, 0), (2, 0.35), 2.0, &[0, 1]),
    );
    if reply.status != Status::Ok {
        let d = call(
            &mut host,
            Command::Diagnostics,
            9,
            0,
            wire::NONE,
            0,
            &5_u64.to_le_bytes(),
        );
        panic!("{}", String::from_utf8_lossy(&d.data));
    }
    assert_eq!(reply.result, 2);
    assert_ne!(saved(&mut host, 6, 1), original);
}

/// 文字の応答を、種類と内容の組に読む
fn texts(host: &mut Host, id: u64, revision: u64, rows: &[u32]) -> Vec<(u32, String)> {
    let reply = call(
        host,
        Command::Texts,
        id,
        revision,
        wire::NONE,
        0,
        &rows_body(rows),
    );
    assert_eq!(reply.status, Status::Ok);
    assert_eq!(reply.data_kind, 9);
    let mut words = Words(&reply.data);
    let count = words.u32().unwrap();
    assert_eq!(u64::from(count), reply.result);
    assert_eq!(count as usize, rows.len());
    let mut out = Vec::new();
    for _ in 0..count {
        let (kind, length) = (words.u32().unwrap(), words.u32().unwrap() as usize);
        let padded = length.next_multiple_of(4);
        let content = String::from_utf8(words.0[..length].to_vec()).unwrap();
        assert!(words.0[length..padded].iter().all(|&byte| byte == 0));
        words.0 = &words.0[padded..];
        out.push((kind, content));
    }
    assert!(words.0.is_empty());
    out
}

/// 行の文字は、文書エンジンの `display_text` を要求の順に運ぶ。消した行は Missing
#[test]
fn texts_carry_the_text_of_rows() {
    let mut budget = zumen_base::Budget::default();
    let mut diag = zumen_base::Diagnostics::new();
    let cancel = zumen_base::CancelToken::new();
    let cx = &mut zumen_base::Cx::new(&mut budget, &mut diag, &cancel);
    for (bytes, has_text) in [(DXF_TABLES, true), (JWW, false), (SFC, true)] {
        let mut host = Host::default();
        open_bytes(&mut host, bytes);
        let mut rows = select(&mut host, 2, 0, EVERYWHERE);
        rows.reverse();
        let actual = texts(&mut host, 3, 0, &rows);
        let expected = engine_texts(bytes, &rows, cx);
        assert_eq!(actual, expected);
        assert_eq!(actual.iter().any(|(kind, _)| *kind != 0), has_text);
        assert!(
            actual
                .iter()
                .all(|(kind, content)| *kind <= 2 && (*kind != 0 || content.is_empty()))
        );
        // 前提の版が違えば返さない。予約の語が 0 でなければ断る
        let stale = call(
            &mut host,
            Command::Texts,
            4,
            9,
            wire::NONE,
            0,
            &rows_body(&rows),
        );
        assert_eq!(stale.status, Status::Stale);
        let mut reserved = rows_body(&rows);
        reserved[4] = 1;
        let invalid = call(&mut host, Command::Texts, 5, 0, wire::NONE, 0, &reserved);
        assert_eq!(invalid.status, Status::Invalid);
        // 消した行を含めば Missing
        let removed = rows
            .iter()
            .copied()
            .find(|&row| {
                call(
                    &mut host,
                    Command::Remove,
                    6,
                    0,
                    wire::NONE,
                    0,
                    &rows_body(&[row]),
                )
                .status
                    == Status::Ok
            })
            .unwrap();
        let missing = call(
            &mut host,
            Command::Texts,
            7,
            1,
            wire::NONE,
            0,
            &rows_body(&[removed]),
        );
        assert_eq!(missing.status, Status::Missing);
    }
}

/// 文書エンジンを直接開いて、行の文字を要求の順に読む
fn engine_texts(bytes: &[u8], rows: &[u32], cx: &mut zumen_base::Cx<'_>) -> Vec<(u32, String)> {
    let mut out = Vec::new();
    if bytes == JWW {
        let live = zumen_jww::JwwLive::open(bytes, cx).unwrap();
        for &row in rows {
            out.push(live.display_text(RowId(row), cx).unwrap().unwrap());
        }
    } else if bytes == SFC {
        let live = zumen_sxf::SxfLive::open(bytes, cx).unwrap();
        for &row in rows {
            out.push(live.display_text(RowId(row), cx).unwrap().unwrap());
        }
    } else {
        let live = zumen_acad::AcadLive::open(bytes, cx).unwrap();
        for &row in rows {
            out.push(live.display_text(RowId(row), cx).unwrap().unwrap());
        }
    }
    out
}

/// SetText の本文。件数・内容のバイト数・4 バイト境界まで 0 で埋めた内容・行の順
fn set_text_body(content: &[u8], rows: &[u32]) -> Vec<u8> {
    let mut bytes = (rows.len() as u32).to_le_bytes().to_vec();
    bytes.extend_from_slice(&(content.len() as u32).to_le_bytes());
    bytes.extend_from_slice(content);
    bytes.resize(bytes.len().next_multiple_of(4), 0);
    for row in rows {
        bytes.extend_from_slice(&row.to_le_bytes());
    }
    bytes
}

/// SetText は文字の行の内容だけを 1 トランザクションで替え、取り消せる。文字でない行を含めば何も替えない
#[test]
fn set_text_replaces_the_content_of_text_rows() {
    for (bytes, content) in [(DXF_TABLES, "NEW TEXT"), (SFC, "改めた文字")] {
        let mut host = Host::default();
        open_bytes(&mut host, bytes);
        let rows = select(&mut host, 2, 0, EVERYWHERE);
        let before = texts(&mut host, 3, 0, &rows);
        let text_rows: Vec<u32> = rows
            .iter()
            .zip(&before)
            .filter(|(_, (kind, _))| *kind != 0)
            .map(|(&row, _)| row)
            .take(2)
            .collect();
        assert_eq!(text_rows.len(), 2);
        let original = saved(&mut host, 4, 0);
        let body = set_text_body(content.as_bytes(), &text_rows);
        let changed = call(&mut host, Command::SetText, 5, 0, wire::NONE, 1, &body);
        assert_eq!(changed.status, Status::Ok);
        assert_eq!(changed.result, 2);
        assert_eq!(changed.revision, 1);
        assert_eq!(changed.display_status, Status::Ok);
        let after = texts(&mut host, 6, 1, &text_rows);
        assert!(after.iter().all(|(_, text)| text == content));
        // 保存して開き直しても同じ文字
        let mut reopened = Host::default();
        open_bytes(&mut reopened, &saved(&mut host, 7, 1));
        let again = select(&mut reopened, 2, 0, EVERYWHERE);
        let all = texts(&mut reopened, 3, 0, &again);
        assert_eq!(all.iter().filter(|(_, text)| text == content).count(), 2);
        // 取り消すと元のバイト列
        let undone = call(&mut host, Command::Undo, 8, 1, wire::NONE, 0, &[]);
        assert_eq!(undone.status, Status::Ok);
        assert_eq!(saved(&mut host, 9, 2), original);
        // 文字でない行を含めば断り、版を進めない
        let other = rows
            .iter()
            .zip(&before)
            .find(|(_, (kind, _))| *kind == 0)
            .map(|(&row, _)| row)
            .unwrap();
        let body = set_text_body(b"x", &[text_rows[0], other]);
        let rejected = call(&mut host, Command::SetText, 10, 2, wire::NONE, 0, &body);
        assert_eq!(rejected.status, Status::Rejected);
        assert_eq!(rejected.revision, 2);
        // 前提の版が違えば Stale。UTF-8 でない内容・0 でない埋め草・長さの食い違いは Invalid
        let stale = call(&mut host, Command::SetText, 11, 0, wire::NONE, 0, &body);
        assert_eq!(stale.status, Status::Stale);
        let mut broken = set_text_body(&[0xff], &[text_rows[0]]);
        let invalid = call(&mut host, Command::SetText, 12, 2, wire::NONE, 0, &broken);
        assert_eq!(invalid.status, Status::Invalid);
        broken = set_text_body(b"x", &[text_rows[0]]);
        broken[9] = 1;
        let padding = call(&mut host, Command::SetText, 13, 2, wire::NONE, 0, &broken);
        assert_eq!(padding.status, Status::Invalid);
        broken = set_text_body(b"x", &[text_rows[0]]);
        broken[4] = 5;
        let length = call(&mut host, Command::SetText, 14, 2, wire::NONE, 0, &broken);
        assert_eq!(length.status, Status::Invalid);
    }
    // JWW の公開サンプルには文字がないので、どの行も断る
    let mut host = Host::default();
    open_bytes(&mut host, JWW);
    let rows = select(&mut host, 2, 0, EVERYWHERE);
    let body = set_text_body(b"x", &rows[..1]);
    let rejected = call(&mut host, Command::SetText, 3, 0, wire::NONE, 0, &body);
    assert_eq!(rejected.status, Status::Rejected);
}

/// 線と円を持つ定義 B1、B1 を入れ子に挿す定義 B2、B2 を挿すモデル空間
const BLOCKS_DXF: &[u8] = b"0\nSECTION\n2\nBLOCKS\n\
0\nBLOCK\n8\n0\n2\nB1\n70\n0\n10\n0\n20\n0\n3\nB1\n\
0\nLINE\n8\n0\n10\n0\n20\n0\n11\n1\n21\n1\n\
0\nCIRCLE\n8\n0\n62\n1\n10\n0\n20\n0\n40\n1\n\
0\nENDBLK\n\
0\nBLOCK\n8\n0\n2\nB2\n70\n0\n10\n0\n20\n0\n3\nB2\n\
0\nINSERT\n8\n0\n2\nB1\n10\n5\n20\n5\n\
0\nENDBLK\n0\nENDSEC\n\
0\nSECTION\n2\nENTITIES\n0\nINSERT\n8\n0\n2\nB2\n10\n0\n20\n0\n0\nENDSEC\n0\nEOF\n";

/// 記号の定義を要求の順に返し、範囲・予約の語を断る
#[test]
fn symbols_carry_the_display_of_definitions() {
    let mut budget = zumen_base::Budget::default();
    let mut diag = zumen_base::Diagnostics::new();
    let cancel = zumen_base::CancelToken::new();
    let cx = &mut zumen_base::Cx::new(&mut budget, &mut diag, &cancel);
    let live = zumen_acad::AcadLive::open(BLOCKS_DXF, cx).unwrap();
    let total = live.display_symbol_count();
    assert!(total >= 2);
    let mut host = Host::default();
    open_bytes(&mut host, BLOCKS_DXF);
    // 逆の順で、同じ定義を 2 度求めてもよい
    let mut indices: Vec<u32> = (0..total).rev().collect();
    indices.push(total - 1);
    let reply = call(
        &mut host,
        Command::Symbols,
        2,
        0,
        wire::NONE,
        0,
        &rows_body(&indices),
    );
    assert_eq!(reply.status, Status::Ok);
    assert_eq!(reply.data_kind, 10);
    assert_eq!(reply.result, indices.len() as u64);
    let mut expected = zumen_live::display::Display::with_rows(0);
    let mut words = Words(&reply.data);
    assert_eq!(words.u32(), Some(total));
    assert_eq!(words.u32(), Some(indices.len() as u32));
    let mut first = 0;
    for &index in &indices {
        let rows = live
            .display_symbol(index, &mut expected, cx)
            .unwrap()
            .unwrap();
        assert_eq!((words.u32(), words.u32()), (Some(first), Some(rows)));
        first += rows;
    }
    assert!(first >= 3);
    let packet = zumen_live::display_wire::encode(&expected, true, cx).unwrap();
    assert_eq!(words.0, packet.as_slice());
    // 定義を求めなければ、数だけを返す
    let empty = call(
        &mut host,
        Command::Symbols,
        3,
        0,
        wire::NONE,
        0,
        &rows_body(&[]),
    );
    assert_eq!((empty.status, empty.result), (Status::Ok, 0));
    assert_eq!(
        empty.data.get(..8),
        Some(
            [total.to_le_bytes(), 0_u32.to_le_bytes()]
                .concat()
                .as_slice()
        )
    );
    // 範囲外の定義・0 でない予約の語は Invalid、前提の版が違えば Stale
    let outside = call(
        &mut host,
        Command::Symbols,
        4,
        0,
        wire::NONE,
        0,
        &rows_body(&[total]),
    );
    assert_eq!(outside.status, Status::Invalid);
    let mut reserved = rows_body(&[0]);
    reserved[4] = 1;
    let invalid = call(&mut host, Command::Symbols, 5, 0, wire::NONE, 0, &reserved);
    assert_eq!(invalid.status, Status::Invalid);
    let stale = call(
        &mut host,
        Command::Symbols,
        7,
        9,
        wire::NONE,
        0,
        &rows_body(&[0]),
    );
    assert_eq!(stale.status, Status::Stale);
    // 表示のフラグは付けられない
    let mut header = [
        b'Z', b'L', b'I', b'P', 1, 0, 19, 0, 1, 0, 0, 0, 0, 0, 0, 0, 80, 0, 0, 0, 0, 0, 0, 0,
    ];
    assert!(wire::Header::parse(&header).is_some());
    header[20] = 1;
    assert!(wire::Header::parse(&header).is_none());
    // SXF は複合図形の定義を返す
    let live = zumen_sxf::SxfLive::open(SFC, cx).unwrap();
    let total = live.display_symbol_count();
    assert!(total > 0);
    let mut host = Host::default();
    open_bytes(&mut host, SFC);
    let indices: Vec<u32> = (0..total).collect();
    let reply = call(
        &mut host,
        Command::Symbols,
        2,
        0,
        wire::NONE,
        0,
        &rows_body(&indices),
    );
    assert_eq!(reply.status, Status::Ok);
    let mut expected = zumen_live::display::Display::with_rows(0);
    for &index in &indices {
        live.display_symbol(index, &mut expected, cx)
            .unwrap()
            .unwrap();
    }
    let packet = zumen_live::display_wire::encode(&expected, true, cx).unwrap();
    assert!(reply.data.ends_with(&packet));
    assert_eq!(reply.data.len(), 8 + indices.len() * 8 + packet.len());
    // JWW は記号の定義を返す。公開サンプルの定義の数は文書エンジンと同じ
    let live = zumen_jww::JwwLive::open(JWW, cx).unwrap();
    let total = live.display_symbol_count();
    let mut host = Host::default();
    open_bytes(&mut host, JWW);
    let indices: Vec<u32> = (0..total).collect();
    let reply = call(
        &mut host,
        Command::Symbols,
        2,
        0,
        wire::NONE,
        0,
        &rows_body(&indices),
    );
    assert_eq!(reply.status, Status::Ok);
    assert_eq!(reply.data.get(..4), Some(total.to_le_bytes().as_slice()));
    let mut expected = zumen_live::display::Display::with_rows(0);
    for &index in &indices {
        live.display_symbol(index, &mut expected, cx)
            .unwrap()
            .unwrap();
    }
    let packet = zumen_live::display_wire::encode(&expected, true, cx).unwrap();
    assert!(reply.data.ends_with(&packet));
    let outside = call(
        &mut host,
        Command::Symbols,
        3,
        0,
        wire::NONE,
        0,
        &rows_body(&[total]),
    );
    assert_eq!(outside.status, Status::Invalid);
}

fn draw_body(template: u32, kind: u32, values: [f64; 5]) -> Vec<u8> {
    let mut data = template.to_le_bytes().to_vec();
    data.extend_from_slice(&kind.to_le_bytes());
    for value in values {
        data.extend_from_slice(&value.to_le_bytes());
    }
    data
}

/// 線分・円・円弧を、表示の座標で遠くに足す。足した行は表示の座標で選ばれ、保存して開き直しても同じ場所で選ばれ、
/// 取り消すと消える。雛形にならない行は断られ、版は進まない
fn draws_shapes_in_display_coordinates(bytes: &[u8]) {
    let mut host = Host::default();
    open_bytes(&mut host, bytes);
    let rows = select(&mut host, 2, 0, EVERYWHERE);
    assert!(!rows.is_empty());
    let shapes = [
        (1, [FAR, FAR, FAR * 1.5, FAR * 1.5, 0.0]),
        (2, [FAR * 1.2, FAR * 1.2, 10.0, 0.0, 0.0]),
        (3, [FAR * 1.4, FAR * 1.4, 10.0, 30.0, 120.0]),
    ];
    let mut id = 10;
    let mut revision = 0;
    let mut drawn = Vec::new();
    for (kind, values) in shapes {
        let mut added = None;
        for &row in &rows {
            id += 1;
            let reply = call(
                &mut host,
                Command::Draw,
                id,
                revision,
                wire::NONE,
                0,
                &draw_body(row, kind, values),
            );
            assert!(matches!(reply.status, Status::Ok | Status::Rejected));
            if reply.status == Status::Ok {
                added = Some(reply);
                break;
            }
            assert_eq!(reply.revision, revision);
        }
        let reply = added.unwrap();
        revision += 1;
        assert_eq!(reply.revision, revision);
        assert_eq!((reply.data_kind, reply.result), (5, 1));
        let mut words = Words(&reply.data);
        assert_eq!(words.u32(), Some(1));
        drawn.push(words.u32().unwrap());
        assert!(words.0.is_empty());
    }
    let far = [FAR * 0.5, FAR * 0.5, FAR * 2.0, FAR * 2.0];
    let mut found = select(&mut host, 30, revision, far);
    found.sort_unstable();
    assert_eq!(found, drawn);
    let saved = call(&mut host, Command::Save, 31, revision, wire::NONE, 0, &[]).data;
    let mut reopened = Host::default();
    open_bytes(&mut reopened, &saved);
    assert_eq!(select(&mut reopened, 2, 0, far).len(), drawn.len());
    for step in 0..3 {
        let undone = call(
            &mut host,
            Command::Undo,
            32 + step,
            revision,
            wire::NONE,
            0,
            &[],
        );
        assert_eq!(undone.status, Status::Ok);
        revision += 1;
    }
    assert!(select(&mut host, 40, revision, far).is_empty());
}

#[test]
fn draw_adds_shapes_like_a_template_row() {
    for bytes in [super::DXF, JWW, SFC] {
        draws_shapes_in_display_coordinates(bytes);
    }
}

/// 形の種類・使わない値・半径・有限でない値の誤りは invalid、生きていない行は missing で、版は進まない
#[test]
fn malformed_draws_are_refused() {
    let mut host = Host::default();
    open_bytes(&mut host, super::DXF);
    for (id, (row, kind, values, status)) in [
        (0, 0, [0.0, 0.0, 1.0, 1.0, 0.0], Status::Invalid),
        (0, 4, [0.0, 0.0, 1.0, 1.0, 0.0], Status::Invalid),
        (0, 1, [0.0, 0.0, 1.0, 1.0, 1.0], Status::Invalid),
        (0, 2, [0.0, 0.0, 0.0, 0.0, 0.0], Status::Invalid),
        (0, 2, [0.0, 0.0, 1.0, 1.0, 0.0], Status::Invalid),
        (0, 3, [0.0, 0.0, -1.0, 0.0, 90.0], Status::Invalid),
        (0, 1, [f64::NAN, 0.0, 1.0, 1.0, 0.0], Status::Invalid),
        (0, 3, [0.0, 0.0, 1.0, f64::INFINITY, 90.0], Status::Invalid),
        (u32::MAX - 1, 1, [0.0, 0.0, 1.0, 1.0, 0.0], Status::Missing),
    ]
    .into_iter()
    .enumerate()
    {
        let reply = call(
            &mut host,
            Command::Draw,
            id as u64 + 2,
            0,
            wire::NONE,
            0,
            &draw_body(row, kind, values),
        );
        assert_eq!(reply.status, status, "{kind} {values:?}");
        assert_eq!(reply.revision, 0);
    }
    // 長さの違う本文は、要求ごと断る
    let mut long = draw_body(0, 1, [0.0, 0.0, 1.0, 1.0, 0.0]);
    long.extend_from_slice(&[0; 4]);
    let reply = call(&mut host, Command::Draw, 20, 0, wire::NONE, 0, &long);
    assert_ne!(reply.status, Status::Ok);
    assert_eq!(select(&mut host, 21, 0, EVERYWHERE).len(), 2);
}

/// 表示のスタイルの色は、どの形式も `display::color` の符号化で、色番号の体系は形式の体系。RGB も持つ色は RGB を運ぶ。
/// 公開サンプルの要素は自分の色番号を持たないので、表示に出る行の色を形式の色番号に変えて確かめる
#[test]
fn display_colors_share_one_encoding() {
    use zumen_drawing::{Changes, Color, ColorCode, ColorSystem, Edit, Inherit, Restyle};
    use zumen_live::display::{Display, color};
    let mut budget = zumen_base::Budget::default();
    let mut diag = zumen_base::Diagnostics::new();
    let cancel = zumen_base::CancelToken::new();
    let cx = &mut zumen_base::Cx::new(&mut budget, &mut diag, &cancel);
    let restyle = |target, system, value| Changes {
        document: None,
        edits: vec![Edit::Restyle {
            target,
            style: Restyle {
                layer: None,
                color: Some(Inherit::Own(Color {
                    code: Some(ColorCode { system, value }),
                    rgb: None,
                })),
                line_type: None,
                line_type_scale: None,
                line_weight: None,
            },
        }],
    };
    // 行の色をどれも符号化のとおりに読み、変えた行は形式の体系の色番号になる
    let check = |display: &Display, rows: usize, changed: u32, system: u8, value: u32| {
        for row in 0..rows as u32 {
            for [_, bits] in display.row_styles(row).into_iter().flatten() {
                assert!(matches!(bits >> 24, 0..=3), "{bits:#x}");
                if bits >> 24 == 0 {
                    assert_eq!(bits, color::LAYER);
                }
                if bits >> 24 == 1 {
                    assert_eq!(bits, color::BLOCK);
                }
            }
        }
        let styles: Vec<_> = display.row_styles(changed).into_iter().flatten().collect();
        assert!(!styles.is_empty());
        // RGB も持つ色（JWW のペンなど）は RGB を運ぶ
        for [_, bits] in styles {
            assert!(
                color::code_of(bits) == Some((system, value)) || color::rgb_of(bits).is_some(),
                "{bits:#x}"
            );
        }
    };
    macro_rules! case {
        ($live:expr, $system:expr, $wire:expr, $value:expr) => {{
            let mut live = $live;
            let changed = live
                .display_order(cx)
                .unwrap()
                .into_iter()
                .find(|&row| {
                    let target = live.seen_of_row(row, cx).unwrap().unwrap();
                    live.apply(&restyle(target, $system, $value), cx).is_ok()
                })
                .unwrap();
            check(live.display(), live.row_count(), changed.0, $wire, $value);
        }};
    }
    case!(
        zumen_acad::AcadLive::open(super::DXF, cx).unwrap(),
        ColorSystem::Aci,
        color::ACI,
        3
    );
    case!(
        zumen_jww::JwwLive::open(JWW, cx).unwrap(),
        ColorSystem::Jww,
        color::JWW,
        2
    );
    case!(
        zumen_sxf::SxfLive::open(SFC, cx).unwrap(),
        ColorSystem::Sxf,
        color::SXF,
        3
    );
}

/// P2Z・SFZ のアーカイブと属性ファイル（SAF）を常駐 IPC で開き、編集・保存・取り消しでアーカイブの他項目と診断を保つ
#[test]
fn sxf_archives_and_saf_round_trip_through_ipc() {
    let saf_missing = br#"<?xml version="1.0" encoding="UTF-8"?>
<SxfAttributeXML version="3.1" date="2026-11-19" sxfFile="D0PL001Z.SFC" application="test">
  <Figure id="999" name="unbound"/>
</SxfAttributeXML>"#;
    let extra = b"RASTER-PLACEHOLDER";
    let sfz = zumen_zip::write(&[
        ("D0PL001Z.SFC", SFC),
        ("D0PL001Z.SAF", saf_missing.as_slice()),
        ("OTHER/NOTE.TXT", extra.as_slice()),
    ])
    .unwrap();

    let mut host = Host::default();
    let mut open_data = 1_u32.to_le_bytes().to_vec();
    open_data.extend_from_slice(&0_u32.to_le_bytes());
    open_data.extend_from_slice(&sfz);
    let opened = call(
        &mut host,
        Command::Open,
        1,
        wire::NONE,
        wire::NONE,
        1,
        &open_data,
    );
    assert_eq!(opened.status, Status::Ok);
    assert_eq!(opened.display_status, Status::Ok);
    // アーカイブ内の SAF の未解決図形診断が Open で報告される
    assert!(opened.diagnostics > 0);
    let diag_reply = call(
        &mut host,
        Command::Diagnostics,
        2,
        wire::NONE,
        wire::NONE,
        0,
        &1_u64.to_le_bytes(),
    );
    assert_eq!(diag_reply.status, Status::Ok);
    let diag_text = String::from_utf8(diag_reply.data).unwrap();
    assert!(diag_text.contains("sxf.saf-figure.dropped"), "{diag_text}");

    let initial_archive = saved(&mut host, 3, 0);
    let rows = select(&mut host, 4, 0, EVERYWHERE);
    let moved_row = rows
        .iter()
        .copied()
        .find(|&row| {
            let reply = call(
                &mut host,
                Command::Transform,
                5,
                0,
                0,
                1,
                &transform(&[row], Affine::translation(FAR, FAR)),
            );
            reply.status == Status::Ok
        })
        .unwrap();
    let far = [FAR * 0.5, FAR * 0.5, FAR * 2.0, FAR * 2.0];
    assert_eq!(select(&mut host, 6, 1, far), [moved_row]);

    let edited_archive = saved(&mut host, 7, 1);
    assert_ne!(edited_archive, initial_archive);
    // 保存したアーカイブから他項目が失われず、再オープンできる
    let mut budget = zumen_base::Budget::default();
    let mut diag = zumen_base::Diagnostics::new();
    let cancel = zumen_base::CancelToken::new();
    let cx = &mut zumen_base::Cx::new(&mut budget, &mut diag, &cancel);
    let parsed_zip = zumen_zip::read(&edited_archive, cx).unwrap();
    let names: Vec<&str> = parsed_zip.entries().iter().map(|e| e.name()).collect();
    assert_eq!(names, ["D0PL001Z.SFC", "D0PL001Z.SAF", "OTHER/NOTE.TXT"]);
    assert_eq!(
        parsed_zip.extract(&parsed_zip.entries()[1], cx).unwrap(),
        saf_missing
    );
    assert_eq!(
        parsed_zip.extract(&parsed_zip.entries()[2], cx).unwrap(),
        extra
    );

    let undone = call(&mut host, Command::Undo, 8, 1, wire::NONE, 1, &[]);
    assert_eq!(undone.status, Status::Ok);
    assert_eq!(saved(&mut host, 9, 2), initial_archive);

    // 単独の SFC に SAF を添えて開くこともでき、SXF でない形式に SAF を添えると Invalid
    let mut with_saf = 1_u32.to_le_bytes().to_vec();
    with_saf.extend_from_slice(&(wire::OPEN_HAS_SAF | saf_missing.len() as u32).to_le_bytes());
    with_saf.extend_from_slice(SFC);
    with_saf.extend_from_slice(saf_missing);
    let mut host_saf = Host::default();
    let reply_saf = call(
        &mut host_saf,
        Command::Open,
        1,
        wire::NONE,
        wire::NONE,
        1,
        &with_saf,
    );
    assert_eq!(reply_saf.status, Status::Ok);
    assert!(reply_saf.diagnostics > 0);

    let mut dxf_with_saf = 1_u32.to_le_bytes().to_vec();
    dxf_with_saf.extend_from_slice(&(wire::OPEN_HAS_SAF | saf_missing.len() as u32).to_le_bytes());
    dxf_with_saf.extend_from_slice(super::DXF);
    dxf_with_saf.extend_from_slice(saf_missing);
    let mut host_bad = Host::default();
    assert_eq!(
        call(
            &mut host_bad,
            Command::Open,
            1,
            wire::NONE,
            wire::NONE,
            1,
            &dxf_with_saf
        )
        .status,
        Status::Invalid
    );
}

/// SetText は書式や段落（`\P`）を持つ MTEXT の内容も 1 トランザクションで替え、取り消せる
#[test]
fn set_text_replaces_formatted_mtext_with_paragraphs() {
    let mtext_dxf =
        b"0\nSECTION\n2\nHEADER\n9\n$ACADVER\n1\nAC1018\n9\n$INSUNITS\n70\n4\n0\nENDSEC\n\
0\nSECTION\n2\nENTITIES\n\
0\nMTEXT\n5\n20\n100\nAcDbEntity\n8\n0\n100\nAcDbMText\n\
10\n1.0\n20\n2.0\n40\n2.5\n41\n40.0\n71\n1\n\
1\n{\\fArial;FIRST}\\P{\\C1;SECOND}\n\
0\nENDSEC\n0\nEOF\n";
    let mut host = Host::default();
    open_bytes(&mut host, mtext_dxf);
    let original = saved(&mut host, 2, 0);
    let before = texts(&mut host, 3, 0, &[0]);
    assert_eq!(before, [(2, "{\\fArial;FIRST}\\P{\\C1;SECOND}".to_owned())]);
    let next = "{\\H2x;CHANGED}\\PLINE2\\P\\S1/2;";
    let reply = call(
        &mut host,
        Command::SetText,
        4,
        0,
        0,
        1,
        &set_text_body(next.as_bytes(), &[0]),
    );
    assert_eq!(reply.status, Status::Ok);
    assert_eq!(reply.result, 1);
    assert_eq!(texts(&mut host, 5, 1, &[0]), [(2, next.to_owned())]);
    let undone = call(&mut host, Command::Undo, 6, 1, wire::NONE, 0, &[]);
    assert_eq!(undone.status, Status::Ok);
    assert_eq!(saved(&mut host, 7, 2), original);
}

fn select_point_body(point: [f64; 2], tolerance: f64) -> Vec<u8> {
    let mut out = Vec::with_capacity(24);
    out.extend_from_slice(&point[0].to_bits().to_le_bytes());
    out.extend_from_slice(&point[1].to_bits().to_le_bytes());
    out.extend_from_slice(&tolerance.to_bits().to_le_bytes());
    out
}

fn decode_u32_rows(data: &[u8], expected_result: u64) -> Vec<u32> {
    let mut words = Words(data);
    let count = words.u32().unwrap();
    assert_eq!(u64::from(count), expected_result);
    let rows: Vec<u32> = (0..count).map(|_| words.u32().unwrap()).collect();
    assert!(words.0.is_empty());
    rows
}

fn select_point(host: &mut Host, id: u64, revision: u64, point: [f64; 2], tol: f64) -> Vec<u32> {
    let reply = call(
        host,
        Command::SelectPoint,
        id,
        revision,
        wire::NONE,
        0,
        &select_point_body(point, tol),
    );
    assert_eq!(reply.status, Status::Ok);
    assert_eq!(reply.data_kind, 5);
    decode_u32_rows(&reply.data, reply.result)
}

/// SelectPoint は囲む箱の空白や円の内側を除いて線分・円弧・記号定義の実形だけで当たり判定し、移動・取り消しに追従する
#[test]
fn select_point_hits_geometry_and_symbol_instances_across_formats() {
    let dxf = b"0\nSECTION\n2\nHEADER\n9\n$ACADVER\n1\nAC1018\n9\n$INSUNITS\n70\n4\n0\nENDSEC\n\
0\nSECTION\n2\nBLOCKS\n\
0\nBLOCK\n8\n0\n2\nFRAME\n70\n0\n10\n0.0\n20\n0.0\n\
0\nLINE\n8\n0\n10\n0.0\n20\n0.0\n11\n20.0\n21\n0.0\n\
0\nLINE\n8\n0\n10\n20.0\n20\n0.0\n11\n20.0\n21\n20.0\n\
0\nENDBLK\n8\n0\n\
0\nENDSEC\n\
0\nSECTION\n2\nENTITIES\n\
0\nLINE\n5\n10\n8\n0\n10\n0.0\n20\n0.0\n11\n10.0\n21\n10.0\n\
0\nCIRCLE\n5\n11\n8\n0\n10\n50.0\n20\n0.0\n40\n10.0\n\
0\nINSERT\n5\n12\n8\n0\n2\nFRAME\n10\n100.0\n20\n0.0\n41\n1.0\n42\n1.0\n50\n0.0\n\
0\nENDSEC\n0\nEOF\n";
    let mut host = Host::default();
    open_bytes(&mut host, dxf);

    // 対角線 (0,0)-(10,10): 線分上 (5,5) は当たり、囲む箱の中の空白 (0,10) は外れる
    assert_eq!(select(&mut host, 2, 0, [0.0, 9.5, 0.5, 10.0]), [0]);
    assert_eq!(select_point(&mut host, 3, 0, [5.0, 5.0], 0.25), [0]);
    assert!(select_point(&mut host, 4, 0, [0.0, 10.0], 0.25).is_empty());

    // 円: 円周上 (60, 0) は当たり、中心 (50, 0) は外れる
    assert_eq!(select_point(&mut host, 5, 0, [60.1, 0.0], 0.25), [1]);
    assert!(select_point(&mut host, 6, 0, [50.0, 0.0], 0.25).is_empty());

    // 記号挿入 FRAME: 定義の辺 (110, 0) と (120, 10) は当たり、囲む箱の左上 (102, 18) は外れる
    assert_eq!(select_point(&mut host, 7, 0, [110.0, 0.1], 0.25), [2]);
    assert_eq!(select_point(&mut host, 8, 0, [120.0, 10.0], 0.25), [2]);
    assert!(select_point(&mut host, 9, 0, [102.0, 18.0], 0.25).is_empty());

    // 移動と取り消しに追従し、古い版は Stale、不正な座標や負の許容距離は Invalid
    let moved = call(
        &mut host,
        Command::Transform,
        10,
        0,
        wire::NONE,
        0,
        &transform(&[0], Affine::translation(0.0, 20.0)),
    );
    assert_eq!(moved.status, Status::Ok);
    assert_eq!(
        call(
            &mut host,
            Command::SelectPoint,
            11,
            0,
            wire::NONE,
            0,
            &select_point_body([5.0, 5.0], 0.25),
        )
        .status,
        Status::Stale
    );
    assert!(select_point(&mut host, 12, 1, [5.0, 5.0], 0.25).is_empty());
    assert_eq!(select_point(&mut host, 13, 1, [5.0, 25.0], 0.25), [0]);
    assert_eq!(
        call(&mut host, Command::Undo, 14, 1, wire::NONE, 0, &[]).status,
        Status::Ok
    );
    assert_eq!(select_point(&mut host, 15, 2, [5.0, 5.0], 0.25), [0]);

    for [bx, by, btol] in [
        [f64::NAN, 0.0, 1.0],
        [0.0, f64::INFINITY, 1.0],
        [0.0, 0.0, -0.1],
        [0.0, 0.0, f64::INFINITY],
    ] {
        assert_eq!(
            call(
                &mut host,
                Command::SelectPoint,
                16,
                2,
                wire::NONE,
                0,
                &select_point_body([bx, by], btol),
            )
            .status,
            Status::Invalid
        );
    }

    // JWW と SFC の公開サンプルでも Draw で足した遠方の線分が線分上だけで選ばれ、空白では選ばれない
    for sample in [JWW, SFC] {
        let mut shost = Host::default();
        open_bytes(&mut shost, sample);
        let rows = select(&mut shost, 2, 0, EVERYWHERE);
        let mut req_id = 3_u64;
        let drawn = rows
            .into_iter()
            .find_map(|row| {
                req_id += 1;
                let reply = call(
                    &mut shost,
                    Command::Draw,
                    req_id,
                    0,
                    wire::NONE,
                    0,
                    &draw_body(row, 1, [FAR, FAR, FAR + 10.0, FAR + 10.0, 0.0]),
                );
                (reply.status == Status::Ok).then_some(reply)
            })
            .unwrap();
        let added = decode_u32_rows(&drawn.data, drawn.result);
        assert_eq!(added.len(), 1);
        assert_eq!(
            select_point(&mut shost, req_id + 1, 1, [FAR + 5.0, FAR + 5.0], 0.2),
            added
        );
        assert!(select_point(&mut shost, req_id + 2, 1, [FAR, FAR + 10.0], 0.2).is_empty());
    }
}

fn pad_utf8(out: &mut Vec<u8>, text: &str) {
    out.extend_from_slice(text.as_bytes());
    let rem = text.len() % 4;
    if rem != 0 {
        out.extend(core::iter::repeat_n(0_u8, 4 - rem));
    }
}

fn dirty_symbols(host: &mut Host, id: u64, revision: u64, clear: bool) -> Vec<u32> {
    let reply = call(
        host,
        Command::DirtySymbols,
        id,
        revision,
        wire::NONE,
        0,
        &u32::from(clear).to_le_bytes(),
    );
    assert_eq!(reply.status, Status::Ok);
    assert_eq!(reply.data_kind, 5);
    decode_u32_rows(&reply.data, reply.result)
}

fn transform_symbol_body(symbol: u32, matrix: Affine, elements: &[u32]) -> Vec<u8> {
    let mut out = Vec::with_capacity(56 + elements.len() * 4);
    out.extend_from_slice(&symbol.to_le_bytes());
    out.extend_from_slice(&(elements.len() as u32).to_le_bytes());
    for v in matrix.m {
        out.extend_from_slice(&v.to_bits().to_le_bytes());
    }
    for &elem in elements {
        out.extend_from_slice(&elem.to_le_bytes());
    }
    out
}

/// TransformSymbol と DirtySymbols は DXF・SFC・JWW の記号定義の編集と Undo を追跡し、保存を一致させる
#[test]
fn dirty_symbols_and_transform_symbol_sync_across_formats() {
    let dxf = b"0\nSECTION\n2\nHEADER\n9\n$ACADVER\n1\nAC1018\n9\n$INSUNITS\n70\n4\n0\nENDSEC\n\
0\nSECTION\n2\nTABLES\n0\nTABLE\n2\nBLOCK_RECORD\n5\n1\n70\n1\n\
0\nBLOCK_RECORD\n5\n1F\n100\nAcDbSymbolTableRecord\n100\nAcDbBlockTableRecord\n2\nB1\n\
0\nENDTAB\n0\nENDSEC\n\
0\nSECTION\n2\nBLOCKS\n\
0\nBLOCK\n5\n20\n8\n0\n100\nAcDbEntity\n100\nAcDbBlockBegin\n2\nB1\n70\n0\n10\n0.0\n20\n0.0\n\
0\nLINE\n5\n21\n8\n0\n100\nAcDbEntity\n100\nAcDbLine\n10\n0.0\n20\n0.0\n11\n10.0\n21\n0.0\n\
0\nENDBLK\n5\n22\n8\n0\n100\nAcDbEntity\n100\nAcDbBlockEnd\n\
0\nENDSEC\n\
0\nSECTION\n2\nENTITIES\n\
0\nINSERT\n5\n30\n8\n0\n100\nAcDbEntity\n100\nAcDbBlockReference\n2\nB1\n10\n0.0\n20\n0.0\n\
0\nENDSEC\n0\nEOF\n";

    for sample in [dxf.as_slice(), SFC] {
        let mut host = Host::default();
        open_bytes(&mut host, sample);
        let original = saved(&mut host, 2, 0);
        assert!(dirty_symbols(&mut host, 3, 0, false).is_empty());

        let sym_info = call(&mut host, Command::Symbols, 4, 0, wire::NONE, 0, &[0; 8]);
        assert_eq!(sym_info.status, Status::Ok);
        let total = Words(&sym_info.data).u32().unwrap();
        let target_sym = (0..total)
            .find(|&idx| {
                let mut req = 1_u32.to_le_bytes().to_vec();
                req.extend_from_slice(&0_u32.to_le_bytes());
                req.extend_from_slice(&idx.to_le_bytes());
                let rep = call(
                    &mut host,
                    Command::Symbols,
                    5 + u64::from(idx),
                    0,
                    wire::NONE,
                    0,
                    &req,
                );
                let mut w = Words(&rep.data);
                let _ = (w.u32(), w.u32(), w.u32());
                w.u32().unwrap_or(0) > 0
            })
            .unwrap();

        let moved = call(
            &mut host,
            Command::TransformSymbol,
            200,
            0,
            wire::NONE,
            1,
            &transform_symbol_body(target_sym, Affine::translation(5.0, 7.0), &[0]),
        );
        assert_eq!(moved.status, Status::Ok);
        assert_eq!(moved.result, 1);
        assert!(dirty_symbols(&mut host, 201, 1, false).contains(&target_sym));
        assert!(dirty_symbols(&mut host, 202, 1, true).contains(&target_sym));
        assert!(dirty_symbols(&mut host, 203, 1, false).is_empty());

        let undone = call(&mut host, Command::Undo, 204, 1, wire::NONE, 1, &[]);
        assert_eq!(undone.status, Status::Ok);
        assert!(dirty_symbols(&mut host, 205, 2, true).contains(&target_sym));
        assert_eq!(saved(&mut host, 206, 2), original);
    }
}

/// EditLayer・EditLineType・AddSymbol・AddElement が 1 トランザクションずつ確定し、Undo で完全に元へ戻る
#[test]
fn table_edits_and_extended_elements_round_trip_and_undo() {
    let dxf = b"0\nSECTION\n2\nHEADER\n9\n$ACADVER\n1\nAC1018\n9\n$HANDSEED\n5\n80\n9\n$INSUNITS\n70\n4\n0\nENDSEC\n\
0\nSECTION\n2\nTABLES\n\
0\nTABLE\n2\nLTYPE\n5\n5\n100\nAcDbSymbolTable\n70\n3\n\
0\nLTYPE\n5\n14\n330\n5\n100\nAcDbSymbolTableRecord\n100\nAcDbLinetypeTableRecord\n2\nByBlock\n70\n0\n3\n\n72\n65\n73\n0\n40\n0.0\n\
0\nLTYPE\n5\n15\n330\n5\n100\nAcDbSymbolTableRecord\n100\nAcDbLinetypeTableRecord\n2\nByLayer\n70\n0\n3\n\n72\n65\n73\n0\n40\n0.0\n\
0\nLTYPE\n5\n16\n330\n5\n100\nAcDbSymbolTableRecord\n100\nAcDbLinetypeTableRecord\n2\nContinuous\n70\n0\n3\nSolid\n72\n65\n73\n0\n40\n0.0\n\
0\nENDTAB\n\
0\nTABLE\n2\nLAYER\n5\n2\n100\nAcDbSymbolTable\n70\n1\n\
0\nLAYER\n5\n10\n330\n2\n100\nAcDbSymbolTableRecord\n100\nAcDbLayerTableRecord\n2\n0\n70\n0\n62\n7\n6\nContinuous\n\
0\nENDTAB\n\
0\nTABLE\n2\nSTYLE\n5\n3\n100\nAcDbSymbolTable\n70\n1\n\
0\nSTYLE\n5\n11\n330\n3\n100\nAcDbSymbolTableRecord\n100\nAcDbTextStyleTableRecord\n2\nStandard\n70\n0\n40\n0.0\n41\n1.0\n50\n0.0\n71\n0\n42\n2.5\n3\ntxt\n4\n\n\
0\nENDTAB\n\
0\nTABLE\n2\nBLOCK_RECORD\n5\n1\n100\nAcDbSymbolTable\n70\n1\n\
0\nBLOCK_RECORD\n5\n1F\n330\n1\n100\nAcDbSymbolTableRecord\n100\nAcDbBlockTableRecord\n2\n*Model_Space\n\
0\nENDTAB\n\
0\nENDSEC\n\
0\nSECTION\n2\nBLOCKS\n\
0\nBLOCK\n5\n20\n330\n1F\n100\nAcDbEntity\n8\n0\n100\nAcDbBlockBegin\n2\n*Model_Space\n70\n0\n10\n0.0\n20\n0.0\n\
0\nENDBLK\n5\n21\n330\n1F\n100\nAcDbEntity\n8\n0\n100\nAcDbBlockEnd\n\
0\nENDSEC\n\
0\nSECTION\n2\nENTITIES\n\
0\nLINE\n5\n30\n330\n1F\n100\nAcDbEntity\n8\n0\n100\nAcDbLine\n10\n0.0\n20\n0.0\n11\n10.0\n21\n0.0\n\
0\nENDSEC\n0\nEOF\n";

    let mut host = Host::default();
    open_bytes(&mut host, dxf);
    let initial = saved(&mut host, 2, 0);
    let mut rev = 0_u64;
    let mut id = 10_u64;

    // 1. 画層 WALL を追加 -> 状態変更 -> 削除
    let mut add_layer = Vec::new();
    add_layer.extend_from_slice(&2_u32.to_le_bytes()); // Add
    add_layer.extend_from_slice(&0_u32.to_le_bytes()); // layer=0
    add_layer.extend_from_slice(&(1_u32 | 2 | 16).to_le_bytes()); // state + color + name
    add_layer.extend_from_slice(&0b0111_u32.to_le_bytes()); // visible, printable, editable
    add_layer.extend_from_slice(&2_u32.to_le_bytes()); // RGB
    add_layer.extend_from_slice(&0_u32.to_le_bytes());
    add_layer.extend_from_slice(&0x00112233_u32.to_le_bytes());
    add_layer.extend_from_slice(&0_u32.to_le_bytes()); // weight_kind
    add_layer.extend_from_slice(&0.0_f64.to_bits().to_le_bytes());
    add_layer.extend_from_slice(&0_u32.to_le_bytes()); // lt_kind
    add_layer.extend_from_slice(&0_u32.to_le_bytes()); // lt_val
    add_layer.extend_from_slice(&4_u32.to_le_bytes()); // "WALL"
    pad_utf8(&mut add_layer, "WALL");
    id += 1;
    assert_eq!(
        call(
            &mut host,
            Command::EditLayer,
            id,
            rev,
            wire::NONE,
            1,
            &add_layer
        )
        .status,
        Status::Ok
    );
    rev += 1;

    // 2. 線種 DASHED を追加
    let mut add_lt = Vec::new();
    add_lt.extend_from_slice(&2_u32.to_le_bytes()); // Add
    add_lt.extend_from_slice(&0_u32.to_le_bytes());
    add_lt.extend_from_slice(&0_u32.to_le_bytes()); // Element basis
    add_lt.extend_from_slice(&6_u32.to_le_bytes()); // "DASHED"
    add_lt.extend_from_slice(&4_u32.to_le_bytes()); // "Dash"
    add_lt.extend_from_slice(&2_u32.to_le_bytes()); // 2 pattern items
    pad_utf8(&mut add_lt, "DASHED");
    pad_utf8(&mut add_lt, "Dash");
    add_lt.extend_from_slice(&5.0_f64.to_bits().to_le_bytes());
    add_lt.extend_from_slice(&(-2.5_f64).to_bits().to_le_bytes());
    id += 1;
    assert_eq!(
        call(
            &mut host,
            Command::EditLineType,
            id,
            rev,
            wire::NONE,
            1,
            &add_lt
        )
        .status,
        Status::Ok
    );
    rev += 1;

    // 3. 記号定義 SYM1 を追加 -> DirtySymbols で検出 -> AddElement(Symbol) で配置
    let mut add_sym = Vec::new();
    add_sym.extend_from_slice(&0_u32.to_le_bytes()); // template=0
    add_sym.extend_from_slice(&4_u32.to_le_bytes()); // "SYM1"
    add_sym.extend_from_slice(&1_u32.to_le_bytes()); // 1 element
    add_sym.extend_from_slice(&0_u32.to_le_bytes());
    add_sym.extend_from_slice(&0.0_f64.to_bits().to_le_bytes());
    add_sym.extend_from_slice(&0.0_f64.to_bits().to_le_bytes());
    pad_utf8(&mut add_sym, "SYM1");
    add_sym.extend_from_slice(&1_u32.to_le_bytes()); // Line
    add_sym.extend_from_slice(&0_u32.to_le_bytes());
    for v in [0.0_f64, 0.0, 20.0, 0.0, 0.0] {
        add_sym.extend_from_slice(&v.to_bits().to_le_bytes());
    }
    id += 1;
    let sym_rep = call(
        &mut host,
        Command::AddSymbol,
        id,
        rev,
        wire::NONE,
        1,
        &add_sym,
    );
    assert_eq!(sym_rep.status, Status::Ok);
    let sym_idx = sym_rep.result as u32;
    rev += 1;
    id += 1;
    assert!(dirty_symbols(&mut host, id, rev, true).contains(&sym_idx));

    // 4. 折れ線・文字・記号挿入を AddElement で追加
    let mut add_poly = Vec::new();
    add_poly.extend_from_slice(&0_u32.to_le_bytes()); // template
    add_poly.extend_from_slice(&1_u32.to_le_bytes()); // Polyline
    add_poly.extend_from_slice(&1_u32.to_le_bytes()); // closed
    add_poly.extend_from_slice(&3_u32.to_le_bytes()); // 3 vertices
    for [x, y, b] in [[100.0_f64, 0.0, 0.0], [110.0, 0.0, 0.5], [110.0, 10.0, 0.0]] {
        add_poly.extend_from_slice(&x.to_bits().to_le_bytes());
        add_poly.extend_from_slice(&y.to_bits().to_le_bytes());
        add_poly.extend_from_slice(&b.to_bits().to_le_bytes());
    }
    id += 1;
    assert_eq!(
        call(
            &mut host,
            Command::AddElement,
            id,
            rev,
            wire::NONE,
            0,
            &add_poly
        )
        .status,
        Status::Ok
    );
    rev += 1;

    let mut add_txt = Vec::new();
    add_txt.extend_from_slice(&0_u32.to_le_bytes());
    add_txt.extend_from_slice(&2_u32.to_le_bytes()); // Text
    add_txt.extend_from_slice(&5_u32.to_le_bytes()); // "HELLO"
    add_txt.extend_from_slice(&0_u32.to_le_bytes());
    for v in [200.0_f64, 10.0, 3.5, 0.0, 1.0] {
        add_txt.extend_from_slice(&v.to_bits().to_le_bytes());
    }
    pad_utf8(&mut add_txt, "HELLO");
    id += 1;
    let txt_rep = call(
        &mut host,
        Command::AddElement,
        id,
        rev,
        wire::NONE,
        0,
        &add_txt,
    );
    assert_eq!(txt_rep.status, Status::Ok);
    let txt_row = *decode_u32_rows(&txt_rep.data, txt_rep.result)
        .first()
        .unwrap();
    rev += 1;
    id += 1;
    assert_eq!(
        texts(&mut host, id, rev, &[txt_row]),
        [(1, "HELLO".to_owned())]
    );

    let mut add_ins = Vec::new();
    add_ins.extend_from_slice(&0_u32.to_le_bytes());
    add_ins.extend_from_slice(&3_u32.to_le_bytes()); // Symbol
    add_ins.extend_from_slice(&sym_idx.to_le_bytes());
    add_ins.extend_from_slice(&0_u32.to_le_bytes());
    for v in [300.0_f64, 50.0, 1.0, 1.0, 0.0] {
        add_ins.extend_from_slice(&v.to_bits().to_le_bytes());
    }
    id += 1;
    let ins_rep = call(
        &mut host,
        Command::AddElement,
        id,
        rev,
        wire::NONE,
        0,
        &add_ins,
    );
    assert_eq!(ins_rep.status, Status::Ok);
    let ins_row = *decode_u32_rows(&ins_rep.data, ins_rep.result)
        .first()
        .unwrap();
    rev += 1;
    id += 1;
    assert_eq!(
        select_point(&mut host, id, rev, [310.0, 50.0], 0.25),
        [ins_row]
    );

    // 6 段すべて Undo すると保存バイト列が完全に初期状態へ戻る
    for _ in 0..6 {
        id += 1;
        assert_eq!(
            call(&mut host, Command::Undo, id, rev, wire::NONE, 1, &[]).status,
            Status::Ok
        );
        rev += 1;
    }
    id += 1;
    assert_eq!(saved(&mut host, id, rev), initial);
}

/// アーカイブの展開・`texts`・`symbols` は、配列の確保の前に展開量の予算を数えて上限で止まる
#[test]
fn document_archive_texts_and_symbols_count_decoded_bytes() {
    use super::super::document::Problem;
    use zumen_base::{Budget, CancelToken, Cx, Diagnostics, Error, Kind, Limit, Limits, codes};

    let sfz = zumen_zip::write(&[
        ("D0PL001Z.SFC", SFC),
        ("OTHER/A.TXT", b"a"),
        ("OTHER/B.TXT", b"b"),
    ])
    .unwrap();
    let mut probe_budget = Budget::default();
    let mut probe_diag = Diagnostics::new();
    let cancel = CancelToken::new();
    zumen_zip::read(
        &sfz,
        &mut Cx::new(&mut probe_budget, &mut probe_diag, &cancel),
    )
    .unwrap();
    let zip_used = probe_budget.used(Kind::DecodedBytes);

    let mut archive_budget = Budget::new(Limits::default().with(
        Kind::DecodedBytes,
        Limit::Finite(zip_used.saturating_add(32)),
    ));
    let mut archive_diag = Diagnostics::new();
    let archive_err = Document::open(
        &sfz,
        1,
        &mut Cx::new(&mut archive_budget, &mut archive_diag, &cancel),
    )
    .err()
    .unwrap();
    assert!(matches!(archive_err, Error::Stopped(_)), "{archive_err:?}");
    assert!(
        archive_diag
            .iter()
            .any(|d| d.code() == codes::BUDGET_DECODED_BYTES_EXCEEDED)
    );

    let mut open_budget = Budget::default();
    let mut open_diag = Diagnostics::new();
    let doc = Document::open(
        super::DXF,
        1,
        &mut Cx::new(&mut open_budget, &mut open_diag, &cancel),
    )
    .unwrap();

    let mut text_budget =
        Budget::new(Limits::default().with(Kind::DecodedBytes, Limit::Finite(32)));
    let mut text_diag = Diagnostics::new();
    let text_err = doc
        .texts(
            &[RowId(0), RowId(1)],
            &mut Cx::new(&mut text_budget, &mut text_diag, &cancel),
        )
        .unwrap_err();
    assert!(
        matches!(text_err, Problem::Core(Error::Stopped(_))),
        "{text_err:?}"
    );
    assert!(
        text_diag
            .iter()
            .any(|d| d.code() == codes::BUDGET_DECODED_BYTES_EXCEEDED)
    );

    let mut sym_budget = Budget::new(Limits::default().with(Kind::DecodedBytes, Limit::Finite(8)));
    let mut sym_diag = Diagnostics::new();
    let sym_err = doc
        .symbols(
            &[0, 1],
            &mut Cx::new(&mut sym_budget, &mut sym_diag, &cancel),
        )
        .err()
        .unwrap();
    assert!(
        matches!(sym_err, Problem::Core(Error::Stopped(_))),
        "{sym_err:?}"
    );
    assert!(
        sym_diag
            .iter()
            .any(|d| d.code() == codes::BUDGET_DECODED_BYTES_EXCEEDED)
    );
}
