//! 線・円弧・楕円・ポリライン・スプライン・頂点・ハッチングなどの幾何図形フィールドの読み取り。

use super::*;

/// LINE のフィールド（ODS 5.4.1 の 20.4.17）。端点はいつも置き、厚さと押し出しの向きは既定でなければ置く
pub(super) fn line(bits: &mut Bits) -> Option<Vec<Value>> {
    let flat = bits.bit()?;
    let start_x = bits.rd()?;
    let end_x = bits.dd(start_x)?;
    let start_y = bits.rd()?;
    let end_y = bits.dd(start_y)?;
    let (start_z, end_z) = if flat {
        (0.0, 0.0)
    } else {
        let start_z = bits.rd()?;
        (start_z, bits.dd(start_z)?)
    };
    let mut values = vec![
        real(10, start_x),
        real(20, start_y),
        real(30, start_z),
        real(11, end_x),
        real(21, end_y),
        real(31, end_z),
    ];
    thickness_and_extrusion(bits, &mut values)?;
    Some(values)
}

/// CIRCLE のフィールド（ODS 5.4.1 の 20.4.20）。中心と半径
pub(super) fn circle(bits: &mut Bits) -> Option<Vec<Value>> {
    let mut values = point3(bits, 10)?;
    values.push(real(40, bits.bd()?));
    thickness_and_extrusion(bits, &mut values)?;
    Some(values)
}

/// ARC のフィールド（ODS 5.4.1 の 20.4.18）。CIRCLE のフィールドと、始まりと終わりの角度。DWG はラジアン、
/// DXF は度で書くので、度に直す
pub(super) fn arc(bits: &mut Bits) -> Option<Vec<Value>> {
    let mut values = circle(bits)?;
    values.push(real(50, bits.bd()?.to_degrees()));
    values.push(real(51, bits.bd()?.to_degrees()));
    Some(values)
}

/// POINT のフィールド（ODS 5.4.1 の 20.4.31）。X 軸の角度（50）は 0 でなければ度に直して置く
pub(super) fn point(bits: &mut Bits) -> Option<Vec<Value>> {
    let mut values = point3(bits, 10)?;
    thickness_and_extrusion(bits, &mut values)?;
    let angle = bits.bd()?;
    if angle != 0.0 {
        values.push(real(50, angle.to_degrees()));
    }
    Some(values)
}

/// SOLID・TRACE のフィールド（ODS 5.4.1 の 20.4.35・20.4.36）。4 つの角の Z（30〜33）は標高とする
pub(super) fn solid(bits: &mut Bits) -> Option<Vec<Value>> {
    let thickness = if bits.bit()? { None } else { Some(bits.bd()?) };
    let elevation = bits.bd()?;
    let mut values = Vec::new();
    for code in 10..14 {
        values.push(real(code, bits.rd()?));
        values.push(real(code + 10, bits.rd()?));
        values.push(real(code + 20, elevation));
    }
    if let Some(thickness) = thickness {
        values.push(real(39, thickness));
    }
    if !bits.bit()? {
        values.extend(point3(bits, 210)?);
    }
    Some(values)
}

/// 3DFACE のフィールド（ODS 5.4.1 の 20.4.32、R2000）。2 つ目からの角は、前の角を既定の値とする DD。
/// 見えない辺のフラグ（70）は、記録されていて 0 でなければ置く
pub(super) fn face3d(bits: &mut Bits) -> Option<Vec<Value>> {
    let no_flags = bits.bit()?;
    let z_is_zero = bits.bit()?;
    let x = bits.rd()?;
    let y = bits.rd()?;
    let z = if z_is_zero { 0.0 } else { bits.rd()? };
    let mut corner = [x, y, z];
    let mut values = Vec::new();
    for code in 10..14 {
        if code > 10 {
            for c in &mut corner {
                *c = bits.dd(*c)?;
            }
        }
        let [x, y, z] = corner;
        values.extend([real(code, x), real(code + 10, y), real(code + 20, z)]);
    }
    if !no_flags {
        let invisible = bits.bs()?;
        if invisible != 0 {
            values.push(int(70, i64::from(invisible)));
        }
    }
    Some(values)
}

/// SHAPE のフィールド（ODS 5.4.1 の 20.4.33）。挿入点（10）と大きさ（40）を置き、回転（50）・幅の係数（41）・
/// 傾き（51）・厚さ（39）・押し出しの向きは既定の値でなければ置く。角度は度に直す。DXF の形の名前（2）は、
/// DWG では形の番号と形のファイル（文字スタイル）で記録され、名前はファイルの中にあるので、まだ置かない
pub(super) fn shape(bits: &mut Bits) -> Option<Vec<Value>> {
    let mut values = point3(bits, 10)?;
    values.push(real(40, bits.bd()?));
    for (code, default, angle) in [(50, 0.0, true), (41, 1.0, false), (51, 0.0, true)] {
        let value = bits.bd()?;
        if value != default {
            values.push(real(code, if angle { value.to_degrees() } else { value }));
        }
    }
    let thickness = bits.bd()?;
    if thickness != 0.0 {
        values.push(real(39, thickness));
    }
    // 形の番号
    bits.bs()?;
    let extrusion = point3(bits, 210)?;
    if !is_up(&extrusion) {
        values.extend(extrusion);
    }
    Some(values)
}

/// ELLIPSE のフィールド（ODS 5.4.1 の 20.4.39）。始まりと終わりの角度（41・42）は、DXF も
/// ラジアンで書くのでそのまま置く。押し出しの向きは既定の値でなければ置く
pub(super) fn ellipse(bits: &mut Bits) -> Option<Vec<Value>> {
    let mut values = point3(bits, 10)?;
    values.extend(point3(bits, 11)?);
    let extrusion = point3(bits, 210)?;
    values.push(real(40, bits.bd()?));
    values.push(real(41, bits.bd()?));
    values.push(real(42, bits.bd()?));
    if !is_up(&extrusion) {
        values.extend(extrusion);
    }
    Some(values)
}

/// RAY・XLINE のフィールド（ODS 5.4.1 の 20.4.42・20.4.43）。始まりの点と向き
pub(super) fn ray(bits: &mut Bits) -> Option<Vec<Value>> {
    let mut values = point3(bits, 10)?;
    values.extend(point3(bits, 11)?);
    Some(values)
}

/// LWPOLYLINE のフィールド（ODS 5.4.1 の 20.4.85、R2000）。DWG のフラグのうち、閉じていること（0x200）を DXF の 70 の
/// 1 に、線種の生成（0x100）を 128 に直す。ほかのフラグは、続くフィールドがあるかを表す。DXF と同じく、頂点ごとの幅がなければ
/// 一定の幅（43）を置き、膨らみ（42）は 0 でなければ置く
pub(super) fn lwpolyline(bits: &mut Bits) -> Option<Vec<Value>> {
    let flags = bits.bs()?;
    let has = |bit: i16| flags & bit != 0;
    let width = if has(4) { bits.bd()? } else { 0.0 };
    let elevation = if has(8) { Some(bits.bd()?) } else { None };
    let thickness = if has(2) { Some(bits.bd()?) } else { None };
    let extrusion = if has(1) {
        Some(point3(bits, 210)?)
    } else {
        None
    };
    let count = usize::try_from(bits.bl()?).ok()?;
    let bulges = if has(16) {
        usize::try_from(bits.bl()?).ok()?
    } else {
        0
    };
    let widths = if has(32) {
        usize::try_from(bits.bl()?).ok()?
    } else {
        0
    };
    let mut points = Vec::new();
    if count > 0 {
        let mut x = bits.rd()?;
        let mut y = bits.rd()?;
        points.push((x, y));
        for _ in 1..count {
            x = bits.dd(x)?;
            y = bits.dd(y)?;
            points.push((x, y));
        }
    }
    let bulges: Vec<f64> = (0..bulges).map(|_| bits.bd()).collect::<Option<_>>()?;
    let widths: Vec<(f64, f64)> = (0..widths)
        .map(|_| Some((bits.bd()?, bits.bd()?)))
        .collect::<Option<_>>()?;
    let mut values = vec![
        int(90, i64::try_from(count).ok()?),
        int(70, i64::from(has(0x200)) | i64::from(has(0x100)) << 7),
    ];
    if widths.is_empty() {
        values.push(real(43, width));
    }
    if let Some(elevation) = elevation {
        values.push(real(38, elevation));
    }
    if let Some(thickness) = thickness {
        values.push(real(39, thickness));
    }
    for (i, (x, y)) in points.into_iter().enumerate() {
        values.push(real(10, x));
        values.push(real(20, y));
        if let Some((start, end)) = widths.get(i) {
            values.push(real(40, *start));
            values.push(real(41, *end));
        }
        if let Some(bulge) = bulges.get(i).filter(|b| **b != 0.0) {
            values.push(real(42, *bulge));
        }
    }
    if let Some(extrusion) = extrusion {
        values.extend(extrusion);
    }
    Some(values)
}

/// SPLINE のフィールド（ODS 5.4.1 の 20.4.40、R2000）。場合（scenario）が 1 ならノットと制御点、2 なら
/// フィット点を読む。DXF のフラグ（70）の平らか（8）と法線（210）は、DXF を書くときに平らかどうかから
/// 決まるので置かない。始まりと終わりの接線（12・13）は、0 でなければ置く
pub(super) fn spline(bits: &mut Bits) -> Option<Vec<Value>> {
    let scenario = bits.bl()?;
    // R2013 からは、スプラインのフラグと、ノットの取り方（どちらも BL）が続く。フラグの 1 が立つときは、
    // 場合が 1 でも、フィット点のフィールドだけがあり、制御点のフィールドはない（図面で確かめた）。このときは
    // DXF と同じく、70 にフィット点で決まること（1024）とノットの取り方のビットを置く（決定 0157）
    let mut fitted = scenario == 2;
    let mut fit_only = false;
    let mut method = 0;
    if bits.release() >= Release::R2013 {
        fit_only = bits.bl()? & 1 != 0;
        fitted |= fit_only;
        let knot = bits.bl()?;
        if fit_only {
            method = 1024 | knot_bit(knot);
        }
    }
    let degree = bits.bl()?;
    let mut values = vec![int(71, i64::from(degree))];
    let mut fit = 0;
    let mut knots = 0;
    let mut controls = 0;
    let mut weighted = false;
    let mut tail = Vec::new();
    if !matches!(scenario, 1 | 2) {
        return None;
    }
    if fitted {
        let tolerance = bits.bd()?;
        let start = point3(bits, 12)?;
        let end = point3(bits, 13)?;
        fit = bits.bl()?;
        if fit_only && scenario == 1 {
            return spline_from_fit(bits, degree, fit, &start, &end, method);
        }
        if method != 0 {
            values.insert(0, int(70, method));
        }
        tail.push(real(44, tolerance));
        for tangent in [start, end] {
            if reals(&tangent).iter().any(|r| *r != Some(0.0)) {
                tail.extend(tangent);
            }
        }
    }
    if scenario == 1 && !fit_only {
        // フラグ（70）の 4 は有理、1 は閉じている、2 は周期的。DXF の 70 は平らか（8）なども持つが、DWG に
        // 書かれないので置かない
        let mut flags = 0;
        for bit in [4, 1, 2] {
            if bits.bit()? {
                flags |= bit;
            }
        }
        values.insert(0, int(70, flags));
        tail.push(real(42, bits.bd()?));
        tail.push(real(43, bits.bd()?));
        knots = bits.bl()?;
        controls = bits.bl()?;
        weighted = bits.bit()?;
    }
    values.push(int(72, i64::from(knots)));
    values.push(int(73, i64::from(controls)));
    values.push(int(74, i64::from(fit)));
    values.extend(tail);
    for _ in 0..knots {
        values.push(real(40, bits.bd()?));
    }
    for _ in 0..controls {
        values.extend(point3(bits, 10)?);
        if weighted {
            values.push(real(41, bits.bd()?));
        }
    }
    for _ in 0..fit {
        values.extend(point3(bits, 11)?);
    }
    Some(values)
}

/// R2013 からの、制御点を書かないスプライン（場合が 1 で、フィット点だけを持つ）。DXF は制御点で
/// 書くので、3 次でフィット点が 2 つのときは、両端の点と接線から Bézier の制御点を求める（接線の
/// 3 分の 1 だけ両端から離す。図面で確かめた）。ノットと制御点の許容差は書かれないので、DXF が置く
/// 値（1e-9 と 1e-10）を置く。それ以外は、フィット点のフィールドとして置く
pub(super) fn spline_from_fit(
    bits: &mut Bits,
    degree: i32,
    fit: i32,
    start: &[Value],
    end: &[Value],
    method: i64,
) -> Option<Vec<Value>> {
    let mut points = Vec::new();
    for _ in 0..fit {
        points.push(point3(bits, 11)?);
    }
    let xyz = |values: &[Value]| -> Option<[f64; 3]> {
        let r = reals(values);
        Some([
            r.first().copied()??,
            r.get(1).copied()??,
            r.get(2).copied()??,
        ])
    };
    let mut values = vec![int(70, method), int(71, i64::from(degree))];
    if degree == 3 && fit == 2 {
        let (p0, p3) = (xyz(points.first()?)?, xyz(points.get(1)?)?);
        let ([x0, y0, z0], [x3, y3, z3]) = (p0, p3);
        let ([a, b, c], [d, e, f]) = (xyz(start)?, xyz(end)?);
        let p1 = [x0 + a / 3.0, y0 + b / 3.0, z0 + c / 3.0];
        let p2 = [x3 - d / 3.0, y3 - e / 3.0, z3 - f / 3.0];
        values.extend([
            int(72, 8),
            int(73, 4),
            int(74, 0),
            real(42, 1e-9),
            real(43, 1e-10),
        ]);
        for knot in [0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0] {
            values.push(real(40, knot));
        }
        for [x, y, z] in [p0, p1, p2, p3] {
            values.extend([real(10, x), real(20, y), real(30, z)]);
        }
        return Some(values);
    }
    values.extend([int(72, 0), int(73, 0), int(74, i64::from(fit))]);
    for tangent in [start, end] {
        if reals(tangent).iter().any(|r| *r != Some(0.0)) {
            values.extend(tangent.iter().cloned());
        }
    }
    values.extend(points.into_iter().flatten());
    Some(values)
}

/// DWG のノットの取り方（0 は弦の長さ、1 は平方根、2 は一様、15 は指定）を、DXF の 70 のビット（32・64・
/// 128・256）にする。ほかの値はビットを立てない
pub(super) fn knot_bit(knot: i32) -> i64 {
    match knot {
        0 => 32,
        1 => 64,
        2 => 128,
        15 => 256,
        _ => 0,
    }
}

/// 2D の VERTEX のフィールド（ODS 5.4.1 の 20.4.11、R2000）。始まりの幅が負なら、終わりの幅は書かれず、
/// 始まりと同じ絶対値とする。幅・膨らみ・接線の向きは 0 でなければ置き、接線の向きは度に直す
pub(super) fn vertex2d(bits: &mut Bits) -> Option<Vec<Value>> {
    let mut values = vec![int(70, i64::from(bits.rc()?))];
    values.extend(point3(bits, 10)?);
    let start = bits.bd()?;
    let (start, end) = if start < 0.0 {
        (start.abs(), start.abs())
    } else {
        (start, bits.bd()?)
    };
    let bulge = bits.bd()?;
    // R2010 からは、頂点の番号（BL、DXF の 91）が続く。表には置かない
    if bits.release() >= Release::R2010 {
        bits.bl()?;
    }
    let tangent = bits.bd()?.to_degrees();
    for (code, value) in [(40, start), (41, end), (42, bulge), (50, tangent)] {
        if value != 0.0 {
            values.push(real(code, value));
        }
    }
    Some(values)
}

/// 3D・網・多面の頂点の VERTEX のフィールド（ODS 5.4.1 の 20.4.12〜20.4.14）。フラグ（70）と点
pub(super) fn vertex3d(bits: &mut Bits) -> Option<Vec<Value>> {
    let mut values = vec![int(70, i64::from(bits.rc()?))];
    values.extend(point3(bits, 10)?);
    Some(values)
}

/// 多面の面の VERTEX のフィールド（ODS 5.4.1 の 20.4.15）。DXF と同じく、点を 0、フラグ（70）を 128 とする。
/// 頂点の番号（71〜74）は 0 でなければ置く。負の番号は、その辺が見えないこと
pub(super) fn face_record(bits: &mut Bits) -> Option<Vec<Value>> {
    let mut values = vec![real(10, 0.0), real(20, 0.0), real(30, 0.0), int(70, 128)];
    for code in 71..75 {
        let index = bits.bs()?;
        if index != 0 {
            values.push(int(code, i64::from(index)));
        }
    }
    Some(values)
}

/// 2D の POLYLINE のフィールド（ODS 5.4.1 の 20.4.16、R2000）。POLYLINE には、DXF と同じく、頂点が続くフラグ（66）を置く。DXF と同じく、標高を 30 に置き、10・20 は
/// 0 とする。曲線の種類（75）と既定の幅（40・41）は 0 でなければ置く
pub(super) fn polyline2d(bits: &mut Bits) -> Option<Vec<Value>> {
    let mut values = vec![int(66, 1), int(70, i64::from(bits.bs()?))];
    let curve = bits.bs()?;
    if curve != 0 {
        values.push(int(75, i64::from(curve)));
    }
    for code in [40, 41] {
        let width = bits.bd()?;
        if width != 0.0 {
            values.push(real(code, width));
        }
    }
    // BT: 1 なら厚さは 0
    if !bits.bit()? {
        values.push(real(39, bits.bd()?));
    }
    values.extend([real(10, 0.0), real(20, 0.0), real(30, bits.bd()?)]);
    // BE: 1 なら押し出しの向きは (0, 0, 1)
    if !bits.bit()? {
        values.extend(point3(bits, 210)?);
    }
    Some(values)
}

/// 3D の POLYLINE のフィールド（ODS 5.4.1 の 20.4.17）。2 つのフラグから、DXF の 70（3D の 8、閉じていれば
/// 1、曲線なら 4）と曲線の種類（75）を作る。点（10）は 0
pub(super) fn polyline3d(bits: &mut Bits) -> Option<Vec<Value>> {
    let curve = bits.rc()?;
    let closed = bits.rc()? & 1;
    let splined = if curve & 3 != 0 { 4 } else { 0 };
    let mut values = vec![
        int(66, 1),
        real(10, 0.0),
        real(20, 0.0),
        real(30, 0.0),
        int(70, i64::from(8 | splined | closed)),
    ];
    if curve & 1 != 0 {
        values.push(int(75, 5));
    } else if curve & 2 != 0 {
        values.push(int(75, 6));
    }
    Some(values)
}

/// 多面の POLYLINE のフィールド（ODS 5.4.1 の 20.4.33）。DXF と同じく、点（10）を 0、フラグ（70）を 64 とし、
/// 頂点の数（71）と面の数（72）を置く
pub(super) fn polyface(bits: &mut Bits) -> Option<Vec<Value>> {
    Some(vec![
        int(66, 1),
        real(10, 0.0),
        real(20, 0.0),
        real(30, 0.0),
        int(70, 64),
        int(71, i64::from(bits.bs()?)),
        int(72, i64::from(bits.bs()?)),
    ])
}

/// 網の POLYLINE のフィールド（ODS 5.4.1 の 20.4.34）。点（10）は 0。フラグ（70）・曲面の種類（75）・頂点の数
/// （71・72）・密度（73・74）を置き、曲面の種類は 0 でなければ置く
pub(super) fn mesh(bits: &mut Bits) -> Option<Vec<Value>> {
    let mut values = vec![
        int(66, 1),
        real(10, 0.0),
        real(20, 0.0),
        real(30, 0.0),
        int(70, i64::from(bits.bs()?)),
    ];
    let surface = bits.bs()?;
    if surface != 0 {
        values.push(int(75, i64::from(surface)));
    }
    for code in 71..75 {
        values.push(int(code, i64::from(bits.bs()?)));
    }
    Some(values)
}

/// HATCH のフィールド（ODS 5.4.1 の 20.4.75、R2000）。DXF と同じく、標高を 30 に置き、10・20 は 0 とする。
/// 角度は度に直す。境界の図形のハンドル（330）は、ハンドルのストリームから読んで輪ごとの数（97）の後に置く
pub(super) fn hatch(bits: &mut Bits) -> Option<Vec<Value>> {
    // R2004 からは、先頭にグラデーションのフィールドが来る。DXF と同じく、グラデーションのときだけ置く
    let gradient = if bits.release() >= Release::R2004 {
        hatch_gradient(bits)?
    } else {
        Vec::new()
    };
    let elevation = bits.bd()?;
    let mut values = vec![real(10, 0.0), real(20, 0.0), real(30, elevation)];
    let extrusion = point3(bits, 210)?;
    if !is_up(&extrusion) {
        values.extend(extrusion);
    }
    values.push(text_value(2, &bits.text()?));
    let solid = bits.bit()?;
    values.push(int(70, i64::from(solid)));
    values.push(int(71, i64::from(bits.bit()?)));
    let paths = bits.bl()?;
    values.push(int(91, i64::from(paths)));
    let mut pixel_size = false;
    for _ in 0..paths {
        let flags = bits.bl()?;
        values.push(int(92, i64::from(flags)));
        pixel_size |= flags & 4 != 0;
        if flags & 2 == 0 {
            let edges = bits.bl()?;
            values.push(int(93, i64::from(edges)));
            for _ in 0..edges {
                hatch_edge(bits, &mut values)?;
            }
        } else {
            // 多角形の境界。DXF は頂点の数を 93 に書く
            let bulges = bits.bit()?;
            values.push(int(72, i64::from(bulges)));
            values.push(int(73, i64::from(bits.bit()?)));
            let count = bits.bl()?;
            values.push(int(93, i64::from(count)));
            for _ in 0..count {
                values.extend(point2(bits, 10)?);
                if bulges {
                    values.push(real(42, bits.bd()?));
                }
            }
        }
        values.push(int(97, i64::from(bits.bl()?)));
    }
    values.push(int(75, i64::from(bits.bs()?)));
    values.push(int(76, i64::from(bits.bs()?)));
    if !solid {
        values.push(real(52, bits.bd()?.to_degrees()));
        values.push(real(41, bits.bd()?));
        values.push(int(77, i64::from(bits.bit()?)));
        let lines = bits.bs()?;
        values.push(int(78, i64::from(lines)));
        for _ in 0..lines {
            values.push(real(53, bits.bd()?.to_degrees()));
            values.push(real(43, bits.bd()?));
            values.push(real(44, bits.bd()?));
            values.push(real(45, bits.bd()?));
            values.push(real(46, bits.bd()?));
            let dashes = bits.bs()?;
            values.push(int(79, i64::from(dashes)));
            for _ in 0..dashes {
                values.push(real(49, bits.bd()?));
            }
        }
    }
    if pixel_size {
        values.push(real(47, bits.bd()?));
    }
    let seeds = bits.bl()?;
    values.push(int(98, i64::from(seeds)));
    for _ in 0..seeds {
        values.extend(point2(bits, 10)?);
    }
    values.extend(gradient);
    Some(values)
}

/// HATCH のグラデーションのフィールド（ODS 5.4.1 の 20.4.75、R2004 から）。グラデーションでなければ空
pub(in crate::dwg) fn hatch_gradient(bits: &mut Bits) -> Option<Vec<Value>> {
    let kind = bits.bl()?;
    let reserved = bits.bl()?;
    let angle = bits.bd()?;
    let shift = bits.bd()?;
    let single = bits.bl()?;
    let tint = bits.bd()?;
    let count = bits.bl()?;
    let mut colors = Vec::new();
    for _ in 0..count {
        colors.push(real(463, bits.bd()?));
        bits.bs()?;
        colors.push(int(421, i64::from(bits.bl()? as u32 & 0x00FF_FFFF)));
        bits.rc()?;
    }
    let name = bits.text()?;
    if kind == 0 {
        return Some(Vec::new());
    }
    let mut values = vec![
        int(450, i64::from(kind)),
        int(451, i64::from(reserved)),
        int(452, i64::from(single)),
        int(453, i64::from(count)),
    ];
    values.extend(colors);
    values.extend([
        real(460, angle),
        real(461, shift),
        real(462, tint),
        text_value(470, &name),
    ]);
    Some(values)
}

/// HATCH の境界の 1 つの辺。種類（72）が 1 なら線分、2 なら円弧、3 なら楕円弧、4 なら SPLINE
pub(super) fn hatch_edge(bits: &mut Bits, values: &mut Vec<Value>) -> Option<()> {
    let kind = bits.rc()?;
    values.push(int(72, i64::from(kind)));
    match kind {
        1 => {
            values.extend(point2(bits, 10)?);
            values.extend(point2(bits, 11)?);
        }
        2 => {
            values.extend(point2(bits, 10)?);
            values.push(real(40, bits.bd()?));
            values.push(real(50, bits.bd()?.to_degrees()));
            values.push(real(51, bits.bd()?.to_degrees()));
            values.push(int(73, i64::from(bits.bit()?)));
        }
        3 => {
            values.extend(point2(bits, 10)?);
            values.extend(point2(bits, 11)?);
            values.push(real(40, bits.bd()?));
            values.push(real(50, bits.bd()?.to_degrees()));
            values.push(real(51, bits.bd()?.to_degrees()));
            values.push(int(73, i64::from(bits.bit()?)));
        }
        4 => {
            values.push(int(94, i64::from(bits.bl()?)));
            let rational = bits.bit()?;
            values.push(int(73, i64::from(rational)));
            values.push(int(74, i64::from(bits.bit()?)));
            let knots = bits.bl()?;
            let controls = bits.bl()?;
            values.push(int(95, i64::from(knots)));
            values.push(int(96, i64::from(controls)));
            for _ in 0..knots {
                values.push(real(40, bits.bd()?));
            }
            for _ in 0..controls {
                values.extend(point2(bits, 10)?);
                // DXF は重みを 42 に書く
                if rational {
                    values.push(real(42, bits.bd()?));
                }
            }
        }
        _ => return None,
    }
    Some(())
}
