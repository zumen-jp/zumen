use std::collections::{BTreeMap, BTreeSet};

use zumen_base::{Budget, CancelToken, Cx, Diagnostics, Kind, Limit, Limits};
use zumen_drawing::{Affine, Changes, Drawing2D, Edit, Hash, Seen, Shape, id};

use super::super::JwwLive;
use super::DEFINITION_SPACES;
use zumen_live::RowId;
use zumen_live::display::{Display, NO_OWNER};

const SAMPLE: &[u8] = include_bytes!("../../../../../../samples/jww/ezjww-geometry.jww");
const EVERYWHERE: [f64; 4] = [-1e300, -1e300, 1e300, 1e300];

#[test]
fn rows_answer_the_ids_and_hashes_of_the_drawing_model() {
    let mut budget = Budget::default();
    let mut diag = Diagnostics::new();
    let cancel = CancelToken::new();
    let cx = &mut Cx::new(&mut budget, &mut diag, &cancel);
    for threads in [1, 4] {
        let mut live = JwwLive::open_with_threads(SAMPLE, threads, cx).unwrap();
        for round in 0..2 {
            let drawing: Drawing2D = live.drawing(cx).unwrap();
            let mut expected = BTreeMap::new();
            for space in &drawing.spaces {
                let place = id::space(space.id);
                let mut namer = id::Namer::new(&place);
                for (index, element) in space.elements.iter().enumerate() {
                    expected.insert(namer.next(index, element), Hash::element(element));
                }
            }
            // 生きた最上位の行は、図面モデルの要素と ID・ハッシュが 1 対 1 に一致する
            let mut actual = BTreeMap::new();
            for index in 0..live.table.len() {
                let row = RowId(index as u32);
                let top = live
                    .table
                    .row(row)
                    .is_some_and(|found| found.space.0 < DEFINITION_SPACES);
                match live.seen_of_row(row, cx).unwrap() {
                    Some(Seen { id, hash }) if top => assert!(actual.insert(id, hash).is_none()),
                    Some(_) => {}
                    None => assert!(!live.table.is_alive(row)),
                }
            }
            assert_eq!(actual, expected, "threads {threads} round {round}");
            // 矩形の選択は、ID で返す選択と同じ要素を返す
            let mut rows = Vec::new();
            live.select_rect_with_cx(EVERYWHERE, &mut rows, cx).unwrap();
            let mut selected: Vec<String> = rows
                .iter()
                .map(|&row| live.seen_of_row(row, cx).unwrap().unwrap().id)
                .collect();
            let mut ids = live.select_sheet(EVERYWHERE, cx).unwrap();
            selected.sort();
            ids.sort();
            assert!(!ids.is_empty());
            assert_eq!(selected, ids);
            // 行の今の ID とハッシュで、そのまま動かせる
            let row = rows.first().copied().unwrap();
            let target = live.seen_of_row(row, cx).unwrap().unwrap();
            live.apply(
                &Changes {
                    document: None,
                    edits: vec![Edit::Transform {
                        target,
                        transform: Affine {
                            m: [1.0, 0.0, 5.0, 0.0, 1.0, -3.0],
                        },
                    }],
                },
                cx,
            )
            .unwrap();
        }
    }
}

#[test]
fn sheet_of_row_maps_the_space_to_the_display_bounds() {
    let mut budget = Budget::default();
    let mut diag = Diagnostics::new();
    let cancel = CancelToken::new();
    let cx = &mut Cx::new(&mut budget, &mut diag, &cancel);
    let live = JwwLive::open(SAMPLE, cx).unwrap();
    let mut checked = 0;
    for index in 0..live.table.len() {
        let row = RowId(index as u32);
        let Some(found) = live.table.row(row) else {
            continue;
        };
        let elem = found.element();
        let Shape::Line { start, end } = elem.shape else {
            continue;
        };
        if !live.table.is_alive(row) || found.space.0 >= DEFINITION_SPACES {
            continue;
        }
        let sheet = live.sheet_of_row(row).unwrap();
        let [x0, y0, x1, y1] = live.table.bounds(row).unwrap();
        for point in [start, end] {
            let (x, y) = sheet.apply(point.x, point.y);
            let margin = 1e-6 * (1.0 + x.abs().max(y.abs()));
            assert!(x >= x0 - margin && x <= x1 + margin, "{x} {x0} {x1}");
            assert!(y >= y0 - margin && y <= y1 + margin, "{y} {y0} {y1}");
        }
        checked += 1;
    }
    assert!(checked > 0);
}

#[test]
fn a_stopped_selection_keeps_the_output_and_reports_the_stop() {
    let mut budget = Budget::default();
    let mut diag = Diagnostics::new();
    let cancel = CancelToken::new();
    let live = JwwLive::open(SAMPLE, &mut Cx::new(&mut budget, &mut diag, &cancel)).unwrap();
    let mut budget = Budget::new(Limits::default().with(Kind::Work, Limit::Finite(0)));
    let mut diag = Diagnostics::new();
    let mut rows = vec![RowId(7)];
    let result = live.select_rect_with_cx(
        EVERYWHERE,
        &mut rows,
        &mut Cx::new(&mut budget, &mut diag, &cancel),
    );
    assert!(matches!(result, Err(zumen_sdk::Error::Stopped(_))));
    assert_eq!(rows, [RowId(7)]);
    assert_eq!(diag.iter().count(), 1);
}

/// 表示の行が使うスタイルの画層の値
fn styled_layers(display: &Display) -> BTreeSet<u32> {
    let mut out = BTreeSet::new();
    for (owner, style) in [
        (&display.segment_owner, &display.segment_style),
        (&display.arc_owner, &display.arc_style),
        (&display.text_owner, &display.text_style),
        (&display.instance_owner, &display.instance_style),
    ] {
        for index in 0..owner.len() {
            if owner.get(index).is_some_and(|&owner| owner != NO_OWNER)
                && let Some([layer, _]) = style.get(index)
            {
                out.insert(*layer);
            }
        }
    }
    out
}

#[test]
fn display_layers_match_the_drawing_and_cover_the_display() {
    let mut budget = Budget::default();
    let mut diag = Diagnostics::new();
    let cancel = CancelToken::new();
    let cx = &mut Cx::new(&mut budget, &mut diag, &cancel);
    let live = JwwLive::open(SAMPLE, cx).unwrap();
    let drawing: Drawing2D = live.drawing(cx).unwrap();
    let pairs = live.display_layers(cx).unwrap();
    // 画層の組は図面モデルの画層と同じ順・同じ値で、値は `LayerId`
    let layers: Vec<_> = pairs.iter().map(|(_, layer)| layer.clone()).collect();
    assert_eq!(layers, drawing.layers);
    assert!(pairs.iter().all(|(value, layer)| *value == layer.id.0));
    // 表示の行のスタイルの画層は、どれも組の値にある
    let values: BTreeSet<u32> = pairs.iter().map(|&(value, _)| value).collect();
    let used = styled_layers(live.display());
    assert!(!used.is_empty());
    assert!(used.is_subset(&values), "{used:?} {values:?}");
    // 止まったときは止まった理由を 1 つ診断する
    for kind in [Kind::Work, Kind::DecodedBytes] {
        let mut budget = Budget::new(Limits::default().with(kind, Limit::Finite(0)));
        let mut diag = Diagnostics::new();
        let result = live.display_layers(&mut Cx::new(&mut budget, &mut diag, &cancel));
        assert!(
            matches!(result, Err(zumen_sdk::Error::Stopped(_))),
            "{kind:?}"
        );
        assert_eq!(diag.iter().count(), 1, "{kind:?}");
    }
}

/// 図面モデルの用紙の並び（ビューの順、その空間の `order`）で、最上位の要素の ID を並べる
fn drawing_order(drawing: &Drawing2D) -> Vec<String> {
    let mut out = Vec::new();
    let sheet = drawing.sheets.first().unwrap();
    for item in &sheet.order {
        let zumen_drawing::SheetItem::View(view) = item else {
            panic!("JWW の用紙に要素はない");
        };
        let id = sheet.views[*view].space;
        let space = drawing.spaces.iter().find(|space| space.id == id).unwrap();
        let place = id::space(space.id);
        let mut namer = id::Namer::new(&place);
        let ids: Vec<String> = space
            .elements
            .iter()
            .enumerate()
            .map(|(index, element)| namer.next(index, element))
            .collect();
        out.extend(space.order.iter().map(|&index| ids[index].clone()));
    }
    out
}

#[test]
fn display_order_matches_the_drawing_order() {
    let mut budget = Budget::default();
    let mut diag = Diagnostics::new();
    let cancel = CancelToken::new();
    let cx = &mut Cx::new(&mut budget, &mut diag, &cancel);
    let mut live = JwwLive::open(SAMPLE, cx).unwrap();
    let mut first = None;
    for round in 0..2 {
        let drawing: Drawing2D = live.drawing(cx).unwrap();
        let order = live.display_order(cx).unwrap();
        let ids: Vec<String> = order
            .iter()
            .map(|&row| live.seen_of_row(row, cx).unwrap().unwrap().id)
            .collect();
        assert_eq!(ids, drawing_order(&drawing), "round {round}");
        // 2 回目は、1 回目の最初の行を動かした後
        let row = *first.get_or_insert(order[0]);
        let target = live.seen_of_row(row, cx).unwrap().unwrap();
        live.apply(
            &Changes {
                document: None,
                edits: vec![Edit::Transform {
                    target,
                    transform: Affine {
                        m: [1.0, 0.0, 5.0, 0.0, 1.0, -3.0],
                    },
                }],
            },
            cx,
        )
        .unwrap();
    }
}

#[test]
fn a_stopped_order_reports_the_stop() {
    let mut budget = Budget::default();
    let mut diag = Diagnostics::new();
    let cancel = CancelToken::new();
    let live = JwwLive::open(SAMPLE, &mut Cx::new(&mut budget, &mut diag, &cancel)).unwrap();
    for kind in [Kind::Work, Kind::Elements, Kind::DecodedBytes] {
        let mut budget = Budget::new(Limits::default().with(kind, Limit::Finite(0)));
        let mut diag = Diagnostics::new();
        let result = live.display_order(&mut Cx::new(&mut budget, &mut diag, &cancel));
        assert!(
            matches!(result, Err(zumen_sdk::Error::Stopped(_))),
            "{kind:?}"
        );
        assert_eq!(diag.iter().count(), 1, "{kind:?}");
    }
}

#[test]
fn display_line_types_cover_the_drawing() {
    let mut budget = Budget::default();
    let mut diag = Diagnostics::new();
    let cancel = CancelToken::new();
    let cx = &mut Cx::new(&mut budget, &mut diag, &cancel);
    let live = JwwLive::open(SAMPLE, cx).unwrap();
    let drawing = live.drawing(cx).unwrap();
    let line_types = live.display_line_types(cx).unwrap();
    // 番号は重複せず、版によらない 18 種類を含む
    let ids: BTreeSet<u32> = line_types.iter().map(|line_type| line_type.id.0).collect();
    assert_eq!(ids.len(), line_types.len());
    assert!((1..=9).chain(11..=19).all(|code| ids.contains(&code)));
    // 図面モデルの線種は、同じ値で表にある
    assert!(!drawing.line_types.is_empty());
    for line_type in &drawing.line_types {
        assert!(line_types.contains(line_type), "{line_type:?}");
    }
    for kind in [Kind::Work, Kind::DecodedBytes] {
        let mut budget = Budget::new(Limits::default().with(kind, Limit::Finite(0)));
        let mut diag = Diagnostics::new();
        let result = live.display_line_types(&mut Cx::new(&mut budget, &mut diag, &cancel));
        assert!(
            matches!(result, Err(zumen_sdk::Error::Stopped(_))),
            "{kind:?}"
        );
        assert_eq!(diag.iter().count(), 1, "{kind:?}");
    }
}

/// 図面モデルの要素の ID から、文字の種類（1 文字・2 書式つきの文字・0 ほか）と内容への表
fn drawing_texts(drawing: &Drawing2D) -> BTreeMap<String, (u32, String)> {
    fn of<P>(shape: &Shape<P>) -> (u32, String) {
        match shape {
            Shape::Text(text) => (1, text.content.clone()),
            Shape::Formatted(text) => (2, text.content.clone()),
            _ => (0, String::new()),
        }
    }
    let mut out = BTreeMap::new();
    id::each_element(drawing, |_, id, found| -> Result<(), ()> {
        let text = match found {
            id::Found::Model(element) => of(&element.shape),
            id::Found::Paper(element) => of(&element.shape),
        };
        out.insert(id, text);
        Ok(())
    })
    .unwrap();
    out
}

/// 描く順の行の文字は、同じ ID の図面モデルの要素の文字と一致する。返した文字の行の数
fn check_texts(live: &JwwLive, cx: &mut Cx<'_>) -> usize {
    let expected = drawing_texts(&live.drawing(cx).unwrap());
    let mut texts = 0;
    for row in live.display_order(cx).unwrap() {
        let id = live.seen_of_row(row, cx).unwrap().unwrap().id;
        let actual = live.display_text(row, cx).unwrap().unwrap();
        assert_eq!(actual, expected[&id], "{id}");
        texts += usize::from(actual.0 != 0);
    }
    texts
}

#[test]
fn a_stopped_text_read_reports_the_stop() {
    let mut budget = Budget::default();
    let mut diag = Diagnostics::new();
    let cancel = CancelToken::new();
    let live = JwwLive::open(SAMPLE, &mut Cx::new(&mut budget, &mut diag, &cancel)).unwrap();
    let mut budget = Budget::default();
    let mut diag = Diagnostics::new();
    let cx = &mut Cx::new(&mut budget, &mut diag, &cancel);
    let row = live.display_order(cx).unwrap().into_iter().next().unwrap();
    // 公開サンプルに文字がなく、文書エンジンもファイルモデルも文字の記録のない図面には文字を足さないので、
    // 行 1 つの数えだけを止める。内容のバイト数の数えは acad・sxf の試験で確かめる
    let mut budget = Budget::new(Limits::default().with(Kind::Work, Limit::Finite(0)));
    let mut diag = Diagnostics::new();
    let result = live.display_text(row, &mut Cx::new(&mut budget, &mut diag, &cancel));
    assert!(matches!(result, Err(zumen_sdk::Error::Stopped(_))));
    assert_eq!(diag.iter().count(), 1);
}

#[test]
fn display_text_matches_the_drawing() {
    let mut budget = Budget::default();
    let mut diag = Diagnostics::new();
    let cancel = CancelToken::new();
    let cx = &mut Cx::new(&mut budget, &mut diag, &cancel);
    // 公開サンプルに文字はない。文字でない行は 0 と空を返す
    let mut live = JwwLive::open(SAMPLE, cx).unwrap();
    assert_eq!(check_texts(&live, cx), 0);
    // 消した行は `None`
    let order = live.display_order(cx).unwrap();
    let removed = order.into_iter().find(|&row| {
        let target = live.seen_of_row(row, cx).unwrap().unwrap();
        let changes = Changes {
            document: None,
            edits: vec![Edit::Remove { target }],
        };
        live.apply(&changes, cx).is_ok()
    });
    let row = removed.unwrap();
    assert_eq!(live.display_text(row, cx).unwrap(), None);
    check_texts(&live, cx);
}

/// 公開サンプルに文字がないので、文字でない行と消した行が `None` になることだけを確かめる
#[test]
fn display_retexted_skips_rows_without_text() {
    let mut budget = Budget::default();
    let mut diag = Diagnostics::new();
    let cancel = CancelToken::new();
    let cx = &mut Cx::new(&mut budget, &mut diag, &cancel);
    let live = JwwLive::open(SAMPLE, cx).unwrap();
    for row in live.display_order(cx).unwrap() {
        assert!(live.display_retexted(row, "x", cx).unwrap().is_none());
    }
    let outside = RowId(u32::try_from(live.table.len()).unwrap());
    assert!(live.display_retexted(outside, "x", cx).unwrap().is_none());
}

/// 記号の定義の要素を、定義の座標のまま 1 要素 1 行で書く。入れ子の記号はインスタンスのまま。
/// 範囲外の定義は `None`、止まれば止まった理由を返す
#[test]
fn display_symbol_writes_definitions_in_their_coordinates() {
    use super::super::equivalence_common::{Archive, Spec};
    let mut spec = Spec::new(700);
    spec.groups[0] = (3, 1.0, [3; 16]);
    spec.groups[3] = (3, 50.0, [3; 16]);
    let mut archive = Archive::new(700);
    archive.count(1);
    archive.tag("CDataBlock");
    archive.base(1, 1);
    archive.reals(&[5.0, 5.0, 1.0, 1.0, 0.0]);
    archive.out.extend(2u32.to_le_bytes());
    archive.count(2);
    archive.block("部品");
    archive.tag("CDataList");
    archive.base(1, 1);
    archive.out.extend(2u32.to_le_bytes());
    archive.out.extend([0; 8]);
    archive.cstring("外側");
    archive.count(2);
    archive.tag("CDataBlock");
    archive.base(1, 1);
    archive.reals(&[7.0, 8.0, 2.0, 0.5, core::f64::consts::FRAC_PI_2]);
    archive.out.extend(1u32.to_le_bytes());
    archive.line();
    archive.out.extend(0u32.to_le_bytes());
    let mut bytes = spec.build();
    bytes.extend(archive.out);
    let mut budget = Budget::default();
    let mut diag = Diagnostics::new();
    let cancel = CancelToken::new();
    let cx = &mut Cx::new(&mut budget, &mut diag, &cancel);
    let live = JwwLive::open(&bytes, cx).unwrap();
    assert_eq!(live.display_symbol_count(), 2);
    let mut display = Display::with_rows(0);
    assert_eq!(live.display_symbol(2, &mut display, cx).unwrap(), None);
    assert_eq!(live.display_symbol(0, &mut display, cx).unwrap(), Some(1));
    assert_eq!(live.display_symbol(1, &mut display, cx).unwrap(), Some(2));
    let mut expected = Display::with_rows(0);
    for index in [0, 1] {
        for (_, element) in live.definition_rows_alive(index).unwrap() {
            let (counts, style) = super::super::display_definition_counts_and_style(
                &element,
                &live.symbol_index_by_key,
            );
            expected.push_with(counts, style, |w| {
                super::super::write_definition_display(&element, &live.symbol_index_by_key, w);
            });
        }
    }
    assert_eq!(display.canonical(), expected.canonical());
    // 定義の 1 つ目の行は、縮尺 1/50 の用紙の座標ではなく、定義の座標の線分
    let mut first = Display::with_rows(0);
    live.display_symbol(0, &mut first, cx).unwrap();
    let mut line = Display::with_rows(0);
    let (_, element) = live.definition_rows_alive(0).unwrap().next().unwrap();
    let (_, style) =
        super::super::display_definition_counts_and_style(&element, &live.symbol_index_by_key);
    line.push_with([1, 0, 0, 0], style, |w| {
        w.push_segment([0.0, 0.0], [10.0, 0.0]);
    });
    assert_eq!(first.canonical(), line.canonical());
    // 作業の予算が尽きれば止まる
    let mut budget = Budget::new(Limits::default().with(Kind::Work, Limit::Finite(0)));
    let mut diag = Diagnostics::new();
    let cx = &mut Cx::new(&mut budget, &mut diag, &cancel);
    let mut display = Display::with_rows(0);
    assert!(live.display_symbol(1, &mut display, cx).is_err());

    // 点選択: 用紙座標で入れ子の線分 (ox+12, oy+13)..(ox+12, oy+33) は当たり、囲む箱の中の空白 (ox+6, oy+25) は外れる
    let mut budget = Budget::default();
    let mut diag = Diagnostics::new();
    let cx = &mut Cx::new(&mut budget, &mut diag, &cancel);
    let [ox, oy] = live.paper_origin;
    let mut rect_hit = Vec::new();
    let probe = [ox + 6.0, oy + 25.0];
    live.select_rect_with_cx(
        [
            probe[0] - 0.1,
            probe[1] - 0.1,
            probe[0] + 0.1,
            probe[1] + 0.1,
        ],
        &mut rect_hit,
        cx,
    )
    .unwrap();
    assert_eq!(rect_hit, [RowId(0)]);
    let mut hit = vec![RowId(99)];
    live.select_point_with_cx([ox + 12.0, oy + 20.0], 0.25, &mut hit, cx)
        .unwrap();
    assert_eq!(hit, [RowId(99), RowId(0)]);
    let mut miss = Vec::new();
    live.select_point_with_cx(probe, 0.25, &mut miss, cx)
        .unwrap();
    assert!(miss.is_empty());
    let mut tight = Budget::new(Limits::default().with(Kind::Work, Limit::Finite(2)));
    let mut tight_diag = Diagnostics::new();
    let tight_cx = &mut Cx::new(&mut tight, &mut tight_diag, &cancel);
    let mut preserved = vec![RowId(77)];
    assert!(
        live.select_point_with_cx([ox + 12.0, oy + 20.0], 0.25, &mut preserved, tight_cx)
            .is_err()
    );
    assert_eq!(preserved, [RowId(77)]);
    assert_eq!(tight_diag.len(), 1);
}
