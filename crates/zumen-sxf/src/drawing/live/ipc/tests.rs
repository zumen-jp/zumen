use std::collections::BTreeMap;

use zumen_base::{Budget, CancelToken, Cx, Diagnostics, Kind, Limit, Limits};
use zumen_drawing::{Affine, Changes, Drawing2D, Edit, Placed, Seen, Shape, id};
use zumen_live::RowId;

use super::super::SxfLive;

const SAMPLE: &[u8] = include_bytes!("../../../../../../samples/sxf/ezsxf-writer-all-features.sfc");
const EVERYWHERE: [f64; 4] = [-1e300, -1e300, 1e300, 1e300];

#[test]
fn rows_answer_the_ids_and_hashes_of_the_drawing_model() {
    let mut budget = Budget::default();
    let mut diag = Diagnostics::new();
    let cancel = CancelToken::new();
    let cx = &mut Cx::new(&mut budget, &mut diag, &cancel);
    for threads in [1, 4] {
        let mut live = SxfLive::open_with_threads(SAMPLE, threads, cx).unwrap();
        for round in 0..2 {
            let drawing: Drawing2D = live.drawing(cx).unwrap();
            let mut expected = BTreeMap::new();
            id::each_element(&drawing, |_, id, found| -> Result<(), ()> {
                expected.insert(id, found.hash());
                Ok(())
            })
            .unwrap();
            // 生きた行の ID とハッシュは、図面モデルの要素のものと一致する
            let mut actual = BTreeMap::new();
            for index in 0..live.row_count() {
                let row = RowId(index as u32);
                match live.seen_of_row(row, cx).unwrap() {
                    Some(Seen { id, hash }) => {
                        assert_eq!(expected.get(&id), Some(&hash), "{id}");
                        assert!(actual.insert(id, hash).is_none());
                    }
                    None => assert!(!live.table.is_alive(row)),
                }
            }
            assert!(!actual.is_empty(), "threads {threads} round {round}");
            // 矩形の選択は、予算を数えない選択と同じ行を返す
            let mut rows = Vec::new();
            live.select_rect_with_cx(EVERYWHERE, &mut rows, cx).unwrap();
            let mut plain = live.select_sheet(EVERYWHERE);
            let mut sorted = rows.clone();
            sorted.sort();
            plain.sort();
            assert!(!plain.is_empty());
            assert_eq!(sorted, plain);
            // 用紙の線を、行の今の ID とハッシュでそのまま動かせる
            let row = rows
                .iter()
                .copied()
                .find(|&row| {
                    matches!(
                        live.placed(row),
                        Some(Placed::Paper(element)) if matches!(element.shape, Shape::Line { .. })
                    )
                })
                .unwrap();
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
fn sheet_of_row_maps_paper_lines_into_the_display_bounds() {
    let mut budget = Budget::default();
    let mut diag = Diagnostics::new();
    let cancel = CancelToken::new();
    let live = SxfLive::open(SAMPLE, &mut Cx::new(&mut budget, &mut diag, &cancel)).unwrap();
    let mut checked = [0usize; 2];
    for index in 0..live.row_count() {
        let row = RowId(index as u32);
        let points = match live.placed(row) {
            Some(Placed::Model(element)) => match &element.shape {
                Shape::Line { start, end } => [(start.x, start.y), (end.x, end.y)],
                _ => continue,
            },
            Some(Placed::Paper(element)) => match &element.shape {
                Shape::Line { start, end } => [(start.x, start.y), (end.x, end.y)],
                _ => continue,
            },
            None => continue,
        };
        let Some(sheet) = live.sheet_of_row(row) else {
            continue;
        };
        let [x0, y0, x1, y1] = live.table.bounds(row).unwrap();
        for (x, y) in points {
            let (x, y) = sheet.apply(x, y);
            let margin = 1e-6 * (1.0 + x.abs().max(y.abs()));
            assert!(x >= x0 - margin && x <= x1 + margin, "{x} {x0} {x1}");
            assert!(y >= y0 - margin && y <= y1 + margin, "{y} {y0} {y1}");
        }
        checked[usize::from(matches!(live.placed(row), Some(Placed::Model(_))))] += 1;
    }
    // 公開サンプルにはビューがなく、実寸の空間の行は表示されない。用紙の行だけを確かめる
    assert!(checked[0] > 0, "{checked:?}");
}

#[test]
fn a_stopped_selection_keeps_the_output_and_reports_the_stop() {
    let mut budget = Budget::default();
    let mut diag = Diagnostics::new();
    let cancel = CancelToken::new();
    let live = SxfLive::open(SAMPLE, &mut Cx::new(&mut budget, &mut diag, &cancel)).unwrap();
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
