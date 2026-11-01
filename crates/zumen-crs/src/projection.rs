//! 経緯度と平面直角座標の換算（決定 0019）。
//!
//! 国土地理院の測量計算サイトの計算式（「平面直角座標への換算 計算式」「緯度、経度への換算 計算式」）を、
//! そのまま書く。順の換算は河瀬（2011）、逆の換算は地心緯度を介する河瀬（2021）の式による。楕円体は GRS80
//! （測量法施行令第 3 条）、X 軸上の縮尺係数は 0.9999（平成 14 年国土交通省告示第 9 号）。日本測地系の座標は、
//! 同じ式をベッセル楕円体で解く（[`Ellipsoid`]）。
//!
//! - 河瀬和重（2011）「Gauss-Krüger 投影における経緯度座標及び平面直角座標相互間の座標換算についてのより簡明な
//!   計算方法」、国土地理院時報 121、109–124
//! - 河瀬和重（2021）「地心緯度を介した正角緯度と地理（測地）緯度との関係について」、国土地理院時報 134、49–56

use core::f64::consts::PI;

use crate::{GeoPoint, MapPoint, Zone};

/// GRS80 の長半径（m）。測量法施行令第 3 条
pub const SEMI_MAJOR_AXIS: f64 = 6_378_137.0;
/// GRS80 の逆扁平率。測量法施行令第 3 条
pub const INVERSE_FLATTENING: f64 = 298.257_222_101;
/// X 軸上の縮尺係数。平成 14 年国土交通省告示第 9 号
pub const SCALE_ON_X_AXIS: f64 = 0.9999;

/// 経緯度と平面直角座標の換算に使う楕円体
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Ellipsoid {
    /// GRS80。JGD2000・JGD2011・JGD2024
    Grs80,
    /// ベッセル楕円体。日本測地系（Tokyo）。長半径 6 377 397.155 m、逆扁平率 299.152 813
    Bessel,
}

impl Ellipsoid {
    /// 長半径（m）と逆扁平率
    fn axes(self) -> (f64, f64) {
        match self {
            Ellipsoid::Grs80 => (SEMI_MAJOR_AXIS, INVERSE_FLATTENING),
            Ellipsoid::Bessel => (6_377_397.155, 299.152_813),
        }
    }
}

/// 平面直角座標に換算した点
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Projected {
    /// 平面直角座標（m）
    pub point: MapPoint,
    /// 子午線収差角（度）
    pub convergence: f64,
    /// 縮尺係数
    pub scale: f64,
}

/// 経緯度に換算した点
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Unprojected {
    /// 経緯度（度）
    pub point: GeoPoint,
    /// 子午線収差角（度）
    pub convergence: f64,
    /// 縮尺係数
    pub scale: f64,
}

/// 楕円体と系の原点から決まる値
struct Series {
    n: f64,
    alpha: [f64; 5],
    beta: [f64; 5],
    delta: [f64; 5],
    /// Ā
    a_bar: f64,
    /// S̄φ0
    s_phi0: f64,
    /// λ0（ラジアン）
    lambda0: f64,
    /// 長半径（m）
    semi_major: f64,
}

impl Series {
    fn new(zone: Zone, ellipsoid: Ellipsoid) -> Series {
        let (semi_major, inverse_flattening) = ellipsoid.axes();
        let n = 1.0 / (2.0 * inverse_flattening - 1.0);
        let [n2, n3, n4, n5] = [n * n, n * n * n, n * n * n * n, n * n * n * n * n];
        let a = [
            1.0 + n2 / 4.0 + n4 / 64.0,
            -(3.0 / 2.0) * (n - n3 / 8.0 - n5 / 64.0),
            (15.0 / 16.0) * (n2 - n4 / 4.0),
            -(35.0 / 48.0) * (n3 - (5.0 / 16.0) * n5),
            (315.0 / 512.0) * n4,
            -(693.0 / 1280.0) * n5,
        ];
        let alpha = [
            n / 2.0 - (2.0 / 3.0) * n2 + (5.0 / 16.0) * n3 + (41.0 / 180.0) * n4
                - (127.0 / 288.0) * n5,
            (13.0 / 48.0) * n2 - (3.0 / 5.0) * n3 + (557.0 / 1440.0) * n4 + (281.0 / 630.0) * n5,
            (61.0 / 240.0) * n3 - (103.0 / 140.0) * n4 + (15061.0 / 26880.0) * n5,
            (49561.0 / 161280.0) * n4 - (179.0 / 168.0) * n5,
            (34729.0 / 80640.0) * n5,
        ];
        let beta = [
            n / 2.0 - (2.0 / 3.0) * n2 + (37.0 / 96.0) * n3
                - (1.0 / 360.0) * n4
                - (81.0 / 512.0) * n5,
            (1.0 / 48.0) * n2 + (1.0 / 15.0) * n3 - (437.0 / 1440.0) * n4 + (46.0 / 105.0) * n5,
            (17.0 / 480.0) * n3 - (37.0 / 840.0) * n4 - (209.0 / 4480.0) * n5,
            (4397.0 / 161280.0) * n4 - (11.0 / 504.0) * n5,
            (4583.0 / 161280.0) * n5,
        ];
        let delta = [
            -(2.0 / 3.0) * n2 - (2.0 / 3.0) * n3 + (4.0 / 9.0) * n4 + (2.0 / 9.0) * n5,
            (1.0 / 3.0) * n2 - (4.0 / 15.0) * n3 - (23.0 / 45.0) * n4 + (68.0 / 45.0) * n5,
            (2.0 / 5.0) * n3 - (24.0 / 35.0) * n4 - (46.0 / 35.0) * n5,
            (83.0 / 126.0) * n4 - (80.0 / 63.0) * n5,
            (52.0 / 45.0) * n5,
        ];
        let (lat0, lon0) = zone.origin_minutes();
        let phi0 = minutes_to_radians(lat0);
        let factor = SCALE_ON_X_AXIS * semi_major / (1.0 + n);
        let a_bar = factor * a[0];
        let mut sum = 0.0;
        for (j, aj) in (1..=5).zip(&a[1..]) {
            sum += aj * libm::sin(2.0 * f64::from(j) * phi0);
        }
        Series {
            n,
            alpha,
            beta,
            delta,
            a_bar,
            s_phi0: a_bar * phi0 + factor * sum,
            lambda0: minutes_to_radians(lon0),
            semi_major,
        }
    }
}

fn minutes_to_radians(minutes: u32) -> f64 {
    f64::from(minutes) * (PI / 10_800.0)
}

fn degrees(radians: f64) -> f64 {
    radians * (180.0 / PI)
}

fn radians(degrees: f64) -> f64 {
    degrees * (PI / 180.0)
}

/// 経緯度を、系 `zone` の平面直角座標に換算する。値が有限でないか、緯度が ±90 度の外か、経度の差が
/// ±90 度に届くなら `None`
pub fn to_plane(zone: Zone, point: GeoPoint) -> Option<Projected> {
    to_plane_on(Ellipsoid::Grs80, zone, point)
}

/// [`to_plane`] を、楕円体 `ellipsoid` で解く
pub fn to_plane_on(ellipsoid: Ellipsoid, zone: Zone, point: GeoPoint) -> Option<Projected> {
    let s = Series::new(zone, ellipsoid);
    let phi = radians(point.latitude);
    let dlambda = radians(point.longitude) - s.lambda0;
    if !phi.is_finite() || phi.abs() >= PI / 2.0 || !dlambda.is_finite() {
        return None;
    }
    let (lambda_c, lambda_s) = (libm::cos(dlambda), libm::sin(dlambda));
    if lambda_c <= 0.0 {
        return None;
    }
    let k = 2.0 * s.n.sqrt() / (1.0 + s.n);
    let sin_phi = libm::sin(phi);
    let t = libm::sinh(libm::atanh(sin_phi) - k * libm::atanh(k * sin_phi));
    let t_bar = (1.0 + t * t).sqrt();
    let xi = libm::atan(t / lambda_c);
    let eta = libm::atanh(lambda_s / t_bar);
    let (mut x, mut y, mut sigma, mut tau) = (xi, eta, 1.0, 0.0);
    for (j, alpha) in (1..=5).zip(s.alpha) {
        let k = 2.0 * f64::from(j);
        let (sin, cos) = (libm::sin(k * xi), libm::cos(k * xi));
        let (sinh, cosh) = (libm::sinh(k * eta), libm::cosh(k * eta));
        x += alpha * sin * cosh;
        y += alpha * cos * sinh;
        sigma += k * alpha * cos * cosh;
        tau += k * alpha * sin * sinh;
    }
    let convergence = libm::atan(
        (tau * t_bar * lambda_c + sigma * t * lambda_s)
            / (sigma * t_bar * lambda_c - tau * t * lambda_s),
    );
    let parametric = (1.0 - s.n) / (1.0 + s.n) * libm::tan(phi);
    let scale = s.a_bar / s.semi_major
        * ((sigma * sigma + tau * tau) / (t * t + lambda_c * lambda_c)
            * (1.0 + parametric * parametric))
            .sqrt();
    let projected = Projected {
        point: MapPoint {
            northing: s.a_bar * x - s.s_phi0,
            easting: s.a_bar * y,
        },
        convergence: degrees(convergence),
        scale,
    };
    [
        projected.point.northing,
        projected.point.easting,
        projected.convergence,
        projected.scale,
    ]
    .iter()
    .all(|v| v.is_finite())
    .then_some(projected)
}

/// 系 `zone` の平面直角座標を、経緯度に換算する。値が有限でなければ `None`
pub fn to_geographic(zone: Zone, point: MapPoint) -> Option<Unprojected> {
    to_geographic_on(Ellipsoid::Grs80, zone, point)
}

/// [`to_geographic`] を、楕円体 `ellipsoid` で解く
pub fn to_geographic_on(ellipsoid: Ellipsoid, zone: Zone, point: MapPoint) -> Option<Unprojected> {
    let s = Series::new(zone, ellipsoid);
    let xi = (point.northing + s.s_phi0) / s.a_bar;
    let eta = point.easting / s.a_bar;
    if !xi.is_finite() || !eta.is_finite() {
        return None;
    }
    let (mut xi1, mut eta1, mut sigma, mut tau) = (xi, eta, 1.0, 0.0);
    for (j, beta) in (1..=5).zip(s.beta) {
        let k = 2.0 * f64::from(j);
        let (sin, cos) = (libm::sin(k * xi), libm::cos(k * xi));
        let (sinh, cosh) = (libm::sinh(k * eta), libm::cosh(k * eta));
        xi1 -= beta * sin * cosh;
        eta1 -= beta * cos * sinh;
        sigma -= k * beta * cos * cosh;
        tau += k * beta * sin * sinh;
    }
    let chi = libm::asin(libm::sin(xi1) / libm::cosh(eta1));
    let mut psi = chi;
    for (j, delta) in (1..=5).zip(s.delta) {
        psi += delta * libm::sin(2.0 * f64::from(j) * chi);
    }
    let ratio = (1.0 + s.n) / (1.0 - s.n);
    let t_psi = ratio * libm::tan(psi);
    let phi = libm::atan(ratio * t_psi);
    let lambda = s.lambda0 + libm::atan(libm::sinh(eta1) / libm::cos(xi1));
    let (tan_xi, tanh_eta) = (libm::tan(xi1), libm::tanh(eta1));
    let convergence =
        libm::atan((tau + sigma * tan_xi * tanh_eta) / (sigma - tau * tan_xi * tanh_eta));
    let (cos_xi, sinh_eta) = (libm::cos(xi1), libm::sinh(eta1));
    let scale = s.a_bar / s.semi_major
        * ((cos_xi * cos_xi + sinh_eta * sinh_eta) / (sigma * sigma + tau * tau)
            * (1.0 + t_psi * t_psi))
            .sqrt();
    let unprojected = Unprojected {
        point: GeoPoint {
            latitude: degrees(phi),
            longitude: degrees(lambda),
        },
        convergence: degrees(convergence),
        scale,
    };
    [
        unprojected.point.latitude,
        unprojected.point.longitude,
        unprojected.convergence,
        unprojected.scale,
    ]
    .iter()
    .all(|v| v.is_finite())
    .then_some(unprojected)
}
