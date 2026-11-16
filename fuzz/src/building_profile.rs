//! プロファイルの JSON と、属性・配置・区分を変えた建物の照合。
#[path = "../../crates/zumen-profile/tests/support/building.rs"]
mod input;
use zumen_base::{Budget, CancelToken, Cx, Diagnostics, Kind, Limit, Limits};
use zumen_building::{ElementRef, Literal, Local, Placement, Transform};
use zumen_profile::building::{AreaGroup, AreaGroupKind, BuildingProfile, Error};

pub fn probe(data: &[u8], cx: &mut Cx<'_>) -> Result<(), zumen_sdk::Error> {
    BuildingProfile::load(&[("fuzz", data)], "fuzz", "1", cx)
        .map(|_| ())
        .map_err(|error| match error {
            Error::Stopped(stop) | Error::Profile(zumen_profile::Error::Stop(stop, _)) => {
                zumen_sdk::Error::Stopped(stop)
            }
            _ => zumen_sdk::Error::Invalid,
        })
}

pub fn exercise(data: &[u8]) {
    let byte = |i| data.get(i).copied().unwrap_or(0u8);
    let number = f64::from_bits(u64::from_le_bytes(std::array::from_fn(|i| byte(i + 1))));
    let p = input::profile(byte(0) & 1 != 0);
    let a = input::building();
    let mut b = input::building();
    let mut value = match byte(9) % 5 {
        0 => Literal::Real(number),
        1 => Literal::Logical(None),
        2 => Literal::Element(ElementRef(u32::from(byte(10)))),
        3 => Literal::Text(String::from_utf8_lossy(&data[..data.len().min(4096)]).into_owned()),
        _ => Literal::Boolean(false),
    };
    for _ in 0..byte(11) % 32 {
        value = Literal::List(vec![value]);
    }
    input::properties(&mut b, ElementRef(1), &[("use", Some(value))]);
    b.components.placement.insert(
        ElementRef(0),
        Placement {
            relative_to: (byte(12) & 1 != 0).then_some(ElementRef(u32::from(byte(13)))),
            local: Local::Transform(Transform::translation(number, 0.0, 0.0)),
        },
    );
    if byte(14) & 1 != 0 {
        b.document.units.clear();
    }
    if byte(14) & 2 != 0 {
        b.components.class.remove(ElementRef(1));
    }
    if byte(14) & 4 != 0 {
        b.components.representation.remove(ElementRef(2));
    }
    let groups = [AreaGroup {
        kind: match byte(15) % 3 {
            0 => AreaGroupKind::Storey,
            1 => AreaGroupKind::Use,
            _ => AreaGroupKind::Building,
        },
        key: "1",
        elements: &[ElementRef(u32::from(byte(16) % 6))],
        origin: zumen_crs::ProjectPoint {
            z: number,
            ..input::origin()
        },
    }];
    let limits = Limits::default()
        .with(Kind::DecodedBytes, Limit::Finite(8 << 20))
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
        let result = if byte(17) & 1 == 0 {
            format!("{:?}", p.check(&[&a, &b], &mut cx))
        } else {
            format!("{:?}", p.areas(&b, &groups, input::tolerance(), &mut cx))
        };
        assert_eq!(budget.used(Kind::Depth), 0);
        (
            result,
            format!("{diagnostics:?}"),
            Kind::ALL.map(|kind| budget.used(kind)),
        )
    };
    assert_eq!(run(limits, false), run(limits, false));
    for kind in [
        Kind::Work,
        Kind::DecodedBytes,
        Kind::Elements,
        Kind::Segments,
        Kind::Depth,
    ] {
        let limits = limits.with(kind, Limit::Finite(u64::from(byte(18))));
        assert_eq!(run(limits, false), run(limits, false));
    }
    assert_eq!(run(limits, true), run(limits, true));
}

#[cfg(test)]
mod tests {
    #[test]
    fn synthetic_mutations_keep_findings_measurements_and_stops_deterministic() {
        for byte in 0..=255u8 {
            super::exercise(&[byte; 32]);
        }
        for data in [
            b"".as_slice(),
            b"{",
            b"{\"format\":\"0.1\",\"id\":\"fuzz\",\"edition\":\"1\",\"revision\":1}",
        ] {
            zumen_sdk::testkit::survives_with(data, super::probe).unwrap();
        }
    }
}
