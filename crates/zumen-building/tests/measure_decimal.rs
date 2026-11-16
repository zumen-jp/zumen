//! 二進の乗算による丸めと区別し、十進の中点・符号・桁・範囲を確かめる。
#![cfg(feature = "measure")]

use zumen_base::{Budget, CancelToken, Cx, Diagnostics, Kind, Limit, Limits};
use zumen_building::measure::{Rounding, RoundingMode, RoundingStage};
use zumen_geom3d::Failure;

#[allow(clippy::unwrap_used, reason = "丸められる既知の値を渡すテスト用の補助")]
fn rounded(value: f64, digits: i8, mode: RoundingMode) -> String {
    let (mut budget, mut diagnostics, cancel) = (
        Budget::new(Limits::default()),
        Diagnostics::new(),
        CancelToken::new(),
    );
    Rounding {
        mode,
        digits,
        stage: RoundingStage::Element,
    }
    .apply(value, &mut Cx::new(&mut budget, &mut diagnostics, &cancel))
    .unwrap()
    .to_string()
}

#[test]
fn decimal_ties_use_the_shortest_decimal_not_a_binary_scaled_product() {
    use RoundingMode::*;
    for (value, mode, expected) in [
        (1.005, HalfAway, "1.01"),
        (1.005, HalfEven, "1.00"),
        (2.675, HalfEven, "2.68"),
        (2.685, HalfEven, "2.68"),
        (-1.005, HalfAway, "-1.01"),
        (-1.005, HalfEven, "-1.00"),
        (-1.001, Floor, "-1.01"),
        (-1.001, Ceiling, "-1.00"),
        (-1.009, Truncate, "-1.00"),
        (1.001, Floor, "1.00"),
        (1.001, Ceiling, "1.01"),
        (1.009, Truncate, "1.00"),
        (0.0, HalfAway, "0.00"),
        (-0.0, Floor, "0.00"),
    ] {
        assert_eq!(rounded(value, 2, mode), expected, "{value:?} {mode:?}");
    }
    assert_eq!(rounded(1250.0, -2, HalfEven), "1200");
    assert_eq!(rounded(1350.0, -2, HalfEven), "1400");
    assert_eq!(rounded(-1250.0, -2, HalfAway), "-1300");
    assert_eq!(rounded(12.0, 0, HalfAway), "12");
}

#[test]
fn tiny_values_and_checked_decimal_sums_do_not_need_large_powers() {
    use RoundingMode::*;
    assert_eq!(
        rounded(f64::from_bits(1), 18, Ceiling),
        "0.000000000000000001"
    );
    assert_eq!(
        rounded(-f64::from_bits(1), 18, Floor),
        "-0.000000000000000001"
    );
    assert_eq!(
        rounded(f64::from_bits(1), 18, HalfAway),
        "0.000000000000000000"
    );
    let (mut budget, mut diagnostics, cancel) = (
        Budget::new(Limits::default()),
        Diagnostics::new(),
        CancelToken::new(),
    );
    let mut cx = Cx::new(&mut budget, &mut diagnostics, &cancel);
    let rule = Rounding {
        mode: HalfAway,
        digits: 1,
        stage: RoundingStage::Element,
    };
    let a = rule.apply(0.1, &mut cx).unwrap();
    let b = rule.apply(0.2, &mut cx).unwrap();
    assert_eq!(a.checked_add(b).unwrap().to_string(), "0.3");
    assert_eq!(a.coefficient(), 1);
    assert_eq!(a.digits(), 1);
    assert!(
        a.checked_add(Rounding { digits: 2, ..rule }.apply(0.1, &mut cx).unwrap())
            .is_none()
    );
    for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY, f64::MAX] {
        assert!(rule.apply(value, &mut cx).is_err());
    }
    for digits in [-128, -19, 19, 127] {
        assert!(Rounding { digits, ..rule }.apply(0.1, &mut cx).is_err());
    }
}

#[test]
fn decimal_allocation_and_work_stop_before_the_operation() {
    let rule = Rounding {
        mode: RoundingMode::HalfEven,
        digits: 2,
        stage: RoundingStage::Total,
    };
    for kind in [Kind::Work, Kind::DecodedBytes] {
        let (mut budget, mut diagnostics, cancel) = (
            Budget::new(Limits::default().with(kind, Limit::Finite(0))),
            Diagnostics::new(),
            CancelToken::new(),
        );
        assert!(matches!(
            rule.apply(2.675, &mut Cx::new(&mut budget, &mut diagnostics, &cancel)),
            Err(Failure::Stopped(_))
        ));
    }
    let (mut budget, mut diagnostics, cancel) = (
        Budget::new(Limits::default()),
        Diagnostics::new(),
        CancelToken::new(),
    );
    cancel.cancel();
    assert!(matches!(
        rule.apply(2.675, &mut Cx::new(&mut budget, &mut diagnostics, &cancel)),
        Err(Failure::Stopped(_))
    ));
}
