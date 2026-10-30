//! 2D の幾何（L3、決定 0018）。曲線を弦の誤差で折れ線にし、線種の列を線と間に展開する（決定 0172）。
//!
//! 点は `[x, y]`、角度は度で反時計回り。三角関数は、どの環境でも同じ値になる `libm` で求める（決定 0007）。
//! 1 つの曲線で作る点の数は [`MAX_STEPS`] で抑える。使う側は、作った点を予算の `segments` で数える。

use std::f64::consts::PI;

/// 1 つの曲線で作る区間の数の上限
pub const MAX_STEPS: usize = 1 << 16;

/// 点
pub type Xy = [f64; 2];

fn radians(degrees: f64) -> f64 {
    degrees * (PI / 180.0)
}

fn distance(a: Xy, b: Xy) -> f64 {
    libm::hypot(b[0] - a[0], b[1] - a[1])
}

/// 円弧の始まりと終わりの角度（度）から、たどる角度（度）を求める。反時計回りなら正。始まりと終わりが
/// 同じ向きか、差が 360 以上なら、一周とする
pub fn sweep(start: f64, end: f64, counterclockwise: bool) -> f64 {
    if !(start.is_finite() && end.is_finite()) {
        return 0.0;
    }
    if (end - start).abs() >= 360.0 {
        return if counterclockwise { 360.0 } else { -360.0 };
    }
    let turn = if counterclockwise {
        (end - start).rem_euclid(360.0)
    } else {
        (start - end).rem_euclid(360.0)
    };
    let turn = if turn == 0.0 { 360.0 } else { turn };
    if counterclockwise { turn } else { -turn }
}

/// 半径 `radius` の円を、弦の誤差 `tolerance` 以下で `sweep` ラジアンだけたどる区間の数
fn arc_steps(radius: f64, sweep: f64, tolerance: f64) -> usize {
    let sweep = sweep.abs();
    if !(radius.is_finite() && sweep.is_finite() && radius > 0.0 && sweep > 0.0) {
        return 1;
    }
    // 弦の誤差は r(1 − cos(δ/2))。誤差が半径以上なら、4 分の 1 周ずつにする
    let step = if tolerance.is_finite() && tolerance > 0.0 && tolerance < radius {
        2.0 * libm::acos(1.0 - tolerance / radius)
    } else {
        PI / 2.0
    };
    let step = step.clamp(f64::MIN_POSITIVE, PI / 2.0);
    let steps = libm::ceil(sweep / step);
    if steps.is_finite() && steps >= 1.0 {
        // 上限の内側に収めてから、整数にする
        (steps.min(MAX_STEPS as f64)) as usize
    } else {
        MAX_STEPS
    }
}

/// 円弧を折れ線にし、始点から終点までの点を `out` に足す。`start` と `sweep` は度
pub fn arc(center: Xy, radius: f64, start: f64, sweep: f64, tolerance: f64, out: &mut Vec<Xy>) {
    let steps = arc_steps(radius, radians(sweep), tolerance);
    let a0 = radians(start);
    let da = radians(sweep) / steps as f64;
    out.reserve(steps + 1);
    for i in 0..=steps {
        let a = a0 + da * i as f64;
        out.push([
            center[0] + radius * libm::cos(a),
            center[1] + radius * libm::sin(a),
        ]);
    }
}

/// 楕円弧を折れ線にし、点を `out` に足す。`major` は長軸の端の点、`ratio` は短軸の長軸に対する比。
/// `start` と `sweep` は、楕円を円に引き延ばしたときの角度（度）
pub fn ellipse(
    center: Xy,
    major: Xy,
    ratio: f64,
    start: f64,
    sweep: f64,
    tolerance: f64,
    out: &mut Vec<Xy>,
) {
    let u = [major[0] - center[0], major[1] - center[1]];
    let v = [-u[1] * ratio, u[0] * ratio];
    let a = libm::hypot(u[0], u[1]);
    let b = a * ratio.abs();
    // いちばん曲がる所の曲率半径 b²/a で区間を決める
    let radius = if a > 0.0 { b * b / a } else { 0.0 };
    let steps = arc_steps(radius, radians(sweep), tolerance);
    let t0 = radians(start);
    let dt = radians(sweep) / steps as f64;
    out.reserve(steps + 1);
    for i in 0..=steps {
        let t = t0 + dt * i as f64;
        let (c, s) = (libm::cos(t), libm::sin(t));
        out.push([
            center[0] + u[0] * c + v[0] * s,
            center[1] + u[1] * c + v[1] * s,
        ]);
    }
}

/// ふくらみ `bulge`（中心角の 1/4 の正接、正なら反時計回り）で結んだ `from` から `to` までを折れ線にし、
/// `from` の次から `to` までの点を `out` に足す
pub fn bulge(from: Xy, to: Xy, bulge: f64, tolerance: f64, out: &mut Vec<Xy>) {
    let chord = distance(from, to);
    if bulge == 0.0 || !bulge.is_finite() || chord == 0.0 || !chord.is_finite() {
        out.push(to);
        return;
    }
    let angle = 4.0 * libm::atan(bulge);
    let half = angle / 2.0;
    let radius = (chord / 2.0 / libm::sin(half)).abs();
    // 中心は、弦の中点から左の法線の向きへ (d/2)/tan(θ/2)
    let h = chord / 2.0 / libm::tan(half);
    let [dx, dy] = [(to[0] - from[0]) / chord, (to[1] - from[1]) / chord];
    let center = [
        (from[0] + to[0]) / 2.0 - dy * h,
        (from[1] + to[1]) / 2.0 + dx * h,
    ];
    let start = libm::atan2(from[1] - center[1], from[0] - center[0]);
    let steps = arc_steps(radius, angle, tolerance);
    let da = angle / steps as f64;
    out.reserve(steps);
    for i in 1..steps {
        let a = start + da * i as f64;
        out.push([
            center[0] + radius * libm::cos(a),
            center[1] + radius * libm::sin(a),
        ]);
    }
    out.push(to);
}

/// ふくらみを持つ頂点の列を折れ線にする。`closed` なら終点から始点へも結び、始点を最後に繰り返す
pub fn polyline(vertices: &[(Xy, f64)], closed: bool, tolerance: f64) -> Vec<Xy> {
    let mut out = Vec::with_capacity(vertices.len() + usize::from(closed));
    let Some(&(first, _)) = vertices.first() else {
        return out;
    };
    out.push(first);
    for pair in vertices.windows(2) {
        if let [(from, b), (to, _)] = pair {
            bulge(*from, *to, *b, tolerance, &mut out);
        }
    }
    if closed
        && vertices.len() > 1
        && let Some(&(last, b)) = vertices.last()
    {
        bulge(last, first, b, tolerance, &mut out);
    }
    out
}

/// 有理か非有理の B スプラインの、パラメーター `t` の点。de Boor の方法で求める
fn de_boor(degree: usize, knots: &[f64], control: &[[f64; 3]], t: f64) -> Option<Xy> {
    let n = control.len();
    // t を含む節点の区間 [k_s, k_{s+1}) を、定義域の中で探す
    let mut span = degree;
    while span + 1 < n && *knots.get(span + 1)? <= t {
        span += 1;
    }
    let mut d: Vec<[f64; 3]> = (0..=degree)
        .map(|j| control.get(j + span - degree).copied())
        .collect::<Option<_>>()?;
    for r in 1..=degree {
        for j in (r..=degree).rev() {
            let i = j + span - degree;
            let left = *knots.get(i)?;
            let right = *knots.get(i + degree + 1 - r)?;
            let alpha = if right > left {
                (t - left) / (right - left)
            } else {
                0.0
            };
            let (prev, cur) = (*d.get(j - 1)?, *d.get(j)?);
            *d.get_mut(j)? = [
                (1.0 - alpha) * prev[0] + alpha * cur[0],
                (1.0 - alpha) * prev[1] + alpha * cur[1],
                (1.0 - alpha) * prev[2] + alpha * cur[2],
            ];
        }
    }
    let [x, y, w] = *d.get(degree)?;
    (w != 0.0 && w.is_finite()).then(|| [x / w, y / w])
}

/// 点 `p` と、`a` から `b` への線分の距離
fn deviation(p: Xy, a: Xy, b: Xy) -> f64 {
    let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
    let length = libm::hypot(dx, dy);
    if length == 0.0 {
        return distance(p, a);
    }
    ((p[0] - a[0]) * dy - (p[1] - a[1]) * dx).abs() / length
}

/// 制御点と節点で決まるスプラインを折れ線にする。定義が合わなければ `None`
pub fn spline(
    degree: u32,
    knots: &[f64],
    control: &[Xy],
    weights: &[f64],
    tolerance: f64,
) -> Option<Vec<Xy>> {
    let degree = usize::try_from(degree).ok()?;
    let n = control.len();
    if degree == 0
        || n <= degree
        || knots.len() != n + degree + 1
        || !(weights.is_empty() || weights.len() == n)
        || knots
            .windows(2)
            .any(|w| matches!(w, [a, b] if a.partial_cmp(b).is_none_or(|o| o.is_gt())))
        || knots.iter().any(|k| !k.is_finite())
    {
        return None;
    }
    let homogeneous: Vec<[f64; 3]> = control
        .iter()
        .enumerate()
        .map(|(i, p)| {
            let w = weights.get(i).copied().unwrap_or(1.0);
            [p[0] * w, p[1] * w, w]
        })
        .collect();
    let (lo, hi) = (*knots.get(degree)?, *knots.get(n)?);
    if lo.partial_cmp(&hi) != Some(std::cmp::Ordering::Less) {
        return None;
    }
    let eval = |t: f64| de_boor(degree, knots, &homogeneous, t.min(hi));
    let mut out = vec![eval(lo)?];
    // 節点の区間ごとに、中点の弦からのずれが許容差に入るまで 2 つに分ける
    let mut stack: Vec<(f64, f64, Xy, Xy, u32)> = Vec::new();
    for span in degree..n {
        let (a, b) = (*knots.get(span)?, *knots.get(span + 1)?);
        let (a, b) = (a.max(lo), b.min(hi));
        if a.partial_cmp(&b) != Some(std::cmp::Ordering::Less) {
            continue;
        }
        stack.push((a, b, eval(a)?, eval(b)?, 0));
        while let Some((t0, t1, p0, p1, depth)) = stack.pop() {
            let mid = (t0 + t1) / 2.0;
            let pm = eval(mid)?;
            let q1 = eval((t0 + mid) / 2.0)?;
            let q3 = eval((mid + t1) / 2.0)?;
            let flat = deviation(pm, p0, p1) <= tolerance
                && deviation(q1, p0, p1) <= tolerance
                && deviation(q3, p0, p1) <= tolerance;
            if (flat && depth >= 1) || depth >= 16 || out.len() >= MAX_STEPS {
                out.push(p1);
            } else {
                // 後に積んだものから取り出すので、後ろの半分を先に積む
                stack.push((mid, t1, pm, p1, depth + 1));
                stack.push((t0, mid, p0, pm, depth + 1));
            }
        }
    }
    Some(out)
}

/// クロソイドの、原点からの長さ `s` での接線の向き（ラジアン）
fn clothoid_angle(angle: f64, a2: f64, turn: f64, s: f64) -> f64 {
    angle + turn * s * s / (2.0 * a2)
}

/// クロソイドの `s0` から `s1` までの変位を、Simpson の公式で求める
fn clothoid_step(angle: f64, a2: f64, turn: f64, s0: f64, s1: f64) -> Xy {
    const PARTS: usize = 8;
    let h = (s1 - s0) / PARTS as f64;
    let (mut x, mut y) = (0.0, 0.0);
    for i in 0..=PARTS {
        let w = match i {
            0 | PARTS => 1.0,
            i if i % 2 == 1 => 4.0,
            _ => 2.0,
        };
        let phi = clothoid_angle(angle, a2, turn, s0 + h * i as f64);
        x += w * libm::cos(phi);
        y += w * libm::sin(phi);
    }
    [x * h / 3.0, y * h / 3.0]
}

/// クロソイドを折れ線にする。`angle` は原点での接線の向き（度）、`parameter` は A、`start` と `end` は
/// 原点からの長さ
pub fn clothoid(
    origin: Xy,
    parameter: f64,
    angle: f64,
    counterclockwise: bool,
    start: f64,
    end: f64,
    tolerance: f64,
) -> Vec<Xy> {
    let a2 = parameter * parameter;
    let angle = radians(angle);
    let turn = if counterclockwise { 1.0 } else { -1.0 };
    if !(a2 > 0.0 && a2.is_finite() && start.is_finite() && end.is_finite()) {
        return Vec::new();
    }
    // 弦の誤差は h²κ/8。いちばん曲がる所の曲率 κ = |s|/A² で刻みを決める
    let curvature = start.abs().max(end.abs()) / a2;
    let step = if curvature > 0.0 && tolerance > 0.0 {
        (8.0 * tolerance / curvature).sqrt()
    } else {
        (end - start).abs()
    };
    let steps = libm::ceil((end - start).abs() / step.max(f64::MIN_POSITIVE));
    let steps = if steps.is_finite() {
        steps.clamp(1.0, MAX_STEPS as f64) as usize
    } else {
        MAX_STEPS
    };
    // 原点から始まりまでを、同じ刻みで積み上げる
    let lead = libm::ceil(start.abs() / step.max(f64::MIN_POSITIVE));
    let lead = if lead.is_finite() {
        lead.clamp(1.0, MAX_STEPS as f64) as usize
    } else {
        MAX_STEPS
    };
    let mut point = origin;
    for i in 0..lead {
        let s0 = start * i as f64 / lead as f64;
        let s1 = start * (i + 1) as f64 / lead as f64;
        let [dx, dy] = clothoid_step(angle, a2, turn, s0, s1);
        point = [point[0] + dx, point[1] + dy];
    }
    let mut out = Vec::with_capacity(steps + 1);
    out.push(point);
    let ds = (end - start) / steps as f64;
    for i in 0..steps {
        let s0 = start + ds * i as f64;
        let [dx, dy] = clothoid_step(angle, a2, turn, s0, s0 + ds);
        point = [point[0] + dx, point[1] + dy];
        out.push(point);
    }
    out
}

/// 折れ線の長さ
pub fn length(points: &[Xy]) -> f64 {
    points
        .windows(2)
        .map(|w| match w {
            [a, b] => distance(*a, *b),
            _ => 0.0,
        })
        .sum()
}

/// 線種の列の 1 周の長さ（線と間と点の長さの絶対値の和）
pub fn period(pattern: &[f64]) -> f64 {
    pattern.iter().map(|v| v.abs()).sum()
}

/// 線種の列（正は線、負は間、0 は点）で、折れ線を線に分ける。点は、同じ点を 2 つ持つ線にする。
/// 列は折れ線の始点から当て、頂点をまたいで続ける。1 周の長さが 0 なら、元の折れ線を 1 本で返す
pub fn dashes(points: &[Xy], pattern: &[f64]) -> Vec<Vec<Xy>> {
    let total = period(pattern);
    if pattern.is_empty()
        || total.is_nan()
        || total <= 0.0
        || !total.is_finite()
        || points.len() < 2
    {
        return vec![points.to_vec()];
    }
    let mut out = Vec::new();
    let mut current: Option<Vec<Xy>> = None;
    let mut index = 0;
    let mut left = pattern.first().map_or(0.0, |v| v.abs());
    let mut drawing = pattern.first().is_some_and(|v| *v >= 0.0);
    if drawing {
        current = points.first().map(|p| vec![*p]);
    }
    for pair in points.windows(2) {
        let [a, b] = pair else { continue };
        let segment = distance(*a, *b);
        let mut at = 0.0;
        let along = |t: f64| {
            if segment > 0.0 {
                let f = t / segment;
                [a[0] + (b[0] - a[0]) * f, a[1] + (b[1] - a[1]) * f]
            } else {
                *a
            }
        };
        loop {
            if left > segment - at {
                left -= segment - at;
                if let Some(line) = current.as_mut() {
                    line.push(*b);
                }
                break;
            }
            at += left;
            let p = along(at);
            if let Some(mut line) = current.take() {
                line.push(p);
                out.push(line);
            }
            index = (index + 1) % pattern.len();
            let value = pattern.get(index).copied().unwrap_or(0.0);
            left = value.abs();
            drawing = value >= 0.0;
            if drawing {
                current = Some(vec![p]);
            }
        }
    }
    if let Some(line) = current
        && line.len() > 1
    {
        out.push(line);
    }
    out
}

/// 当てはめる点の間の媒介変数の取り方
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Spacing {
    /// 隣の点との距離に比例させる
    Chord,
    /// 隣の点との距離の平方根に比例させる
    SquareRoot,
    /// 等しい間隔にする
    Uniform,
}

/// 3 次の B スプラインの、`t` での 4 つの基底関数の値（`span` は `knots[span] <= t < knots[span + 1]`）。
/// The NURBS Book（Piegl・Tiller）の A2.2 の形
fn basis(knots: &[f64], span: usize, t: f64) -> Option<[f64; 4]> {
    let mut n = [1.0, 0.0, 0.0, 0.0];
    let mut left = [0.0; 4];
    let mut right = [0.0; 4];
    for j in 1..4 {
        *left.get_mut(j)? = t - *knots.get(span + 1 - j)?;
        *right.get_mut(j)? = *knots.get(span + j)? - t;
        let mut saved = 0.0;
        for r in 0..j {
            let denominator = *right.get(r + 1)? + *left.get(j - r)?;
            let temp = if denominator == 0.0 {
                0.0
            } else {
                *n.get(r)? / denominator
            };
            *n.get_mut(r)? = saved + *right.get(r + 1)? * temp;
            saved = *left.get(j - r)? * temp;
        }
        *n.get_mut(j)? = saved;
    }
    Some(n)
}

/// 当てはめる点を通る 3 次のスプラインの、節点と制御点。The NURBS Book の 9.2.4 の、両端の接線を与える
/// 補間による。媒介変数は 0〜1 にそろえ、節点は内側の点の媒介変数にする。接線（向き）の長さは、点を結ぶ
/// 弦の長さの和にする。接線がなければ、端の 3 点を通る 2 次式の接線にする（2 点なら弦）。閉じるなら、
/// 終わりに始めの点を足し、接線がなければ両端に、始めの点の前後の点を通る 2 次式の接線を使う。
/// 点が 2 つ未満か、続く 2 点が重なるか、値が有限でなければ `None`
pub fn interpolate(
    points: &[Xy],
    start: Option<Xy>,
    end: Option<Xy>,
    spacing: Spacing,
    closed: bool,
) -> Option<(Vec<f64>, Vec<Xy>)> {
    let mut q = points.to_vec();
    if closed && q.len() >= 3 && q.first() != q.last() {
        q.push(*q.first()?);
    }
    if q.len() < 2 || q.iter().any(|p| !(p[0].is_finite() && p[1].is_finite())) {
        return None;
    }
    let n = q.len() - 1;
    let mut chord = 0.0;
    let mut steps = Vec::with_capacity(n);
    for w in q.windows(2) {
        let [a, b] = w else { continue };
        let d = distance(*a, *b);
        if d <= 0.0 {
            return None;
        }
        chord += d;
        steps.push(match spacing {
            Spacing::Chord => d,
            Spacing::SquareRoot => d.sqrt(),
            Spacing::Uniform => 1.0,
        });
    }
    let total: f64 = steps.iter().sum();
    if !(total.is_finite() && total > 0.0 && chord.is_finite()) {
        return None;
    }
    let mut u = Vec::with_capacity(n + 1);
    let mut walked = 0.0;
    u.push(0.0);
    for step in &steps {
        walked += step;
        u.push(walked / total);
    }
    *u.last_mut()? = 1.0;
    let scaled = |t: Xy| {
        let l = libm::hypot(t[0], t[1]);
        (l > 0.0 && l.is_finite()).then(|| [t[0] / l * chord, t[1] / l * chord])
    };
    // 3 点 a・b・c を媒介変数の差 h0・h1 で通る 2 次式の、a・b・c での接線
    let quadratic = |a: Xy, b: Xy, c: Xy, h0: f64, h1: f64, at: usize| {
        let (ka, kb, kc) = match at {
            0 => (
                -(2.0 * h0 + h1) / (h0 * (h0 + h1)),
                (h0 + h1) / (h0 * h1),
                -h0 / (h1 * (h0 + h1)),
            ),
            1 => (
                -h1 / (h0 * (h0 + h1)),
                (h1 - h0) / (h0 * h1),
                h0 / (h1 * (h0 + h1)),
            ),
            _ => (
                h1 / (h0 * (h0 + h1)),
                -(h0 + h1) / (h0 * h1),
                (2.0 * h1 + h0) / (h1 * (h0 + h1)),
            ),
        };
        [
            ka * a[0] + kb * b[0] + kc * c[0],
            ka * a[1] + kb * b[1] + kc * c[1],
        ]
    };
    let first = *q.first()?;
    let last = *q.last()?;
    let seam = if closed && n >= 3 {
        let (before, after) = (*q.get(n - 1)?, *q.get(1)?);
        Some(quadratic(
            before,
            first,
            after,
            1.0 - *u.get(n - 1)?,
            *u.get(1)?,
            1,
        ))
    } else {
        None
    };
    let estimate = |at_start: bool| -> Option<Xy> {
        if let Some(seam) = seam {
            return Some(seam);
        }
        if n == 1 {
            return Some([last[0] - first[0], last[1] - first[1]]);
        }
        if at_start {
            let (h0, h1) = (*u.get(1)?, *u.get(2)? - *u.get(1)?);
            Some(quadratic(first, *q.get(1)?, *q.get(2)?, h0, h1, 0))
        } else {
            let h0 = *u.get(n - 1)? - *u.get(n - 2)?;
            let h1 = 1.0 - *u.get(n - 1)?;
            Some(quadratic(*q.get(n - 2)?, *q.get(n - 1)?, last, h0, h1, 2))
        }
    };
    let d0 = match start.and_then(scaled) {
        Some(d) => d,
        None => estimate(true)?,
    };
    let dn = match end.and_then(scaled) {
        Some(d) => d,
        None => estimate(false)?,
    };
    let mut knots = vec![0.0; 4];
    knots.extend(u.get(1..n)?.iter().copied());
    knots.extend([1.0; 4]);
    let a = *u.get(1)? / 3.0;
    let b = (1.0 - *u.get(n - 1)?) / 3.0;
    let p1 = [first[0] + d0[0] * a, first[1] + d0[1] * a];
    let pn = [last[0] - dn[0] * b, last[1] - dn[1] * b];
    // 内側の点 Q1〜Q(n-1) を通す、P2〜Pn の三重対角の連立方程式（Thomas の方法）
    let m = n - 1;
    let mut lower = Vec::with_capacity(m);
    let mut diagonal = Vec::with_capacity(m);
    let mut upper = Vec::with_capacity(m);
    let mut rhs: Vec<Xy> = Vec::with_capacity(m);
    for k in 1..n {
        let [na, nb, nc, _] = basis(&knots, k + 3, *u.get(k)?)?;
        let mut r = *q.get(k)?;
        if k == 1 {
            r = [r[0] - na * p1[0], r[1] - na * p1[1]];
        }
        if k == n - 1 {
            r = [r[0] - nc * pn[0], r[1] - nc * pn[1]];
        }
        lower.push(na);
        diagonal.push(nb);
        upper.push(nc);
        rhs.push(r);
    }
    for i in 1..m {
        let w = *lower.get(i)? / *diagonal.get(i - 1)?;
        *diagonal.get_mut(i)? -= w * *upper.get(i - 1)?;
        let prev = *rhs.get(i - 1)?;
        let r = rhs.get_mut(i)?;
        *r = [r[0] - w * prev[0], r[1] - w * prev[1]];
    }
    let mut inner = vec![[0.0; 2]; m];
    for i in (0..m).rev() {
        let next = inner.get(i + 1).copied().unwrap_or([0.0; 2]);
        let c = if i + 1 < m { *upper.get(i)? } else { 0.0 };
        let r = *rhs.get(i)?;
        let d = *diagonal.get(i)?;
        *inner.get_mut(i)? = [(r[0] - c * next[0]) / d, (r[1] - c * next[1]) / d];
    }
    let mut control = vec![first, p1];
    control.extend(inner);
    control.push(pn);
    control.push(last);
    control
        .iter()
        .all(|p| p[0].is_finite() && p[1].is_finite())
        .then_some((knots, control))
}

/// 点が輪の内側にあるか（偶奇の規則）。輪は閉じているものとする
pub fn inside(ring: &[Xy], p: Xy) -> bool {
    let mut odd = false;
    let count = ring.len();
    for i in 0..count {
        let (Some(a), Some(b)) = (ring.get(i), ring.get((i + 1) % count)) else {
            continue;
        };
        if (a[1] > p[1]) != (b[1] > p[1]) {
            let x = a[0] + (p[1] - a[1]) / (b[1] - a[1]) * (b[0] - a[0]);
            if p[0] < x {
                odd = !odd;
            }
        }
    }
    odd
}

/// 平行線の組の、輪に掛かる線の番号の範囲 `(最初, 最後)`。`k` 番目の線は、`base + k × offset` を通り、
/// 向き `angle`（度）に引く。線の間隔が 0 か、輪がなければ `None`
pub fn hatch_rows(rings: &[Vec<Xy>], angle: f64, base: Xy, offset: Xy) -> Option<(f64, f64)> {
    let r = radians(angle);
    let d = [libm::cos(r), libm::sin(r)];
    let spacing = d[0] * offset[1] - d[1] * offset[0];
    if !(spacing.is_finite() && spacing != 0.0) {
        return None;
    }
    let (mut lo, mut hi) = (f64::INFINITY, f64::NEG_INFINITY);
    for p in rings.iter().flatten() {
        let s = (d[0] * (p[1] - base[1]) - d[1] * (p[0] - base[0])) / spacing;
        lo = lo.min(s);
        hi = hi.max(s);
    }
    (lo.is_finite() && hi.is_finite()).then(|| (libm::ceil(lo), libm::floor(hi)))
}

/// 平行線の組の `k` 番目の線の、輪の内側（偶奇の規則）の部分を、線と間の列（正は線、負は間、0 は点）で
/// 分けた線。列は `base + k × offset` から測る。列が空なら実線
pub fn hatch_row(
    rings: &[Vec<Xy>],
    angle: f64,
    base: Xy,
    offset: Xy,
    k: f64,
    dashes: &[f64],
) -> Vec<Vec<Xy>> {
    let r = radians(angle);
    let d = [libm::cos(r), libm::sin(r)];
    let o = [base[0] + k * offset[0], base[1] + k * offset[1]];
    let side = |p: &Xy| d[0] * (p[1] - o[1]) - d[1] * (p[0] - o[0]);
    let along = |p: &Xy| d[0] * (p[0] - o[0]) + d[1] * (p[1] - o[1]);
    let mut cuts = Vec::new();
    for ring in rings {
        let count = ring.len();
        for i in 0..count {
            let (Some(a), Some(b)) = (ring.get(i), ring.get((i + 1) % count)) else {
                continue;
            };
            let (sa, sb) = (side(a), side(b));
            if (sa > 0.0) != (sb > 0.0) {
                let f = sa / (sa - sb);
                cuts.push(along(a) + (along(b) - along(a)) * f);
            }
        }
    }
    cuts.retain(|t| t.is_finite());
    cuts.sort_by(f64::total_cmp);
    let at = |t: f64| [o[0] + d[0] * t, o[1] + d[1] * t];
    let total = period(dashes);
    let mut out = Vec::new();
    for &[t0, t1] in cuts.as_chunks::<2>().0 {
        if dashes.is_empty() || !(total.is_finite() && total > 0.0) {
            out.push(vec![at(t0), at(t1)]);
            continue;
        }
        let mut pos = libm::floor(t0 / total) * total;
        let mut steps = 0;
        'walk: while pos <= t1 {
            for value in dashes {
                let end = pos + value.abs();
                if *value > 0.0 {
                    let (a, b) = (pos.max(t0), end.min(t1));
                    if a < b {
                        out.push(vec![at(a), at(b)]);
                    }
                } else if *value == 0.0 && pos >= t0 && pos <= t1 {
                    out.push(vec![at(pos), at(pos)]);
                }
                pos = end;
                steps += 1;
                if pos > t1 || steps >= MAX_STEPS {
                    break 'walk;
                }
            }
        }
    }
    out
}

/// 線種の列の `index` 番目の要素が始まる位置と、そこでの線の向き（度）。列は折れ線の始点から当て、頂点を
/// またいで続ける（[`dashes`] と同じ）。1 周の長さが 0 なら空
pub fn starts(points: &[Xy], pattern: &[f64], index: usize) -> Vec<(Xy, f64)> {
    let total = period(pattern);
    if !(total.is_finite() && total > 0.0) || index >= pattern.len() {
        return Vec::new();
    }
    let first: f64 = pattern.iter().take(index).map(|v| v.abs()).sum();
    let mut out = Vec::new();
    let mut next = first;
    let mut walked = 0.0;
    for pair in points.windows(2) {
        let [a, b] = pair else { continue };
        let segment = distance(*a, *b);
        if segment <= 0.0 {
            continue;
        }
        let direction = libm::atan2(b[1] - a[1], b[0] - a[0]).to_degrees();
        while next <= walked + segment && out.len() < MAX_STEPS {
            let f = (next - walked) / segment;
            out.push((
                [a[0] + (b[0] - a[0]) * f, a[1] + (b[1] - a[1]) * f],
                direction,
            ));
            next += total;
        }
        walked += segment;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: Xy, b: Xy) -> bool {
        (a[0] - b[0]).abs() < 1e-9 && (a[1] - b[1]).abs() < 1e-9
    }

    #[test]
    fn sweeps_follow_the_direction() {
        assert_eq!(sweep(0.0, 90.0, true), 90.0);
        assert_eq!(sweep(0.0, 90.0, false), -270.0);
        assert_eq!(sweep(30.0, 30.0, true), 360.0);
        assert_eq!(sweep(0.0, 720.0, false), -360.0);
    }

    #[test]
    fn arcs_stay_within_the_tolerance() {
        let mut out = Vec::new();
        arc([0.0, 0.0], 10.0, 0.0, 90.0, 0.01, &mut out);
        assert!(close(out[0], [10.0, 0.0]));
        assert!(close(*out.last().unwrap(), [0.0, 10.0]));
        for w in out.windows(2) {
            let mid = [(w[0][0] + w[1][0]) / 2.0, (w[0][1] + w[1][1]) / 2.0];
            assert!(10.0 - libm::hypot(mid[0], mid[1]) <= 0.01 + 1e-12);
        }
        // 弦の誤差 0.01 なら、刻みは 2 acos(0.999) ≈ 5.1 度で、18 区間
        assert_eq!(out.len(), 19);
    }

    #[test]
    fn bulges_make_counterclockwise_half_circles() {
        let points = polyline(&[([0.0, 0.0], 1.0), ([2.0, 0.0], 0.0)], false, 0.001);
        assert!(close(points[0], [0.0, 0.0]));
        assert!(close(*points.last().unwrap(), [2.0, 0.0]));
        // 反時計回りの半円は、弦の下（右）を通る
        let lowest = points.iter().map(|p| p[1]).fold(f64::INFINITY, f64::min);
        assert!((lowest + 1.0).abs() < 0.001);
    }

    #[test]
    fn clamped_quadratic_splines_hit_their_ends() {
        let points = spline(
            2,
            &[0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
            &[[0.0, 0.0], [1.0, 2.0], [2.0, 0.0]],
            &[],
            0.001,
        )
        .unwrap();
        assert!(close(points[0], [0.0, 0.0]));
        assert!(close(*points.last().unwrap(), [2.0, 0.0]));
        // 頂点は (1, 1)
        let top = points.iter().map(|p| p[1]).fold(0.0, f64::max);
        assert!((top - 1.0).abs() < 0.002);
        assert!(spline(2, &[0.0, 1.0], &[[0.0, 0.0]; 3], &[], 0.1).is_none());
    }

    #[test]
    fn clothoids_start_straight_and_turn() {
        let points = clothoid([0.0, 0.0], 10.0, 0.0, true, 0.0, 10.0, 0.001);
        assert!(close(points[0], [0.0, 0.0]));
        let end = *points.last().unwrap();
        // 長さ 10・A 10 の終わりは、Fresnel 積分から (9.7529, 1.6371) ほど
        assert!((end[0] - 9.7529).abs() < 1e-3, "{end:?}");
        assert!((end[1] - 1.6371).abs() < 1e-3, "{end:?}");
    }

    #[test]
    fn dashes_continue_across_vertices() {
        let lines = dashes(
            &[[0.0, 0.0], [3.0, 0.0], [3.0, 3.0]],
            &[2.0, -1.0, 0.0, -1.0],
        );
        assert_eq!(
            lines,
            vec![
                vec![[0.0, 0.0], [2.0, 0.0]],
                vec![[3.0, 0.0], [3.0, 0.0]],
                vec![[3.0, 1.0], [3.0, 3.0]],
            ]
        );
        assert_eq!(dashes(&[[0.0, 0.0], [1.0, 0.0]], &[]).len(), 1);
    }

    #[test]
    fn interpolated_splines_pass_through_their_points() {
        let points = [[0.0, 0.0], [1.0, 1.0], [2.0, 0.0], [3.0, 1.0]];
        let (knots, control) = interpolate(&points, None, None, Spacing::Chord, false).unwrap();
        assert_eq!(control.len(), points.len() + 2);
        assert_eq!(knots.len(), control.len() + 4);
        let curve = spline(3, &knots, &control, &[], 1e-6).unwrap();
        for p in points {
            let nearest = curve
                .iter()
                .map(|c| libm::hypot(c[0] - p[0], c[1] - p[1]))
                .fold(f64::INFINITY, f64::min);
            assert!(nearest < 1e-3, "{p:?} {nearest}");
        }
        // 直線に並ぶ点は、直線になる
        let (k, c) = interpolate(
            &[[0.0, 0.0], [1.0, 0.0], [3.0, 0.0]],
            None,
            None,
            Spacing::Chord,
            false,
        )
        .unwrap();
        assert!(
            spline(3, &k, &c, &[], 1e-9)
                .unwrap()
                .iter()
                .all(|p| p[1].abs() < 1e-12)
        );
        // 接線を与えると、その向きで出る
        let (_, c) = interpolate(
            &[[0.0, 0.0], [1.0, 0.0]],
            Some([0.0, 1.0]),
            None,
            Spacing::Chord,
            false,
        )
        .unwrap();
        assert!(close(c[1], [0.0, 1.0 / 3.0]));
        assert!(
            interpolate(&[[0.0, 0.0], [0.0, 0.0]], None, None, Spacing::Chord, false).is_none()
        );
    }

    #[test]
    fn hatch_rows_keep_the_inside_of_rings() {
        let outer = vec![[0.0, 0.0], [10.0, 0.0], [10.0, 10.0], [0.0, 10.0]];
        let hole = vec![[4.0, 4.0], [6.0, 4.0], [6.0, 6.0], [4.0, 6.0]];
        let rings = vec![outer, hole];
        let (lo, hi) = hatch_rows(&rings, 0.0, [0.0, 0.5], [0.0, 1.0]).unwrap();
        assert_eq!((lo, hi), (-0.0, 9.0));
        let row = hatch_row(&rings, 0.0, [0.0, 0.5], [0.0, 1.0], 4.0, &[]);
        assert_eq!(
            row,
            vec![vec![[0.0, 4.5], [4.0, 4.5]], vec![[6.0, 4.5], [10.0, 4.5]]]
        );
        let dashed = hatch_row(&rings, 0.0, [0.0, 0.5], [0.0, 1.0], 0.0, &[3.0, -2.0]);
        assert_eq!(dashed.len(), 2);
        assert_eq!(dashed[1], vec![[5.0, 0.5], [8.0, 0.5]]);
        assert!(hatch_rows(&rings, 0.0, [0.0, 0.0], [1.0, 0.0]).is_none());
        assert!(inside(&rings[0], [5.0, 5.0]));
        assert!(!inside(&rings[1], [1.0, 1.0]));
    }

    #[test]
    fn starts_repeat_along_the_line() {
        let found = starts(&[[0.0, 0.0], [5.0, 0.0], [5.0, 5.0]], &[2.0, -2.0], 1);
        let places: Vec<Xy> = found.iter().map(|(p, _)| *p).collect();
        assert_eq!(places, vec![[2.0, 0.0], [5.0, 1.0], [5.0, 5.0]]);
        assert_eq!(found[1].1, 90.0);
    }
}
