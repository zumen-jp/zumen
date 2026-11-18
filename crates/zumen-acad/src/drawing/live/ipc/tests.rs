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
