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
        let Shape::Line { start, end } = &found.element.shape else {
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
