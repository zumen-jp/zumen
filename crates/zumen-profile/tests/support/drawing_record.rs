//! 作図の結果、診断、全予算、停止位置のバイト列。
#![allow(clippy::unwrap_used, reason = "固定入力の契約記録")]
use super::input;
use zumen_base::{Budget, CancelToken, Cx, Diagnostics, Kind, Limit, Limits};
use zumen_building::{ElementRef, drawing as b};
use zumen_profile::{building::Error, building_drawing::DrawingProfile};

fn text(out: &mut Vec<u8>, value: &str) {
    out.extend_from_slice(&(value.len() as u64).to_le_bytes());
    out.extend_from_slice(value.as_bytes());
}
fn case<T: std::fmt::Debug>(out: &mut Vec<u8>, run: impl Fn(&mut Cx<'_>) -> Result<T, Error>) {
    let mut budget = Budget::default();
    run(&mut Cx::new(
        &mut budget,
        &mut Diagnostics::new(),
        &CancelToken::new(),
    ))
    .unwrap();
    let mut cases = vec![(Limits::default(), false), (Limits::default(), true)];
    for kind in Kind::ALL {
        for limit in [0, budget.used(kind).saturating_sub(1), budget.used(kind)] {
            cases.push((Limits::default().with(kind, Limit::Finite(limit)), false));
        }
    }
    for (limits, cancelled) in cases {
        let (mut budget, mut diagnostics, cancel) =
            (Budget::new(limits), Diagnostics::new(), CancelToken::new());
        if cancelled {
            cancel.cancel();
        }
        let result = run(&mut Cx::new(&mut budget, &mut diagnostics, &cancel));
        text(out, &format!("{result:?}"));
        for kind in Kind::ALL {
            out.extend_from_slice(&budget.used(kind).to_le_bytes());
        }
        text(out, &format!("{diagnostics:?}"));
    }
}
pub fn record() -> Vec<u8> {
    let mut out = b"zumen-profile drawing 1\n".to_vec();
    case(&mut out, |cx| {
        DrawingProfile::load(
            &[("base", input::BUILDING_DRAWING)],
            "zumen-building-drawing",
            "2026-11",
            cx,
        )
        .map(|p| p.defaults())
    });
    let p = input::profile();
    for (scale, translation) in [(0.5, [0.0; 3]), (1.0, [0.5, 0.25, 0.0])] {
        let mut b = input::building();
        input::mapped(&mut b, scale, translation);
        let view = input::with(|cx| p.default_plan(&b, input::origin(), cx)).unwrap();
        case(&mut out, |cx| p.cut(&b, view, 0.00001, 0.001, None, cx));
    }
    let mut b = input::building();
    input::hole(&mut b);
    input::grid(&mut b);
    let door = input::door(&mut b, "SINGLE_SWING_LEFT");
    let view = input::with(|cx| p.default_plan(&b, input::origin(), cx)).unwrap();
    case(&mut out, |cx| p.cut(&b, view, 0.00001, 0.001, None, cx));
    b.components.extension.remove(door);
    b.components.material.remove(ElementRef(0));
    case(&mut out, |cx| {
        p.cut(
            &b,
            view,
            0.00001,
            0.001,
            Some(b::Paper {
                denominator: 50.0,
                ..p.defaults().paper()
            }),
            cx,
        )
    });
    let view = b::View::Vertical {
        start: input::origin(),
        end: zumen_crs::ProjectPoint {
            x: 1.0,
            ..input::origin()
        },
        side: b::Side::Right,
        depth: 4.0,
        section: true,
    };
    case(&mut out, |cx| p.cut(&b, view, 0.00001, 0.001, None, cx));
    out
}
