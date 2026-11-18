use std::collections::{BTreeMap, BTreeSet};

use zumen_base::{Budget, CancelToken, Cx, Diagnostics, Kind, Limit, Limits};
use zumen_drawing::{Changes, Drawing2D, Edit, Shape, id};
use zumen_live::display::{Display, NO_OWNER};

use super::super::AcadLive;

const SAMPLES: [&[u8]; 3] = [
    include_bytes!("../../../../../../samples/dxf/ezdxf-ascii-r12.dxf"),
    include_bytes!("../../../../../../samples/dxf/opendxfviewer-latin1.dxf"),
    include_bytes!("../../../../../../samples/dwg/zumen-layout-r2018.dwg"),
];

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
    let mut checked = 0;
    for (index, sample) in SAMPLES.into_iter().enumerate() {
        let live = AcadLive::open(sample, cx).unwrap();
        let drawing: Drawing2D = live.drawing(cx).unwrap();
        let pairs = live.display_layers(cx).unwrap();
        // 画層の組は図面モデルの画層と同じ順・同じ値
        let layers: Vec<_> = pairs.iter().map(|(_, layer)| layer.clone()).collect();
        assert_eq!(layers, drawing.layers, "sample {index}");
        // 表示の行のスタイルの画層は、どれも組の値にある
        let slots: BTreeSet<u32> = pairs.iter().map(|&(slot, _)| slot).collect();
        assert_eq!(slots.len(), pairs.len());
        let used = styled_layers(live.display());
        assert!(used.is_subset(&slots), "sample {index}: {used:?} {slots:?}");
        checked += used.len();
    }
    assert!(checked > 0);
}

#[test]
fn a_stopped_read_reports_the_stop() {
    let mut budget = Budget::default();
    let mut diag = Diagnostics::new();
    let cancel = CancelToken::new();
    let live = AcadLive::open(SAMPLES[0], &mut Cx::new(&mut budget, &mut diag, &cancel)).unwrap();
    for kind in [Kind::Work, Kind::DecodedBytes] {
        let mut budget = Budget::new(Limits::default().with(kind, Limit::Finite(0)));
        let mut diag = Diagnostics::new();
        let result = live.display_layers(&mut Cx::new(&mut budget, &mut diag, &cancel));
        assert!(
            matches!(result, Err(zumen_sdk::Error::Stopped(_))),
            "{kind:?}"
        );
        assert_eq!(diag.iter().count(), 1);
    }
}

/// モデル空間の 3 本の線（ハンドル 30・31・32）と、30 を最後に・31 を最初に描く SORTENTSTABLE
fn sorted_dxf() -> Vec<u8> {
    let mut text = String::from(
        "0\nSECTION\n2\nHEADER\n9\n$INSUNITS\n70\n4\n0\nENDSEC\n\
         0\nSECTION\n2\nTABLES\n0\nTABLE\n2\nBLOCK_RECORD\n\
         0\nBLOCK_RECORD\n5\n1F\n2\n*Model_Space\n0\nENDTAB\n0\nENDSEC\n\
         0\nSECTION\n2\nENTITIES\n",
    );
    for (index, handle) in ["30", "31", "32"].into_iter().enumerate() {
        text += &format!(
            "0\nLINE\n5\n{handle}\n330\n1F\n8\n0\n10\n{index}\n20\n0\n11\n{index}\n21\n1\n"
        );
    }
    text += "0\nENDSEC\n0\nSECTION\n2\nOBJECTS\n\
             0\nSORTENTSTABLE\n5\n50\n100\nAcDbSortentsTable\n330\n1F\n331\n30\n5\n99\n331\n31\n5\n2\n\
             0\nENDSEC\n0\nEOF\n";
    text.into_bytes()
}

#[test]
fn display_order_matches_the_drawing_order() {
    let mut budget = Budget::default();
    let mut diag = Diagnostics::new();
    let cancel = CancelToken::new();
    let cx = &mut Cx::new(&mut budget, &mut diag, &cancel);
    let sorted = sorted_dxf();
    let mut inputs: Vec<&[u8]> = SAMPLES.to_vec();
    inputs.push(&sorted);
    for (index, input) in inputs.into_iter().enumerate() {
        let live = AcadLive::open(input, cx).unwrap();
        let drawing = live.drawing(cx).unwrap();
        let order = live.display_order(cx).unwrap();
        // 行はすべてモデル空間の要素になり、行の番号が要素の添字になる
        let model = &drawing.spaces[0];
        assert_eq!(model.elements.len(), live.table().len(), "sample {index}");
        let rows: Vec<usize> = order.iter().map(|row| row.0 as usize).collect();
        assert_eq!(rows, model.order, "sample {index}");
    }
    let live = AcadLive::open(&sorted, cx).unwrap();
    let rows: Vec<u32> = live
        .display_order(cx)
        .unwrap()
        .iter()
        .map(|row| row.0)
        .collect();
    assert_eq!(rows, [1, 2, 0]);
}

#[test]
fn a_stopped_order_reports_the_stop() {
    let mut budget = Budget::default();
    let mut diag = Diagnostics::new();
    let cancel = CancelToken::new();
    let sorted = sorted_dxf();
    let live = AcadLive::open(&sorted, &mut Cx::new(&mut budget, &mut diag, &cancel)).unwrap();
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
fn display_line_types_match_the_drawing() {
    let mut budget = Budget::default();
    let mut diag = Diagnostics::new();
    let cancel = CancelToken::new();
    let cx = &mut Cx::new(&mut budget, &mut diag, &cancel);
    let mut checked = 0;
    for (index, sample) in SAMPLES.into_iter().enumerate() {
        let live = AcadLive::open(sample, cx).unwrap();
        let drawing = live.drawing(cx).unwrap();
        let line_types = live.display_line_types(cx).unwrap();
        assert_eq!(line_types, drawing.line_types, "sample {index}");
        checked += line_types.len();
    }
    assert!(checked > 0);
    for kind in [Kind::Work, Kind::DecodedBytes] {
        let live = AcadLive::open(SAMPLES[0], cx).unwrap();
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

/// 文字（TEXT・MTEXT）を持つ公開サンプル
const TEXT_SAMPLE: &[u8] = include_bytes!("../../../../../../samples/dxf/text_alignments.dxf");

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
fn check_texts(live: &AcadLive, cx: &mut Cx<'_>) -> usize {
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
    let live = AcadLive::open(TEXT_SAMPLE, &mut Cx::new(&mut budget, &mut diag, &cancel)).unwrap();
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
    for input in SAMPLES {
        let live = AcadLive::open(input, cx).unwrap();
        check_texts(&live, cx);
    }
    let live = AcadLive::open(TEXT_SAMPLE, cx).unwrap();
    assert!(check_texts(&live, cx) > 0);
    // 範囲の外の行は `None`
    let outside = zumen_live::RowId(live.table().len() as u32);
    assert_eq!(live.display_text(outside, cx).unwrap(), None);
}

#[test]
fn display_retexted_replaces_only_the_content() {
    let mut budget = Budget::default();
    let mut diag = Diagnostics::new();
    let cancel = CancelToken::new();
    let cx = &mut Cx::new(&mut budget, &mut diag, &cancel);
    let mut live = AcadLive::open(TEXT_SAMPLE, cx).unwrap();
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
        let content = format!("NEW {}", row.0);
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
    let reopened = AcadLive::open(&saved, cx).unwrap();
    check_texts(&reopened, cx);
    let texts = |live: &AcadLive, cx: &mut Cx<'_>| -> Vec<(u32, String)> {
        live.display_order(cx)
            .unwrap()
            .into_iter()
            .map(|row| live.display_text(row, cx).unwrap().unwrap())
            .collect()
    };
    assert_eq!(texts(&live, cx), texts(&reopened, cx));
}

/// 表示の挿入が指す定義の値（定義が見つからない印は除く）
fn instance_definitions(display: &Display) -> BTreeSet<u32> {
    (0..display.instance_owner.len())
        .filter(|&index| {
            display
                .instance_owner
                .get(index)
                .is_some_and(|&owner| owner != NO_OWNER)
        })
        .filter_map(|index| display.instance_ref.get(index).map(|iref| iref[0]))
        .filter(|&definition| definition != u32::MAX)
        .collect()
}

/// 線と円を持つ定義 B1、B1 を入れ子に挿す定義 B2、B2 を挿すモデル空間
fn block_dxf() -> Vec<u8> {
    let groups: &[(&str, &str)] = &[
        ("0", "SECTION"),
        ("2", "BLOCKS"),
        ("0", "BLOCK"),
        ("8", "0"),
        ("2", "B1"),
        ("70", "0"),
        ("10", "0"),
        ("20", "0"),
        ("3", "B1"),
        ("0", "LINE"),
        ("8", "0"),
        ("10", "0"),
        ("20", "0"),
        ("11", "1"),
        ("21", "1"),
        ("0", "CIRCLE"),
        ("8", "0"),
        ("62", "1"),
        ("10", "0"),
        ("20", "0"),
        ("40", "1"),
        ("0", "ENDBLK"),
        ("0", "BLOCK"),
        ("8", "0"),
        ("2", "B2"),
        ("70", "0"),
        ("10", "0"),
        ("20", "0"),
        ("3", "B2"),
        ("0", "INSERT"),
        ("8", "0"),
        ("2", "B1"),
        ("10", "5"),
        ("20", "5"),
        ("0", "ENDBLK"),
        ("0", "ENDSEC"),
        ("0", "SECTION"),
        ("2", "ENTITIES"),
        ("0", "INSERT"),
        ("8", "0"),
        ("2", "B2"),
        ("10", "0"),
        ("20", "0"),
        ("0", "ENDSEC"),
        ("0", "EOF"),
    ];
    groups
        .iter()
        .flat_map(|(code, value)| {
            format!(
                "{code}
{value}
"
            )
            .into_bytes()
        })
        .collect()
}

#[test]
fn display_symbol_writes_each_element_of_the_definition() {
    let mut budget = Budget::default();
    let mut diag = Diagnostics::new();
    let cancel = CancelToken::new();
    let cx = &mut Cx::new(&mut budget, &mut diag, &cancel);
    let mut written = 0;
    let blocks = block_dxf();
    for (index, sample) in SAMPLES
        .into_iter()
        .chain([TEXT_SAMPLE, &blocks])
        .enumerate()
    {
        let live = AcadLive::open(sample, cx).unwrap();
        let drawing: Drawing2D = live.drawing(cx).unwrap();
        let count = live.display_symbol_count();
        assert_eq!(count as usize, drawing.symbols.len(), "sample {index}");
        // 文書の表示の挿入は、どれも定義の範囲を指す
        let mut referenced = instance_definitions(live.display());
        let mut display = Display::with_rows(0);
        let mut first = 0;
        for (definition, symbol) in (0..count).zip(&drawing.symbols) {
            let rows = live
                .display_symbol(definition, &mut display, cx)
                .unwrap()
                .unwrap();
            // 1 要素 1 行
            assert_eq!(rows as usize, symbol.space.elements.len(), "sample {index}");
            first += rows;
            assert_eq!(display.canonical().len(), first as usize);
        }
        written += first;
        referenced.extend(instance_definitions(&display));
        assert!(
            referenced.iter().all(|&definition| definition < count),
            "sample {index}"
        );
        let styles: BTreeSet<u32> = live
            .display_layers(cx)
            .unwrap()
            .iter()
            .map(|&(slot, _)| slot)
            .collect();
        assert!(styled_layers(&display).is_subset(&styles), "sample {index}");
        // 範囲外は `None` で、表示に足さない
        assert!(
            live.display_symbol(count, &mut display, cx)
                .unwrap()
                .is_none()
        );
        assert_eq!(display.canonical().len(), first as usize);
    }
    // B1 の線と円、B2 の入れ子の挿入
    assert!(written >= 3);
}

#[test]
fn a_stopped_symbol_read_reports_the_stop() {
    let mut budget = Budget::default();
    let mut diag = Diagnostics::new();
    let cancel = CancelToken::new();
    let cx = &mut Cx::new(&mut budget, &mut diag, &cancel);
    let live = AcadLive::open(&block_dxf(), cx).unwrap();
    let definition = (0..live.display_symbol_count())
        .find(|&definition| {
            let mut display = Display::with_rows(0);
            live.display_symbol(definition, &mut display, cx).unwrap() > Some(0)
        })
        .unwrap();
    let mut budget = Budget::new(Limits::default().with(Kind::Work, Limit::Finite(0)));
    let mut diag = Diagnostics::new();
    let cx = &mut Cx::new(&mut budget, &mut diag, &cancel);
    let mut display = Display::with_rows(0);
    let result = live.display_symbol(definition, &mut display, cx);
    assert!(matches!(result, Err(zumen_sdk::Error::Stopped(_))));
    assert_eq!(diag.iter().count(), 1);
}
