//! チェック・面積・数値のビット・診断・全予算・停止を同じバイト列にする。
#![allow(clippy::unwrap_used, reason = "試験と照合の固定入力")]
use super::input;
use zumen_base::{Budget, CancelToken, Cx, Diagnostics, Kind, Limit, Limits};
use zumen_building::{ElementRef, Literal};
use zumen_profile::building::{AreaGroup, AreaGroupKind, BuildingProfile, Error};

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
    let mut out = b"zumen-profile building 1\n".to_vec();
    case(&mut out, |cx| {
        BuildingProfile::load(
            &[("base", input::BASE), ("child", input::CHILD)],
            "test.permit",
            "1",
            cx,
        )
        .map(|p| (p.chain().to_vec(), p.hash()))
    });
    let a = input::building();
    let mut b = input::building();
    b.document.header.schemas = vec!["IFC4".into()];
    b.document.units.clear();
    b.components.representation.remove(ElementRef(1));
    input::properties(
        &mut b,
        ElementRef(1),
        &[
            ("use", Some(Literal::List(vec![Literal::Logical(None)]))),
            ("ratio", None),
        ],
    );
    for configured in [false, true] {
        let p = input::profile(configured);
        case(&mut out, |cx| p.check(&[&a, &a], cx));
        case(&mut out, |cx| p.check(&[&a, &b], cx));
        case(&mut out, |cx| p.check(&[], cx));
        let groups = [
            AreaGroup {
                kind: AreaGroupKind::Storey,
                key: "1",
                elements: &[ElementRef(1), ElementRef(2)],
                origin: input::origin(),
            },
            AreaGroup {
                kind: AreaGroupKind::Use,
                key: "1/a",
                elements: &[ElementRef(1)],
                origin: input::origin(),
            },
            AreaGroup {
                kind: AreaGroupKind::Building,
                key: "building",
                elements: &[ElementRef(1), ElementRef(2)],
                origin: input::origin(),
            },
        ];
        case(&mut out, |cx| {
            p.areas(&a, &groups, input::tolerance(), cx).map(|r| {
                // Debug に加え、丸め前の数と許容差のビットも照らす。
                let bits: Vec<_> = r
                    .groups
                    .iter()
                    .flat_map(|g| {
                        let m = &g.measurement;
                        m.raw_total
                            .into_iter()
                            .chain(m.measurements.iter().filter_map(|v| v.raw))
                            .chain([m.tolerance.length, m.tolerance.chord])
                            .map(f64::to_bits)
                    })
                    .collect();
                (r, bits)
            })
        });
    }
    out
}
