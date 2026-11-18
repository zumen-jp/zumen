use std::collections::BTreeMap;

use zumen_base::{Budget, CancelToken, Cx, Diagnostics, Kind, Limit, Limits};
use zumen_drawing::{Affine, Changes, Drawing2D, Edit, Hash, Seen, Shape, id};

use super::super::JwwLive;
use super::DEFINITION_SPACES;
use zumen_live::RowId;

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
