use std::collections::BTreeSet;

use zumen_base::{Budget, CancelToken, Cx, Diagnostics, Kind, Limit, Limits};
use zumen_drawing::Drawing2D;
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
