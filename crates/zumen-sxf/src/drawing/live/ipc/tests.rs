use std::collections::{BTreeMap, BTreeSet};

use zumen_base::{Budget, CancelToken, Cx, Diagnostics, Kind, Limit, Limits};
use zumen_drawing::{Affine, Changes, Drawing2D, Edit, Placed, Seen, Shape, id};
use zumen_live::RowId;
use zumen_live::display::{Display, NO_OWNER};

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
    let live = SxfLive::open(SAMPLE, cx).unwrap();
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

/// 図面モデルの用紙の `order` で、表示に出る要素の ID を並べる。ビューは置く空間の要素を、最初の 1 回だけ並べる
fn drawing_order(drawing: &Drawing2D) -> Vec<String> {
    let mut ids = BTreeMap::new();
    id::each_element(drawing, |at, id, _| -> Result<(), ()> {
        ids.insert(format!("{at:?}"), id);
        Ok(())
    })
    .unwrap();
    let sheet = drawing.sheets.first().unwrap();
    let mut placed = BTreeSet::new();
    let mut out = Vec::new();
    for item in &sheet.order {
        match item {
            zumen_drawing::SheetItem::Element(index) => {
                let at = zumen_drawing::Position::Paper {
                    sheet: sheet.id,
                    index: *index,
                };
                out.push(ids[&format!("{at:?}")].clone());
            }
            zumen_drawing::SheetItem::View(view) => {
                let id = sheet.views[*view].space;
                if !placed.insert(id) {
                    continue;
                }
                let space = drawing.spaces.iter().find(|space| space.id == id).unwrap();
                for &index in &space.order {
                    let at = zumen_drawing::Position::Model { space: id, index };
                    out.push(ids[&format!("{at:?}")].clone());
                }
            }
        }
    }
    out
}

#[test]
fn display_order_matches_the_drawing_order() {
    let mut budget = Budget::default();
    let mut diag = Diagnostics::new();
    let cancel = CancelToken::new();
    let cx = &mut Cx::new(&mut budget, &mut diag, &cancel);
    let mut live = SxfLive::open(SAMPLE, cx).unwrap();
    for round in 0..2 {
        let drawing: Drawing2D = live.drawing(cx).unwrap();
        let order = live.display_order(cx).unwrap();
        let ids: Vec<String> = order
            .iter()
            .map(|&row| live.seen_of_row(row, cx).unwrap().unwrap().id)
            .collect();
        let expected = drawing_order(&drawing);
        assert!(!expected.is_empty());
        assert_eq!(ids, expected, "round {round}");
        // 2 回目は、消せる最初の要素を消した後（断られた削除は文書を変えない）
        let removed = order.iter().any(|&row| {
            let target = live.seen_of_row(row, cx).unwrap().unwrap();
            let changes = Changes {
                document: None,
                edits: vec![Edit::Remove { target }],
            };
            live.apply(&changes, cx).is_ok()
        });
        assert!(removed);
    }
}

#[test]
fn a_stopped_order_reports_the_stop() {
    let mut budget = Budget::default();
    let mut diag = Diagnostics::new();
    let cancel = CancelToken::new();
    let live = SxfLive::open(SAMPLE, &mut Cx::new(&mut budget, &mut diag, &cancel)).unwrap();
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
fn display_line_types_cover_the_defined_line_types_of_the_drawing() {
    let mut budget = Budget::default();
    let mut diag = Diagnostics::new();
    let cancel = CancelToken::new();
    let cx = &mut Cx::new(&mut budget, &mut diag, &cancel);
    let live = SxfLive::open(SAMPLE, cx).unwrap();
    let drawing = live.drawing(cx).unwrap();
    let line_types = live.display_line_types(cx).unwrap();
    let ids: BTreeSet<u32> = line_types.iter().map(|line_type| line_type.id.0).collect();
    assert_eq!(ids.len(), line_types.len());
    assert!((1..=15).all(|code| ids.contains(&code)));
    // 既定の線種と、文書が定義する利用者定義の線種は、同じ値で表にある
    let mut checked = 0;
    for line_type in &drawing.line_types {
        if line_type.id.0 <= 15 || line_type.source != zumen_base::Ref::default() {
            assert!(line_types.contains(line_type), "{line_type:?}");
            checked += 1;
        }
    }
    assert!(checked > 0);
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

/// 文字を持つ公開サンプル
const TEXT_SAMPLE: &[u8] = SAMPLE;

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
fn check_texts(live: &SxfLive, cx: &mut Cx<'_>) -> usize {
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
    let live = SxfLive::open(TEXT_SAMPLE, &mut Cx::new(&mut budget, &mut diag, &cancel)).unwrap();
    let mut budget = Budget::default();
    let mut diag = Diagnostics::new();
    let cx = &mut Cx::new(&mut budget, &mut diag, &cancel);
    let row = live
        .display_order(cx)
        .unwrap()
        .into_iter()
        .find(|&row| live.display_text(row, cx).unwrap().unwrap().0 != 0)
        .unwrap();
    for kind in [Kind::Work, Kind::DecodedBytes] {
        let mut budget = Budget::new(Limits::default().with(kind, Limit::Finite(0)));
        let mut diag = Diagnostics::new();
        let result = live.display_text(row, &mut Cx::new(&mut budget, &mut diag, &cancel));
        assert!(
            matches!(result, Err(zumen_sdk::Error::Stopped(_))),
            "{kind:?}"
        );
        assert_eq!(diag.iter().count(), 1, "{kind:?}");
    }
}

#[test]
fn display_text_matches_the_drawing() {
    let mut budget = Budget::default();
    let mut diag = Diagnostics::new();
    let cancel = CancelToken::new();
    let cx = &mut Cx::new(&mut budget, &mut diag, &cancel);
    let mut live = SxfLive::open(TEXT_SAMPLE, cx).unwrap();
    assert!(check_texts(&live, cx) > 0);
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

#[test]
fn display_retexted_replaces_only_the_content() {
    let mut budget = Budget::default();
    let mut diag = Diagnostics::new();
    let cancel = CancelToken::new();
    let cx = &mut Cx::new(&mut budget, &mut diag, &cancel);
    let mut live = SxfLive::open(TEXT_SAMPLE, cx).unwrap();
    let order = live.display_order(cx).unwrap();
    let kinds: Vec<u32> = order
        .iter()
        .map(|&row| live.display_text(row, cx).unwrap().unwrap().0)
        .collect();
    // 文字でない行は `None`
    for (&row, &kind) in order.iter().zip(&kinds) {
        if kind == 0 {
            assert!(live.display_retexted(row, "x", cx).unwrap().is_none());
        }
    }
    let mut changed = 0;
    for (&row, &kind) in order.iter().zip(&kinds) {
        if kind == 0 {
            continue;
        }
        // SXF は JIS X 0208 の文字も書ける
        let content = format!("改め {}", row.0);
        let target = live.seen_of_row(row, cx).unwrap().unwrap();
        let element = live.display_retexted(row, &content, cx).unwrap().unwrap();
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
            (kind, content)
        );
        changed += 1;
    }
    assert!(changed > 0);
    check_texts(&live, cx);
    // 保存して開き直しても同じ文字
    let saved = live.save(cx).unwrap();
    let reopened = SxfLive::open(&saved, cx).unwrap();
    check_texts(&reopened, cx);
    let texts = |live: &SxfLive, cx: &mut Cx<'_>| -> Vec<(u32, String)> {
        live.display_order(cx)
            .unwrap()
            .into_iter()
            .map(|row| live.display_text(row, cx).unwrap().unwrap())
            .collect()
    };
    assert_eq!(texts(&live, cx), texts(&reopened, cx));
}

#[test]
fn display_symbol_writes_each_element_of_the_definition() {
    let mut budget = Budget::default();
    let mut diag = Diagnostics::new();
    let cancel = CancelToken::new();
    let cx = &mut Cx::new(&mut budget, &mut diag, &cancel);
    let live = SxfLive::open(SAMPLE, cx).unwrap();
    let drawing = live.drawing(cx).unwrap();
    let count = live.display_symbol_count();
    assert_eq!(count as usize, drawing.symbols.len());
    let mut display = Display::with_rows(0);
    let mut first = 0;
    for (definition, symbol) in (0..count).zip(&drawing.symbols) {
        let rows = live
            .display_symbol(definition, &mut display, cx)
            .unwrap()
            .unwrap();
        // 1 要素 1 行
        assert_eq!(rows as usize, symbol.space.elements.len());
        first += rows;
        assert_eq!(display.canonical().len(), first as usize);
    }
    assert!(first > 0);
    // 挿入は、どれも定義の範囲を指す
    for index in 0..display.instance_ref.len() {
        if let Some(iref) = display.instance_ref.get(index) {
            assert!(iref[0] < count);
        }
    }
    // 範囲外は `None` で、表示に足さない
    assert!(
        live.display_symbol(count, &mut display, cx)
            .unwrap()
            .is_none()
    );
    assert_eq!(display.canonical().len(), first as usize);
}
