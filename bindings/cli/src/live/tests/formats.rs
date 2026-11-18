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
