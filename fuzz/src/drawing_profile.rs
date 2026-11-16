//! 作図の語彙と、変異した建具・材料・切断面を、小さい予算でも確かめる。
#[path = "../../crates/zumen-profile/tests/support/drawing.rs"]
mod input;
use zumen_base::{Budget, CancelToken, Cx, Diagnostics, Kind, Limit, Limits};
use zumen_building::{ElementRef, Literal, Local, Placement, Transform};
use zumen_profile::{building::Error, building_drawing::DrawingProfile};

pub fn probe(data: &[u8], cx: &mut Cx<'_>) -> Result<(), zumen_sdk::Error> {
    DrawingProfile::load(
        &[("base", input::BUILDING_DRAWING), ("fuzz", data)],
        "fuzz",
        "1",
        cx,
    )
    .map(|_| ())
    .map_err(|e| match e {
        Error::Stopped(s) | Error::Profile(zumen_profile::Error::Stop(s, _)) => {
            zumen_sdk::Error::Stopped(s)
        }
        _ => zumen_sdk::Error::Invalid,
    })
}
pub fn exercise(data: &[u8]) {
    let byte = |i| data.get(i).copied().unwrap_or(0u8);
    let number = f64::from_bits(u64::from_le_bytes(std::array::from_fn(|i| byte(i + 1))));
    let profile = input::profile();
    let mut building = input::building();
    if byte(0) & 1 != 0 {
        input::hole(&mut building);
    }
    if byte(0) & 2 != 0 {
        input::grid(&mut building);
    }
    let at = input::door(
        &mut building,
        [
            "SINGLE_SWING_LEFT",
            "SINGLE_SWING_RIGHT",
            "SLIDING_TO_LEFT",
            "NOTDEFINED",
        ][usize::from(byte(9) % 4)],
    );
    if let Some(mut extension) = building.components.extension.remove(at) {
        for value in &mut extension.values {
            if value.name == "OverallWidth" {
                value.value.value = Literal::Real(number);
            }
        }
        building.components.extension.insert(at, extension);
    }
    if byte(10) & 1 != 0 {
        building.components.material.remove(ElementRef(0));
    }
    if byte(10) & 2 != 0 {
        building.document.units.clear();
    }
    building.components.placement.insert(
        at,
        Placement {
            relative_to: (byte(11) & 1 != 0).then_some(ElementRef(u32::from(byte(12)))),
            local: Local::Transform(Transform::translation(number, 0.0, 0.0)),
        },
    );
    let limits = Limits::default()
        .with(Kind::DecodedBytes, Limit::Finite(16 << 20))
        .with(Kind::Work, Limit::Finite(1 << 24))
        .with(Kind::Segments, Limit::Finite(8192))
        .with(Kind::Depth, Limit::Finite(32));
    let run = |limits, cancelled| {
        let (mut budget, mut diagnostics, cancel) =
            (Budget::new(limits), Diagnostics::new(), CancelToken::new());
        if cancelled {
            cancel.cancel();
        }
        let mut cx = Cx::new(&mut budget, &mut diagnostics, &cancel);
        let result = profile
            .default_plan(&building, input::origin(), &mut cx)
            .and_then(|view| {
                profile.cut(
                    &building,
                    view,
                    if byte(13) & 1 != 0 { number } else { 0.00001 },
                    0.001,
                    None,
                    &mut cx,
                )
            });
        assert_eq!(budget.used(Kind::Depth), 0);
        (
            format!("{result:?}"),
            format!("{diagnostics:?}"),
            Kind::ALL.map(|kind| budget.used(kind)),
        )
    };
    assert_eq!(run(limits, false), run(limits, false));
    for kind in [Kind::Work, Kind::DecodedBytes, Kind::Segments, Kind::Depth] {
        let limits = limits.with(kind, Limit::Finite(u64::from(byte(14))));
        assert_eq!(run(limits, false), run(limits, false));
    }
    assert_eq!(run(limits, true), run(limits, true));
}

#[cfg(test)]
mod tests {
    #[test]
    fn synthetic_mutations_keep_drawing_and_budget_deterministic() {
        for byte in 0..=255u8 {
            super::exercise(&[byte; 32]);
        }
        for input in [b"".as_slice(), b"{", br#"{"format":"0.1","id":"fuzz","edition":"1","revision":1,"extends":{"id":"zumen-building-drawing","edition":"2026-11"}}"#] { zumen_sdk::testkit::survives_with(input, super::probe).unwrap(); }
    }
}
