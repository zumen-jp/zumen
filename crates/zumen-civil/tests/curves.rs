//! 3 次放物線・ウィーン曲線と、縦断の円弧・クロソイドを、曲線の式から細かく積んだ値と比べる（#467）。

use zumen_base::{Budget, CancelToken, Cx, Diagnostics};
use zumen_civil::{Alignment, Horizontal, HorizontalKind, Vertical, VerticalKind};
use zumen_crs::MapPoint;
use zumen_geom2d::{Ramp, along};

const R: f64 = 200.0;

#[cfg(test)]
/// 0.1 µm まで合えばよい
fn close(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-7
}

#[cfg(test)]
/// 台形の公式で、`f` を 0 から `end` まで 20 万の刻みで積む
fn integrate(end: f64, f: impl Fn(f64) -> f64) -> f64 {
    const N: usize = 200_000;
    let h = end / N as f64;
    (0..N)
        .map(|i| (f(h * i as f64) + f(h * (i + 1) as f64)) * h / 2.0)
        .sum()
}

#[cfg(test)]
/// 3 次放物線 y = x³ / 6RX の、x = 0 から `x` までの曲線の長さ
fn cubic_arc(x: f64, end: f64) -> f64 {
    integrate(x, |u| libm::hypot(1.0, u * u / (2.0 * R * end)))
}

#[test]
fn the_cubic_parabola_lies_on_its_formula_in_both_directions() {
    let end = 40.0;
    let length = cubic_arc(end, end);
    let curve = Some(Ramp::CubicParabola);
    for x in [10.0, 25.0, end] {
        let s = cubic_arc(x, end);
        let ([n, e], direction) = along([0.0, 0.0], 0.0, [0.0, 1.0 / R], length, curve, s);
        assert!(close(n, x), "{x} {n}");
        assert!(close(e, x * x * x / (6.0 * R * end)), "{x} {e}");
        assert!(
            close(direction, libm::atan(x * x / (2.0 * R * end))),
            "{direction}"
        );
    }
    // 円弧の側から直線へたどると、基本の形の原点に戻る。逆にたどるので、初めの曲率は左へ曲がる向きになる
    let tip = [end, end * end / (6.0 * R)];
    let back = libm::atan(end / (2.0 * R)) + core::f64::consts::PI;
    let ([n, e], direction) = along(tip, back, [-1.0 / R, 0.0], length, curve, length);
    assert!(close(n, 0.0) && close(e, 0.0), "{n} {e}");
    assert!(close(direction, core::f64::consts::PI), "{direction}");
    let s = length - cubic_arc(25.0, end);
    let ([n, e], _) = along(tip, back, [-1.0 / R, 0.0], length, curve, s);
    assert!(close(n, 25.0) && close(e, 25.0 * 25.0 * 25.0 / (6.0 * R * end)));
}

#[test]
fn the_viennese_bend_follows_its_seventh_order_curvature() {
    let length = 60.0;
    // 曲率 (35t⁴ − 84t⁵ + 70t⁶ − 20t⁷) / R を積んだ向き
    let angle = |u: f64| {
        let t = u / length;
        let t4 = t * t * t * t;
        length * t4 * t * (7.0 - 14.0 * t + 10.0 * t * t - 2.5 * t * t * t) / R
    };
    // 曲率の式を細かく積んでも、同じ向きになる
    let curvature = |v: f64| {
        let t = v / length;
        let t4 = t * t * t * t;
        (35.0 * t4 - 84.0 * t4 * t + 70.0 * t4 * t * t - 20.0 * t4 * t * t * t) / R
    };
    assert!((integrate(length, curvature) - angle(length)).abs() < 1e-9);
    for s in [20.0, 45.0, length] {
        let ([n, e], direction) = along(
            [0.0, 0.0],
            0.0,
            [0.0, 1.0 / R],
            length,
            Some(Ramp::Viennese),
            s,
        );
        assert!((direction - angle(s)).abs() < 1e-9, "{s} {direction}");
        // 点は、向きの式を細かく積んだ値と比べる
        let n_expected = integrate(s, |u| libm::cos(angle(u)));
        let e_expected = integrate(s, |u| libm::sin(angle(u)));
        assert!((n - n_expected).abs() < 1e-6, "{s} {n} {n_expected}");
        assert!((e - e_expected).abs() < 1e-6, "{s} {e} {e_expected}");
    }
}

#[cfg(test)]
fn with_cx<T>(f: impl FnOnce(&mut Cx<'_>) -> T) -> (T, Diagnostics) {
    let mut diag = Diagnostics::new();
    let mut budget = Budget::default();
    let cancel = CancelToken::new();
    let mut cx = Cx::new(&mut budget, &mut diag, &cancel);
    let out = f(&mut cx);
    (out, diag)
}

#[cfg(test)]
/// 北へ 100 m の直線に、2 % の勾配から始まる縦断の区間を 1 つ置く
fn profile(kind: VerticalKind) -> Alignment {
    Alignment {
        id: "a".into(),
        horizontal: vec![Horizontal {
            start: MapPoint {
                northing: 0.0,
                easting: 0.0,
            },
            direction: 0.0,
            length: 100.0,
            source: Default::default(),
            kind: HorizontalKind::Line,
        }],
        vertical: vec![Vertical {
            start_distance: 0.0,
            start_height: 10.0,
            start_gradient: 0.02,
            length: 80.0,
            source: Default::default(),
            kind,
        }],
        ..Alignment::default()
    }
}

#[cfg(test)]
fn height(alignment: &Alignment, distance: f64) -> (f64, f64) {
    let (placed, diag) = with_cx(|cx| zumen_civil::place(alignment, distance, cx).unwrap());
    assert!(diag.is_empty(), "{diag:?}");
    (placed.height.unwrap(), placed.gradient.unwrap())
}

#[cfg(test)]
/// 初めの勾配 2 % で高さ 10 m の点を通る、半径 `radius`（上に凸を正）の円の、水平に `u` の所の高さと勾配
fn circle(radius: f64, u: f64) -> (f64, f64) {
    let theta = libm::atan(0.02);
    let (cu, ch) = (radius * libm::sin(theta), 10.0 - radius * libm::cos(theta));
    let dy = libm::sqrt(radius * radius - (u - cu) * (u - cu)).copysign(radius);
    (ch + dy, -(u - cu) / dy)
}

#[test]
fn the_vertical_arc_lies_on_its_circle() {
    for radius in [1500.0, -1500.0] {
        let alignment = profile(VerticalKind::CircularArc { radius });
        for u in [0.0, 30.0, 80.0] {
            let (h, g) = height(&alignment, u);
            let (eh, eg) = circle(radius, u);
            assert!(
                (h - eh).abs() < 1e-9 && (g - eg).abs() < 1e-12,
                "{radius} {u}"
            );
        }
        // 2 次放物線とも、1 cm の内で同じ高さになる
        let (h, _) = height(&alignment, 80.0);
        let parabola = 10.0 + 0.02 * 80.0 - 80.0 * 80.0 / (2.0 * radius);
        assert!((h - parabola).abs() < 1e-2, "{h} {parabola}");
    }
}

#[test]
fn the_vertical_clothoid_bends_like_its_formula() {
    // 半径が変わらなければ円弧に、半径がなければ一定の勾配になる
    let constant = profile(VerticalKind::Clothoid {
        start_radius: Some(1500.0),
        end_radius: Some(1500.0),
    });
    let straight = profile(VerticalKind::Clothoid {
        start_radius: None,
        end_radius: None,
    });
    for u in [20.0, 80.0] {
        let (h, g) = height(&constant, u);
        let (eh, eg) = circle(1500.0, u);
        assert!(close(h, eh) && (g - eg).abs() < 1e-9, "{u} {h} {eh}");
        let (h, g) = height(&straight, u);
        assert!(close(h, 10.0 + 0.02 * u) && close(g, 0.02), "{u} {h}");
    }
    // 上に凸へ移るクロソイド。曲線の長さ S を、水平の長さが 80 m になるように二分法で解き、
    // 勾配の角 θ(s) = θ₀ − s² / 2RS を細かく積んだ値と比べる
    let theta0 = libm::atan(0.02);
    let angle = |s: f64, total: f64| theta0 - s * s / (2.0 * R * total);
    let (mut low, mut high) = (80.0, 90.0);
    for _ in 0..60 {
        let total = (low + high) / 2.0;
        if integrate(total, |s| libm::cos(angle(s, total))) < 80.0 {
            low = total;
        } else {
            high = total;
        }
    }
    let total = (low + high) / 2.0;
    let rising = profile(VerticalKind::Clothoid {
        start_radius: None,
        end_radius: Some(R),
    });
    let (h, g) = height(&rising, 80.0);
    let eh = 10.0 + integrate(total, |s| libm::sin(angle(s, total)));
    assert!((h - eh).abs() < 1e-6, "{h} {eh}");
    assert!((g - libm::tan(angle(total, total))).abs() < 1e-9, "{g}");
}
