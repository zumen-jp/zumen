//! LWPOLYLINE・SPLINE・HATCH のフィールドの符号化。

use super::*;

/// LWPOLYLINE のフィールド（R2000）。続くフィールドがあるかのフラグは、値があるかから選ぶ。膨らみ（42）が 1 つでもあれば、
/// すべての頂点の膨らみを書く（ないものは 0）。頂点ごとの幅（40・41）は、初めの頂点から続けて置く。
/// 頂点ごとの幅があるときは、表にない一定の幅を元のまま残す。閉じている（0x200）と線種の生成（0x100）は
/// 70 の 1 と 128 から書き、ほかのフラグのビットは元のまま写す。足した
/// LWPOLYLINE（`data` が空）では、どちらも 0 にする（決定 0056）
pub(super) fn lwpolyline(
    fields: &mut BitWriter,
    object: &Object,
    data: &[u8],
    start: usize,
) -> Option<()> {
    let (old, old_width) = if data.is_empty() {
        (0, 0.0)
    } else {
        let mut bits = Bits::with_release(data, Release::R2000);
        bits.seek(start);
        let old = bits.bs()?;
        (old, if old & 4 != 0 { bits.bd()? } else { 0.0 })
    };
    // 頂点ごとの x・y・始まりの幅・終わりの幅・膨らみ
    let mut vertices: Vec<[Option<f64>; 5]> = Vec::new();
    for value in &object.values {
        let slot = match value.code {
            10 => {
                vertices.push([None; 5]);
                0
            }
            20 => 1,
            40 => 2,
            41 => 3,
            42 => 4,
            _ => continue,
        };
        let Data::Real(v) = value.data else {
            return None;
        };
        let slot = vertices.last_mut()?.get_mut(slot)?;
        if slot.is_some() {
            return None;
        }
        *slot = Some(v);
    }
    let count = object.values.iter().filter(|value| value.code == 90);
    match count.map(|value| &value.data).collect::<Vec<_>>()[..] {
        [Data::Int(count)] if usize::try_from(*count).ok()? == vertices.len() => {}
        _ => return None,
    }
    // DXF の 70 は、閉じている（1）と線種の生成（128）だけを受け付ける
    let flags70 = small(object, 70)?.unwrap_or(0);
    if flags70 & !(1 | 128) != 0 {
        return None;
    }
    let (closed, generation) = (flags70 & 1 != 0, flags70 & 128 != 0);
    let widths = vertices.iter().take_while(|v| v[2].is_some()).count();
    let (with, without) = vertices.split_at_checked(widths)?;
    if without.iter().any(|v| v[2].is_some() || v[3].is_some())
        || with.iter().any(|v| v[3].is_none())
    {
        return None;
    }
    let width = match (real(object, 43)?, widths) {
        (Some(_), 1..) => return None,
        (width, 0) => width.unwrap_or(0.0),
        (None, _) => old_width,
    };
    let elevation = real(object, 38)?;
    let thickness = real(object, 39)?;
    let extrusion = match [210, 220, 230].map(|code| real(object, code)) {
        [Some(None), Some(None), Some(None)] => None,
        [Some(Some(x)), Some(Some(y)), Some(Some(z))] => Some([x, y, z]),
        _ => return None,
    };
    let bulges = vertices.iter().any(|v| v[4].is_some());
    let mut flags = old & !(0x3F | 0x100 | 0x200);
    for (on, bit) in [
        (extrusion.is_some(), 1),
        (thickness.is_some(), 2),
        (width.to_bits() != 0, 4),
        (elevation.is_some(), 8),
        (bulges, 16),
        (widths > 0, 32),
        (generation, 0x100),
        (closed, 0x200),
    ] {
        if on {
            flags |= bit;
        }
    }
    fields.bs(flags);
    if flags & 4 != 0 {
        fields.bd(width);
    }
    for value in [elevation, thickness].into_iter().flatten() {
        fields.bd(value);
    }
    for value in extrusion.into_iter().flatten() {
        fields.bd(value);
    }
    let count = i32::try_from(vertices.len()).ok()?;
    fields.bl(count);
    if bulges {
        fields.bl(count);
    }
    if widths > 0 {
        fields.bl(i32::try_from(widths).ok()?);
    }
    let mut last: Option<(f64, f64)> = None;
    for vertex in &vertices {
        let (x, y) = (vertex[0]?, vertex[1]?);
        match last {
            None => {
                fields.rd(x);
                fields.rd(y);
            }
            Some((last_x, last_y)) => {
                fields.dd(x, last_x);
                fields.dd(y, last_y);
            }
        }
        last = Some((x, y));
    }
    if bulges {
        for vertex in &vertices {
            fields.bd(vertex[4].unwrap_or(0.0));
        }
    }
    for vertex in with {
        fields.bd(vertex[2]?);
        fields.bd(vertex[3]?);
    }
    Some(())
}

/// R2013 からの、制御点を書かないスプライン（場合が 1 で、フラグの 1 が立つ）のうち、読む側がフィット点のフィールドと
/// して置いたもののフィールド。ノットと制御点（10・40〜43）があれば `None`。フラグ・ノットの取り方・当てはめる許容差は元の
/// ビットを写し、次数（71）・接線（12・13。なければ 0）・フィット点（11）を書く
pub(super) fn fit_points(
    fields: &mut BitWriter,
    object: &Object,
    data: &[u8],
    start: usize,
    release: Release,
) -> Option<()> {
    let mut bits = Bits::with_release(data, release);
    bits.seek(start);
    bits.bl()?;
    let method_at = bits.pos();
    bits.bl()?;
    bits.bl()?;
    let method_end = bits.pos();
    bits.bl()?;
    let tolerance_at = bits.pos();
    bits.bd()?;
    let tolerance_end = bits.pos();
    if object
        .values
        .iter()
        .any(|value| matches!(value.code, 10 | 20 | 30 | 40 | 41 | 42 | 43))
    {
        return None;
    }
    let mut fit: Vec<[Option<f64>; 3]> = Vec::new();
    for value in &object.values {
        let slot = match value.code {
            11 => {
                fit.push([None; 3]);
                0
            }
            21 => 1,
            31 => 2,
            _ => continue,
        };
        let Data::Real(v) = value.data else {
            return None;
        };
        let slot = fit.last_mut()?.get_mut(slot)?;
        if slot.is_some() {
            return None;
        }
        *slot = Some(v);
    }
    if integer(object, 74)?.unwrap_or(0) != i64::try_from(fit.len()).ok()? {
        return None;
    }
    fields.bl(1);
    fields.copy(data, method_at..method_end)?;
    fields.bl(i32::try_from(integer(object, 71)??).ok()?);
    fields.copy(data, tolerance_at..tolerance_end)?;
    for code in [12, 13] {
        match [code, code + 10, code + 20].map(|code| real(object, code)) {
            [Some(None), Some(None), Some(None)] => [0.0; 3].map(|v| fields.bd(v)),
            [Some(Some(x)), Some(Some(y)), Some(Some(z))] => [x, y, z].map(|v| fields.bd(v)),
            _ => return None,
        };
    }
    fields.bl(i32::try_from(fit.len()).ok()?);
    for point in &fit {
        for v in point {
            fields.bd((*v)?);
        }
    }
    Some(())
}

/// R2013 からの、制御点を書かないスプライン（場合が 1 で、フラグの 1 が立つ）のフィールド（決定 0103）。読む側は、3 次で
/// フィット点が 2 つのものを、両端の点と接線から Bézier の制御点に直して置く。その形（3 次・ノットが
/// 0 と 1 の 4 つずつ・重みのない 4 つの制御点・許容差が 1e-9 と 1e-10）の値だけを受け付け、両端の制御点を
/// フィット点に、両端の制御点との差の 3 倍を接線に戻して書く。両端の 2 つの制御点が変わらなければ、元の
/// 接線のビットを写す。フラグ・ノットの取り方・当てはめる許容差は元のビットを写す
pub(super) fn fit_bezier(
    fields: &mut BitWriter,
    object: &Object,
    data: &[u8],
    start: usize,
    release: Release,
) -> Option<()> {
    let mut bits = Bits::with_release(data, release);
    bits.seek(start);
    bits.bl()?;
    let method_at = bits.pos();
    bits.bl()?;
    bits.bl()?;
    let method_end = bits.pos();
    (bits.bl()? == 3).then_some(())?;
    let tolerance_at = bits.pos();
    bits.bd()?;
    let tangents_at = bits.pos();
    let point =
        |bits: &mut Bits| -> Option<[f64; 3]> { Some([bits.bd()?, bits.bd()?, bits.bd()?]) };
    let (start_tangent, end_tangent) = (point(&mut bits)?, point(&mut bits)?);
    (bits.bl()? == 2).then_some(())?;
    let (first, last) = (point(&mut bits)?, point(&mut bits)?);
    // 値は、読む側が置く形でなければならない
    let codes: Vec<_> = object
        .values
        .iter()
        .filter(|value| matches!(value.code, 10..=13 | 20..=23 | 30..=33 | 40..=44 | 71..=74))
        .map(|value| (value.code, &value.data))
        .collect();
    let int = |data: &Data| match data {
        Data::Int(value) => Some(*value),
        _ => None,
    };
    let float = |data: &Data| match data {
        Data::Real(value) => Some(*value),
        _ => None,
    };
    let [
        (71, degree),
        (72, knots),
        (73, controls),
        (74, fit),
        (42, knot_tolerance),
        (43, control_tolerance),
        rest @ ..,
    ] = codes.as_slice()
    else {
        return None;
    };
    ([int(degree)?, int(knots)?, int(controls)?, int(fit)?] == [3, 8, 4, 0]
        && float(knot_tolerance)? == 1e-9
        && float(control_tolerance)? == 1e-10)
        .then_some(())?;
    let [
        knots @ ..,
        p0x,
        p0y,
        p0z,
        p1x,
        p1y,
        p1z,
        p2x,
        p2y,
        p2z,
        p3x,
        p3y,
        p3z,
    ] = rest
    else {
        return None;
    };
    (knots.len() == 8
        && knots
            .iter()
            .zip([0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0])
            .all(|((code, data), knot)| *code == 40 && float(data) == Some(knot)))
    .then_some(())?;
    let mut points = Vec::new();
    for xyz in [
        [p0x, p0y, p0z],
        [p1x, p1y, p1z],
        [p2x, p2y, p2z],
        [p3x, p3y, p3z],
    ] {
        let [(10, x), (20, y), (30, z)] = xyz.map(|(code, data)| (*code, *data)) else {
            return None;
        };
        points.push([float(x)?, float(y)?, float(z)?]);
    }
    let [p0, p1, p2, p3] = points.as_slice() else {
        return None;
    };
    // 読む側と同じく、元の接線から求めた制御点
    let near = |end: [f64; 3], tangent: [f64; 3], sign: f64| -> [f64; 3] {
        let mut out = end;
        for (v, t) in out.iter_mut().zip(tangent) {
            *v += sign * t / 3.0;
        }
        out
    };
    let same =
        |a: &[f64; 3], b: &[f64; 3]| a.iter().zip(b).all(|(a, b)| a.to_bits() == b.to_bits());
    fields.bl(1);
    fields.copy(data, method_at..method_end)?;
    fields.bl(3);
    fields.copy(data, tolerance_at..tangents_at)?;
    for (end, inner, tangent, sign) in [(p0, p1, start_tangent, 1.0), (p3, p2, end_tangent, -1.0)] {
        let original = if sign > 0.0 { first } else { last };
        if same(end, &original) && same(inner, &near(original, tangent, sign)) {
            for v in tangent {
                fields.bd(v);
            }
        } else {
            for (inner, end) in inner.iter().zip(end) {
                fields.bd(sign * (inner - end) * 3.0);
            }
        }
    }
    fields.bl(2);
    for v in p0.iter().chain(p3) {
        fields.bd(*v);
    }
    Some(())
}

/// SPLINE のフィールド（R2000）。場合（scenario）は元のままとし、1 ならノットと制御点、2 ならフィット点を書く。
/// 場合に合わない値があれば `None`。有理・閉じている・周期的のフラグは、フラグ（70）があればその 4・1・2 を書き、
/// なければ元のビットを写す。重みがあるかのフラグは 41 があるかから選ぶ。接線（12・13）がなければ 0 を書く。足した SPLINE（`data` が空）は、
/// フィット点（11）を持てば場合を 2、持たなければ 1 とし、閉じている・周期的のフラグを 0、有理のフラグを
/// 重みがあるかにする（決定 0057・0059）。ノットの取り方（70 の 1024）を持つものは足さない（決定 0157）
pub(super) fn spline(
    fields: &mut BitWriter,
    object: &Object,
    data: &[u8],
    start: usize,
    release: Release,
) -> Option<()> {
    let added = data.is_empty();
    let r2013 = release >= Release::R2013;
    let (scenario, flags_at, method) = if added {
        // ノットの取り方（70 の 1024 とそのビット）は、足すスプラインでは書けない
        if integer(object, 70)?.unwrap_or(0) & 1024 != 0 {
            return None;
        }
        let fitted = object.values.iter().any(|value| value.code == 11);
        (if fitted { 2 } else { 1 }, 0, None)
    } else {
        let mut bits = Bits::with_release(data, release);
        bits.seek(start);
        let scenario = bits.bl()?;
        // R2013 からは、場合の後にスプラインのフラグとノットの取り方（BL）が続く。フラグの 1 が立つ制御点の
        // スプラインは、読む側がフィット点から制御点を求めて置くので、フィット点に戻して書く（決定 0103）
        let method = if r2013 {
            let at = bits.pos();
            let flags = bits.bl()?;
            bits.bl()?;
            if scenario == 1 && flags & 1 != 0 {
                // フィット点のフィールドとして読んだものは、フィット点で書き、Bézier の制御点に直したものは
                // フィット点に戻して書く
                if object.values.iter().any(|value| value.code == 11) {
                    return fit_points(fields, object, data, start, release);
                }
                return fit_bezier(fields, object, data, start, release);
            }
            Some(at..bits.pos())
        } else {
            None
        };
        bits.bl()?;
        (scenario, bits.pos(), method)
    };
    // 制御点（x・y・z・重み）と、フィット点（x・y・z）
    let mut controls: Vec<[Option<f64>; 4]> = Vec::new();
    let mut fit: Vec<[Option<f64>; 4]> = Vec::new();
    let mut knots = Vec::new();
    for value in &object.values {
        let (list, slot) = match value.code {
            10 => {
                controls.push([None; 4]);
                (&mut controls, 0)
            }
            20 => (&mut controls, 1),
            30 => (&mut controls, 2),
            41 => (&mut controls, 3),
            11 => {
                fit.push([None; 4]);
                (&mut fit, 0)
            }
            21 => (&mut fit, 1),
            31 => (&mut fit, 2),
            40 => {
                let Data::Real(knot) = value.data else {
                    return None;
                };
                knots.push(knot);
                continue;
            }
            _ => continue,
        };
        let Data::Real(v) = value.data else {
            return None;
        };
        let slot = list.last_mut()?.get_mut(slot)?;
        if slot.is_some() {
            return None;
        }
        *slot = Some(v);
    }
    for (code, len) in [(72, knots.len()), (73, controls.len()), (74, fit.len())] {
        if integer(object, code)?.unwrap_or(0) != i64::try_from(len).ok()? {
            return None;
        }
    }
    let present = |codes: &[i16]| {
        codes
            .iter()
            .any(|code| object.values.iter().any(|v| v.code == *code))
    };
    fields.bl(scenario);
    match method {
        Some(range) => fields.copy(data, range)?,
        // 足すのは制御点のスプラインだけとし、サンプルの図面と同じくフラグを 0、ノットの取り方を 15 とする
        None if r2013 => {
            (scenario == 1).then_some(())?;
            fields.bl(0);
            fields.bl(15);
        }
        None => {}
    }
    fields.bl(i32::try_from(integer(object, 71)??).ok()?);
    match scenario {
        1 => {
            if !fit.is_empty() || present(&[44, 12, 22, 32, 13, 23, 33]) {
                return None;
            }
            let weighted = controls.first().is_some_and(|c| c[3].is_some());
            if controls.iter().any(|c| c[3].is_some() != weighted) {
                return None;
            }
            match integer(object, 70)? {
                Some(flags) => {
                    for bit in [4, 1, 2] {
                        fields.bit(flags & bit != 0);
                    }
                }
                None if added => {
                    for on in [weighted, false, false] {
                        fields.bit(on);
                    }
                }
                None => fields.copy(data, flags_at..flags_at + 3)?,
            }
            for code in [42, 43] {
                fields.bd(real(object, code)??);
            }
            fields.bl(i32::try_from(knots.len()).ok()?);
            fields.bl(i32::try_from(controls.len()).ok()?);
            fields.bit(weighted);
            for knot in knots {
                fields.bd(knot);
            }
            for control in &controls {
                for v in &control[..3] {
                    fields.bd((*v)?);
                }
                if weighted {
                    fields.bd(control[3]?);
                }
            }
        }
        2 => {
            if !controls.is_empty() || !knots.is_empty() || present(&[42, 43]) {
                return None;
            }
            // フィット点のスプラインは、閉じている・周期的のフラグを書けない
            if integer(object, 70)?.unwrap_or(0) & 3 != 0 {
                return None;
            }
            fields.bd(real(object, 44)??);
            for code in [12, 13] {
                match [code, code + 10, code + 20].map(|code| real(object, code)) {
                    [Some(None), Some(None), Some(None)] => [0.0; 3].map(|v| fields.bd(v)),
                    [Some(Some(x)), Some(Some(y)), Some(Some(z))] => {
                        [x, y, z].map(|v| fields.bd(v))
                    }
                    _ => return None,
                };
            }
            fields.bl(i32::try_from(fit.len()).ok()?);
            for point in &fit {
                for v in &point[..3] {
                    fields.bd((*v)?);
                }
            }
        }
        _ => return None,
    }
    Some(())
}

/// HATCH のフィールド（R2000）。境界の辺では同じグループ・コードが何度も出るので、表の値を読む側が置く順に
/// 先頭から読み進めて書く。順が合わないか、境界の図形の数（97）が変わったら `None`。10・20 は 0 で
/// なければならず、押し出しの向きがなければ (0, 0, 1) を書く。角度は度からラジアンに直す
pub(super) fn hatch(fields: &mut BitWriter, old: &Object, new: &Object) -> Option<()> {
    let sources = |object: &Object| -> Vec<_> {
        object
            .values
            .iter()
            .filter(|value| value.code == 97)
            .cloned()
            .collect()
    };
    if sources(old) != sources(new) {
        return None;
    }
    let mut values = Cursor(
        new.values
            .iter()
            .filter(|value| HATCH.contains(&value.code))
            .collect::<Vec<_>>()
            .into_iter()
            .peekable(),
    );
    if values.real(10)?.to_bits() != 0 || values.real(20)?.to_bits() != 0 {
        return None;
    }
    fields.bd(values.real(30)?);
    let extrusion = if values.next_is(210) {
        [values.real(210)?, values.real(220)?, values.real(230)?]
    } else {
        [0.0, 0.0, 1.0]
    };
    for v in extrusion {
        fields.bd(v);
    }
    let Data::Text(name) = values.next(2)? else {
        return None;
    };
    let mut name = uncaret(name)?;
    name.push(0);
    fields.text(&name)?;
    let solid = values.bit(70)?;
    fields.bit(solid);
    fields.bit(values.bit(71)?);
    let paths = values.int(91)?;
    fields.bl(paths);
    let mut pixel_size = false;
    for _ in 0..paths {
        let flags = values.int(92)?;
        fields.bl(flags);
        pixel_size |= flags & 4 != 0;
        if flags & 2 == 0 {
            let edges = values.int(93)?;
            fields.bl(edges);
            for _ in 0..edges {
                hatch_edge(fields, &mut values)?;
            }
        } else {
            let bulges = values.bit(72)?;
            fields.bit(bulges);
            fields.bit(values.bit(73)?);
            let count = values.int(93)?;
            fields.bl(count);
            for _ in 0..count {
                fields.rd(values.real(10)?);
                fields.rd(values.real(20)?);
                if bulges {
                    fields.bd(values.real(42)?);
                }
            }
        }
        fields.bl(values.int(97)?);
    }
    for code in [75, 76] {
        fields.bs(i16::try_from(values.int(code)?).ok()?);
    }
    if !solid {
        fields.bd(values.real(52)?.to_radians());
        fields.bd(values.real(41)?);
        fields.bit(values.bit(77)?);
        let lines = i16::try_from(values.int(78)?).ok()?;
        fields.bs(lines);
        for _ in 0..lines {
            fields.bd(values.real(53)?.to_radians());
            for code in [43, 44, 45, 46] {
                fields.bd(values.real(code)?);
            }
            let dashes = i16::try_from(values.int(79)?).ok()?;
            fields.bs(dashes);
            for _ in 0..dashes {
                fields.bd(values.real(49)?);
            }
        }
    }
    if pixel_size {
        fields.bd(values.real(47)?);
    }
    let seeds = values.int(98)?;
    fields.bl(seeds);
    for _ in 0..seeds {
        fields.rd(values.real(10)?);
        fields.rd(values.real(20)?);
    }
    values.0.next().is_none().then_some(())
}

/// HATCH の境界の 1 つの辺。種類（72）が 1 なら線分、2 なら円弧、3 なら楕円弧、4 なら SPLINE
pub(super) fn hatch_edge(fields: &mut BitWriter, values: &mut Cursor<'_>) -> Option<()> {
    let kind = u8::try_from(values.int(72)?).ok()?;
    fields.rc(kind);
    let point = |fields: &mut BitWriter, values: &mut Cursor<'_>, code| -> Option<()> {
        fields.rd(values.real(code)?);
        fields.rd(values.real(code + 10)?);
        Some(())
    };
    match kind {
        1 => {
            point(fields, values, 10)?;
            point(fields, values, 11)?;
        }
        2 | 3 => {
            point(fields, values, 10)?;
            if kind == 3 {
                point(fields, values, 11)?;
            }
            fields.bd(values.real(40)?);
            for code in [50, 51] {
                fields.bd(values.real(code)?.to_radians());
            }
            fields.bit(values.bit(73)?);
        }
        4 => {
            fields.bl(values.int(94)?);
            let rational = values.bit(73)?;
            fields.bit(rational);
            fields.bit(values.bit(74)?);
            let knots = values.int(95)?;
            let controls = values.int(96)?;
            fields.bl(knots);
            fields.bl(controls);
            for _ in 0..knots {
                fields.bd(values.real(40)?);
            }
            for _ in 0..controls {
                point(fields, values, 10)?;
                if rational {
                    fields.bd(values.real(42)?);
                }
            }
        }
        _ => return None,
    }
    Some(())
}
