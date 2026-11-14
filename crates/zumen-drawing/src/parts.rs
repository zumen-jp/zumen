//! 寸法と表の定義から、表示の部品（線・円弧・矢印・文字）を作る。
//!
//! 描いた部品を持たない寸法と表をシーンで描く処理（決定 0184）と、寸法や表を編集して書き戻すときに
//! 匿名のブロックを作り直す処理（#314）で、同じ幾何の計算を使う。

use crate::{
    Axis, CellContent, CellFormat, Color, Dimension, DimensionKind, Horizontal, Inherit, Label,
    Length, Piece, SymbolKey, Table, Vertical,
};

/// 寸法の補助線のずれと突き出し。どちらも要素の空間の文書の単位で、0 以上の値
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct DimensionOffsets {
    /// 定義点から補助線の始まりまでの隙間（DXF の DIMEXO）
    pub offset: f64,
    /// 寸法線から補助線の終わりまでの突き出し（DXF の DIMEXE）
    pub extension: f64,
}

/// 寸法線（直線か反時計回りの円弧）
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum DimensionCurve {
    /// 線分
    Line([[f64; 2]; 2]),
    /// 反時計回りの円弧（角度は度）
    Arc {
        /// 中心
        center: [f64; 2],
        /// 半径
        radius: f64,
        /// 開始角（度）
        start: f64,
        /// 掃引角（度、0 より大きい）
        sweep: f64,
    },
}

/// 寸法線の端の矢印の位置と向き
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DimensionEnd {
    /// 矢印の先端の位置
    pub tip: [f64; 2],
    /// 矢印の先端へ向かう単位ベクトル
    pub toward_tip: [f64; 2],
    /// 1 本目の補助線の側なら `0`、2 本目の側なら `1`
    pub side: usize,
}

/// 定義点から求めた寸法の部品と測定値
#[derive(Clone, Debug, PartialEq)]
pub struct DimensionParts {
    /// 補助線の線分
    pub extension_lines: Vec<[[f64; 2]; 2]>,
    /// 寸法線
    pub curve: Option<DimensionCurve>,
    /// 寸法線の端（座標寸法では空、半径では 1 つ、ほかは 2 つ）
    pub ends: Vec<DimensionEnd>,
    /// 寸法値の文字の既定の位置
    pub middle: [f64; 2],
    /// 寸法値の文字の既定の向き（度）
    pub along: f64,
    /// 文字に書く測定値（長さは要素の空間の単位、角度は度）
    pub display_value: f64,
    /// 保存する生の測定値（長さは要素の空間の単位、角度はラジアン。DXF の 42）
    pub raw_measurement: f64,
}

/// 表のセルから求めた文字の配置
#[derive(Clone, Debug, PartialEq)]
pub struct TableTextPart {
    /// 行と列の番号
    pub cell: [usize; 2],
    /// セルの中の基準点
    pub position: [f64; 2],
    /// 回転（度、反時計回り）
    pub rotation: f64,
    /// 文字の高さ
    pub height: f64,
    /// 水平と垂直の位置合わせ
    pub alignment: (Horizontal, Vertical),
    /// セルの幅（折り返しの枠に使う）
    pub width: f64,
    /// 書式を解いた文字列（シーン用）
    pub plain: String,
    /// 書式の符号を含む元の文字列（MTEXT の書き戻し用）
    pub formatted: String,
    /// 文字スタイルの名前
    pub text_style: Option<String>,
    /// 文字の色
    pub text_color: Option<Inherit<Color>>,
}

/// 表のセルから求めた記号の配置
#[derive(Clone, Debug, PartialEq)]
pub struct TableSymbolPart {
    /// 行と列の番号
    pub cell: [usize; 2],
    /// セルの中央の位置
    pub position: [f64; 2],
    /// 回転（度、反時計回り）
    pub rotation: f64,
    /// 記号のキー
    pub key: SymbolKey,
}

/// 行と列から求めた表の部品
#[derive(Clone, Debug, PartialEq)]
pub struct TableParts {
    /// 枠の線分
    pub borders: Vec<[[f64; 2]; 2]>,
    /// 文字のセル
    pub texts: Vec<TableTextPart>,
    /// 記号のセル
    pub symbols: Vec<TableSymbolPart>,
}

/// 2 点の距離
fn distance([ax, ay]: [f64; 2], [bx, by]: [f64; 2]) -> f64 {
    libm::hypot(bx - ax, by - ay)
}

/// `a` から `b` への向き（度）
fn angle([ax, ay]: [f64; 2], [bx, by]: [f64; 2]) -> f64 {
    libm::atan2(by - ay, bx - ax).to_degrees()
}

/// 反時計回りに `start` から `end` まで進む角度（度、0 以上 360 以下）
fn ccw_sweep(start: f64, end: f64) -> f64 {
    let diff = (end - start).rem_euclid(360.0);
    if diff == 0.0 && end != start {
        360.0
    } else {
        diff
    }
}

/// 2 本の直線（2 点ずつ）の交点
fn intersection(
    [[ax, ay], [bx, by]]: [[f64; 2]; 2],
    [[cx, cy], [dx, dy]]: [[f64; 2]; 2],
) -> Option<[f64; 2]> {
    let [rx, ry] = [bx - ax, by - ay];
    let [sx, sy] = [dx - cx, dy - cy];
    let denominator = rx * sy - ry * sx;
    if denominator == 0.0 || !denominator.is_finite() {
        return None;
    }
    let factor = ((cx - ax) * sy - (cy - ay) * sx) / denominator;
    let point = [ax + rx * factor, ay + ry * factor];
    (point[0].is_finite() && point[1].is_finite()).then_some(point)
}

/// 定義点 `from` から寸法線上の点 `to` への補助線に、隙間と突き出しを当てる
fn extension_segment(
    [fx, fy]: [f64; 2],
    [tx, ty]: [f64; 2],
    offsets: DimensionOffsets,
) -> [[f64; 2]; 2] {
    let offset = if offsets.offset.is_finite() && offsets.offset > 0.0 {
        offsets.offset
    } else {
        0.0
    };
    let ext = if offsets.extension.is_finite() && offsets.extension > 0.0 {
        offsets.extension
    } else {
        0.0
    };
    if offset == 0.0 && ext == 0.0 {
        return [[fx, fy], [tx, ty]];
    }
    let len = distance([fx, fy], [tx, ty]);
    if !(len > 1e-12 && len.is_finite()) {
        return [[fx, fy], [tx, ty]];
    }
    let [ux, uy] = [(tx - fx) / len, (ty - fy) / len];
    let shift = offset.min(len);
    [
        [fx + ux * shift, fy + uy * shift],
        [tx + ux * ext, ty + uy * ext],
    ]
}

/// 長さ寸法と平行寸法の部品
fn linear_parts(
    first: [f64; 2],
    second: [f64; 2],
    line: [f64; 2],
    degrees: f64,
    oblique: f64,
    offsets: DimensionOffsets,
) -> Option<DimensionParts> {
    if !degrees.is_finite() || !oblique.is_finite() {
        return None;
    }
    let r = degrees.to_radians();
    let [dx, dy] = [libm::cos(r), libm::sin(r)];
    let [nx, ny] = [-dy, dx];
    let foot = |[px, py]: [f64; 2]| -> [f64; 2] {
        if oblique != 0.0 {
            let ob = oblique.to_radians();
            let [ex, ey] = [libm::cos(ob), libm::sin(ob)];
            if let Some(hit) = intersection(
                [[px, py], [px + ex, py + ey]],
                [line, [line[0] + dx, line[1] + dy]],
            ) {
                return hit;
            }
        }
        let k = nx * (line[0] - px) + ny * (line[1] - py);
        [px + nx * k, py + ny * k]
    };
    let (f1, f2) = (foot(first), foot(second));
    let span = distance(f1, f2);
    let ends = if span > 1e-12 {
        let [ux, uy] = [(f2[0] - f1[0]) / span, (f2[1] - f1[1]) / span];
        vec![
            DimensionEnd {
                tip: f1,
                toward_tip: [-ux, -uy],
                side: 0,
            },
            DimensionEnd {
                tip: f2,
                toward_tip: [ux, uy],
                side: 1,
            },
        ]
    } else {
        Vec::new()
    };
    let measured = (dx * (second[0] - first[0]) + dy * (second[1] - first[1])).abs();
    Some(DimensionParts {
        extension_lines: vec![
            extension_segment(first, f1, offsets),
            extension_segment(second, f2, offsets),
        ],
        curve: Some(DimensionCurve::Line([f1, f2])),
        ends,
        middle: [(f1[0] + f2[0]) / 2.0, (f1[1] + f2[1]) / 2.0],
        along: degrees,
        display_value: measured,
        raw_measurement: measured,
    })
}

/// 中心 `vertex`・開始角 `start`・掃引角 `sweep` の円弧の寸法部品を組み立てる
#[allow(clippy::too_many_arguments)]
fn arc_parts(
    vertex: [f64; 2],
    radius: f64,
    start: f64,
    sweep: f64,
    start_side: usize,
    ext_from: Option<([f64; 2], [f64; 2])>,
    offsets: DimensionOffsets,
    display_value: f64,
    raw_measurement: f64,
) -> DimensionParts {
    let s0 = start.to_radians();
    let s1 = (start + sweep).to_radians();
    let p0 = [
        vertex[0] + radius * libm::cos(s0),
        vertex[1] + radius * libm::sin(s0),
    ];
    let p1 = [
        vertex[0] + radius * libm::cos(s1),
        vertex[1] + radius * libm::sin(s1),
    ];
    let mut extension_lines = Vec::new();
    if let Some((from0, from1)) = ext_from {
        if distance(from0, p0) > 1e-9 {
            extension_lines.push(extension_segment(from0, p0, offsets));
        }
        if distance(from1, p1) > 1e-9 {
            extension_lines.push(extension_segment(from1, p1, offsets));
        }
    }
    let ends = if radius > 1e-12 && sweep > 1e-12 {
        vec![
            DimensionEnd {
                tip: p0,
                toward_tip: [libm::sin(s0), -libm::cos(s0)],
                side: start_side,
            },
            DimensionEnd {
                tip: p1,
                toward_tip: [-libm::sin(s1), libm::cos(s1)],
                side: 1 - start_side,
            },
        ]
    } else {
        Vec::new()
    };
    let half = (start + sweep / 2.0).to_radians();
    DimensionParts {
        extension_lines,
        curve: Some(DimensionCurve::Arc {
            center: vertex,
            radius,
            start,
            sweep,
        }),
        ends,
        middle: [
            vertex[0] + radius * libm::cos(half),
            vertex[1] + radius * libm::sin(half),
        ],
        along: (start + sweep / 2.0) - 90.0,
        display_value,
        raw_measurement,
    }
}

/// 寸法の定義点から、表示の部品と測定値を求める。座標か長さが有限でなければ `None`
pub fn dimension<P>(
    dimension: &Dimension<P>,
    xy: impl Fn(&P) -> Option<[f64; 2]>,
    len: impl Fn(&Length) -> Option<f64>,
    offsets: DimensionOffsets,
) -> Option<DimensionParts> {
    let pt = |p: &P| {
        let [x, y] = xy(p)?;
        (x.is_finite() && y.is_finite()).then_some([x, y])
    };
    match &dimension.kind {
        DimensionKind::Linear {
            first,
            second,
            line,
            rotation,
            oblique,
        } => linear_parts(
            pt(first)?,
            pt(second)?,
            pt(line)?,
            *rotation,
            *oblique,
            offsets,
        ),
        DimensionKind::Aligned {
            first,
            second,
            line,
            oblique,
        } => {
            let (a, b, l) = (pt(first)?, pt(second)?, pt(line)?);
            linear_parts(a, b, l, angle(a, b), *oblique, offsets)
        }
        DimensionKind::Radius {
            center,
            chord,
            leader,
        } => {
            let (c, p) = (pt(center)?, pt(chord)?);
            let extra = len(leader)?.max(0.0);
            if !extra.is_finite() {
                return None;
            }
            let r = distance(c, p);
            let (end_pt, ends) = if r > 1e-12 {
                let [ux, uy] = [(p[0] - c[0]) / r, (p[1] - c[1]) / r];
                let far = [p[0] + ux * extra, p[1] + uy * extra];
                let toward_tip = if extra > 0.0 { [-ux, -uy] } else { [ux, uy] };
                (
                    far,
                    vec![DimensionEnd {
                        tip: p,
                        toward_tip,
                        side: 0,
                    }],
                )
            } else {
                (p, Vec::new())
            };
            Some(DimensionParts {
                extension_lines: Vec::new(),
                curve: Some(DimensionCurve::Line([c, end_pt])),
                ends,
                middle: p,
                along: angle(c, p),
                display_value: r,
                raw_measurement: r,
            })
        }
        DimensionKind::Diameter {
            chord,
            opposite,
            leader,
        } => {
            let (p, opp) = (pt(chord)?, pt(opposite)?);
            let extra = len(leader)?.max(0.0);
            if !extra.is_finite() {
                return None;
            }
            let d = distance(opp, p);
            let (end_pt, ends) = if d > 1e-12 {
                let [ux, uy] = [(p[0] - opp[0]) / d, (p[1] - opp[1]) / d];
                let far = [p[0] + ux * extra, p[1] + uy * extra];
                let chord_dir = if extra > 0.0 { [-ux, -uy] } else { [ux, uy] };
                (
                    far,
                    vec![
                        DimensionEnd {
                            tip: opp,
                            toward_tip: [-ux, -uy],
                            side: 0,
                        },
                        DimensionEnd {
                            tip: p,
                            toward_tip: chord_dir,
                            side: 1,
                        },
                    ],
                )
            } else {
                (p, Vec::new())
            };
            Some(DimensionParts {
                extension_lines: Vec::new(),
                curve: Some(DimensionCurve::Line([opp, end_pt])),
                ends,
                middle: [(opp[0] + p[0]) / 2.0, (opp[1] + p[1]) / 2.0],
                along: angle(opp, p),
                display_value: d,
                raw_measurement: d,
            })
        }
        DimensionKind::Ordinate {
            origin,
            feature,
            leader_end,
            axis,
        } => {
            let (o, f, e) = (pt(origin)?, pt(feature)?, pt(leader_end)?);
            let signed = match axis {
                Axis::X => f[0] - o[0],
                Axis::Y => f[1] - o[1],
            };
            Some(DimensionParts {
                extension_lines: Vec::new(),
                curve: Some(DimensionCurve::Line([f, e])),
                ends: Vec::new(),
                middle: e,
                along: angle(f, e),
                display_value: signed.abs(),
                raw_measurement: signed.abs(),
            })
        }
        DimensionKind::Angular {
            vertex,
            first,
            second,
            arc,
        } => {
            let (v, a, b, through) = (pt(vertex)?, pt(first)?, pt(second)?, pt(arc)?);
            let radius = distance(v, through);
            let (a1, a2, am) = (angle(v, a), angle(v, b), angle(v, through));
            let forward = ccw_sweep(a1, a2);
            let (start, sweep, side, pair) = if (am - a1).rem_euclid(360.0) <= forward {
                (a1, forward, 0, (a, b))
            } else {
                (a2, ccw_sweep(a2, a1), 1, (b, a))
            };
            let ext = (offsets.offset > 0.0 || offsets.extension > 0.0).then_some(pair);
            Some(arc_parts(
                v,
                radius,
                start,
                sweep,
                side,
                ext,
                offsets,
                sweep,
                sweep.to_radians(),
            ))
        }
        DimensionKind::AngularLines { first, second, arc } => {
            let a = [pt(&first[0])?, pt(&first[1])?];
            let b = [pt(&second[0])?, pt(&second[1])?];
            let through = pt(arc)?;
            let v = intersection(a, b)?;
            let radius = distance(v, through);
            // 2 直線が頂点で作る 4 つの半直線を角度順に並べ、円弧の点が含まれる扇形を選ぶ
            let dir_a = angle(a[0], a[1]).rem_euclid(360.0);
            let dir_b = angle(b[0], b[1]).rem_euclid(360.0);
            let mut rays = [
                (dir_a, 0usize, a),
                ((dir_a + 180.0).rem_euclid(360.0), 0usize, a),
                (dir_b, 1usize, b),
                ((dir_b + 180.0).rem_euclid(360.0), 1usize, b),
            ];
            rays.sort_by(|left, right| left.0.total_cmp(&right.0));
            let am = angle(v, through).rem_euclid(360.0);
            let [r0, r1, r2, r3] = rays;
            let sectors = [
                (r0, r1, r1.0 - r0.0),
                (r1, r2, r2.0 - r1.0),
                (r2, r3, r3.0 - r2.0),
                (r3, r0, r0.0 + 360.0 - r3.0),
            ];
            let chosen = sectors
                .into_iter()
                .find(|(start, _, sweep)| (am - start.0).rem_euclid(360.0) <= *sweep)
                .unwrap_or((r0, r1, (r1.0 - r0.0).max(0.0)));
            let ((start_deg, side, seg0), (_, _, seg1), sweep) = chosen;
            let ray_anchor = |ray_deg: f64, [p, q]: [[f64; 2]; 2]| -> [f64; 2] {
                let rr = ray_deg.to_radians();
                let [ux, uy] = [libm::cos(rr), libm::sin(rr)];
                let dp = (p[0] - v[0]) * ux + (p[1] - v[1]) * uy;
                let dq = (q[0] - v[0]) * ux + (q[1] - v[1]) * uy;
                if dp >= dq && dp > 0.0 {
                    p
                } else if dq > 0.0 {
                    q
                } else {
                    v
                }
            };
            let ext = (offsets.offset > 0.0 || offsets.extension > 0.0).then(|| {
                (
                    ray_anchor(start_deg, seg0),
                    ray_anchor(start_deg + sweep, seg1),
                )
            });
            Some(arc_parts(
                v,
                radius,
                start_deg,
                sweep,
                side,
                ext,
                offsets,
                sweep,
                sweep.to_radians(),
            ))
        }
        DimensionKind::ArcLength {
            center,
            first,
            second,
            arc,
        } => {
            let (c, a, b, through) = (pt(center)?, pt(first)?, pt(second)?, pt(arc)?);
            let radius = distance(c, through);
            let (a1, a2, am) = (angle(c, a), angle(c, b), angle(c, through));
            let forward = ccw_sweep(a1, a2);
            let (start, sweep, side, pair) = if (am - a1).rem_euclid(360.0) <= forward {
                (a1, forward, 0, (a, b))
            } else {
                (a2, ccw_sweep(a2, a1), 1, (b, a))
            };
            let arc_len = distance(c, a) * sweep.to_radians();
            let ext = (offsets.offset > 0.0 || offsets.extension > 0.0).then_some(pair);
            Some(arc_parts(
                c, radius, start, sweep, side, ext, offsets, arc_len, arc_len,
            ))
        }
    }
}

/// 読める向きにした文字の回転（度）。右から左へ向くなら半回転する
pub fn readable_angle(degrees: f64) -> f64 {
    let turn = degrees.rem_euclid(360.0);
    if turn > 90.0 && turn <= 270.0 {
        degrees + 180.0
    } else {
        degrees
    }
}

/// 測定値の文字列。小数 `precision` 桁までに丸め、小数点より後の末尾の `0` を落とす
pub fn format_measurement(value: f64, precision: usize) -> String {
    let places = precision.min(8);
    let formatted = format!("{value:.places$}");
    let trimmed = if places > 0 {
        formatted.trim_end_matches('0').trim_end_matches('.')
    } else {
        formatted.as_str()
    };
    if trimmed == "-0" {
        "0".to_owned()
    } else {
        trimmed.to_owned()
    }
}

/// 寸法の種類に応じた、既定の寸法値文字列（MTEXT 用。直径の `∅` と角度の `°` は ASCII の `\U+XXXX` で表す）
pub fn default_dimension_mtext<P>(
    kind: &DimensionKind<P>,
    display_value: f64,
    precision: usize,
) -> String {
    let num = format_measurement(display_value, precision);
    match kind {
        DimensionKind::Radius { .. } => format!("R{num}"),
        DimensionKind::Diameter { .. } => format!("\\U+2205{num}"),
        DimensionKind::Angular { .. } | DimensionKind::AngularLines { .. } => {
            format!("{num}\\U+00B0")
        }
        DimensionKind::Linear { .. }
        | DimensionKind::Aligned { .. }
        | DimensionKind::ArcLength { .. }
        | DimensionKind::Ordinate { .. } => num,
    }
}

/// 寸法値の文字（[`Label`]）から、書き戻す MTEXT の文字列を作る。`Label::Hidden` なら `None`
pub fn dimension_mtext<P>(
    kind: &DimensionKind<P>,
    label: &Label,
    display_value: f64,
    precision: usize,
) -> Option<String> {
    match label {
        Label::Hidden => None,
        Label::Measurement => Some(default_dimension_mtext(kind, display_value, precision)),
        Label::Override { content, .. } => {
            let measured = default_dimension_mtext(kind, display_value, precision);
            let mut out = String::with_capacity(content.len() + measured.len());
            let mut rest = content.as_str();
            while let Some(ch) = rest.chars().next() {
                let after = rest.get(ch.len_utf8()..).unwrap_or_default();
                match ch {
                    '\\' => match after.chars().next() {
                        Some('X') => {
                            out.push_str("\\P");
                            rest = after.get(1..).unwrap_or_default();
                        }
                        Some(escaped) => {
                            out.push('\\');
                            out.push(escaped);
                            rest = after.get(escaped.len_utf8()..).unwrap_or_default();
                        }
                        None => {
                            out.push('\\');
                            rest = "";
                        }
                    },
                    '<' if after.starts_with('>') => {
                        out.push_str(&measured);
                        rest = after.get(1..).unwrap_or_default();
                    }
                    _ => {
                        out.push(ch);
                        rest = after;
                    }
                }
            }
            Some(out)
        }
    }
}

/// 塗った閉じた矢印の 3 頂点。幅は長さの 1/3
pub fn arrow_triangle(tip: [f64; 2], toward_tip: [f64; 2], size: f64) -> Option<[[f64; 2]; 3]> {
    if !(size > 0.0 && size.is_finite()) {
        return None;
    }
    let norm = libm::hypot(toward_tip[0], toward_tip[1]);
    if !(norm > 0.0 && norm.is_finite()) {
        return None;
    }
    let [dx, dy] = [toward_tip[0] / norm, toward_tip[1] / norm];
    let [bx, by] = [tip[0] - dx * size, tip[1] - dy * size];
    let half = size / 6.0;
    let [nx, ny] = [-dy * half, dx * half];
    Some([tip, [bx + nx, by + ny], [bx - nx, by - ny]])
}

/// 寸法線の端に矢印の代わりに描く 45 度の斜線
pub fn tick_line(tip: [f64; 2], toward_tip: [f64; 2], size: f64) -> Option<[[f64; 2]; 2]> {
    if !(size > 0.0 && size.is_finite()) {
        return None;
    }
    let norm = libm::hypot(toward_tip[0], toward_tip[1]);
    if !(norm > 0.0 && norm.is_finite()) {
        return None;
    }
    let [dx, dy] = [toward_tip[0] / norm, toward_tip[1] / norm];
    let inv_sqrt2 = core::f64::consts::FRAC_1_SQRT_2;
    let [tx, ty] = [(dx - dy) * inv_sqrt2, (dx + dy) * inv_sqrt2];
    let half = size / 2.0;
    Some([
        [tip[0] - tx * half, tip[1] - ty * half],
        [tip[0] + tx * half, tip[1] + ty * half],
    ])
}

/// 表の行と列から、枠の線分とセルの文字・記号の配置を求める。`origin` は挿入点の座標（シーンでは
/// `table.position`、DXF の `*T` ブロックでは `[0.0, 0.0]`）。`default_text_height` は、セルにも表の
/// スタイルにも文字の高さがないときの既定値（シーンでは `None`）
pub fn table<P>(
    table: &Table<P>,
    origin: [f64; 2],
    len: impl Fn(&Length) -> Option<f64>,
    default_text_height: Option<f64>,
) -> Option<TableParts> {
    if !(origin[0].is_finite() && origin[1].is_finite()) {
        return None;
    }
    let mut heights = Vec::with_capacity(table.row_heights.len());
    for h in &table.row_heights {
        let v = len(h)?;
        if !v.is_finite() {
            return None;
        }
        heights.push(v);
    }
    let mut widths = Vec::with_capacity(table.column_widths.len());
    for w in &table.column_widths {
        let v = len(w)?;
        if !v.is_finite() {
            return None;
        }
        widths.push(v);
    }
    let (rows, columns) = (heights.len(), widths.len());
    if rows == 0 || columns == 0 {
        return Some(TableParts {
            borders: Vec::new(),
            texts: Vec::new(),
            symbols: Vec::new(),
        });
    }
    let ys: Vec<f64> = core::iter::once(0.0)
        .chain(heights.iter().scan(0.0, |sum, h| {
            *sum += h;
            Some(*sum)
        }))
        .collect();
    let xs: Vec<f64> = core::iter::once(0.0)
        .chain(widths.iter().scan(0.0, |sum, w| {
            *sum += w;
            Some(*sum)
        }))
        .collect();
    let [dx, dy] = table.direction;
    let norm = libm::hypot(dx, dy);
    let d = if norm > 0.0 && norm.is_finite() {
        [dx / norm, dy / norm]
    } else {
        [1.0, 0.0]
    };
    let degrees = angle([0.0, 0.0], d);
    let at = |x: f64, y: f64| {
        [
            origin[0] + d[0] * x + d[1] * y,
            origin[1] + d[1] * x - d[0] * y,
        ]
    };
    let (width, height) = (
        xs.last().copied().unwrap_or(0.0),
        ys.last().copied().unwrap_or(0.0),
    );
    let mut borders = vec![
        [at(width, 0.0), at(width, height)],
        [at(0.0, height), at(width, height)],
    ];
    let mut texts = Vec::new();
    let mut symbols = Vec::new();
    let row_kind = |row: usize| -> Option<&CellFormat> {
        let formats = table.formats.as_deref()?;
        let title = usize::from(formats.title);
        Some(if formats.title && row == 0 {
            &formats.title_format
        } else if formats.header && row == title {
            &formats.header_format
        } else {
            &formats.data_format
        })
    };
    for row in 0..rows {
        for column in 0..columns {
            let Some(cell) = table
                .cells
                .get(row.saturating_mul(columns).saturating_add(column))
            else {
                continue;
            };
            if cell.covered {
                continue;
            }
            let [across, down] = cell.span;
            let end_column = column.saturating_add(across.max(1) as usize).min(columns);
            let end_row = row.saturating_add(down.max(1) as usize).min(rows);
            let (Some(&x0), Some(&x1), Some(&y0), Some(&y1)) = (
                xs.get(column),
                xs.get(end_column),
                ys.get(row),
                ys.get(end_row),
            ) else {
                continue;
            };
            borders.push([at(x0, y0), at(x1, y0)]);
            borders.push([at(x0, y0), at(x0, y1)]);
            let row_format = row_kind(row);
            match &cell.content {
                CellContent::Empty => {}
                CellContent::Text { content, spans } => {
                    let resolved_height = cell
                        .format
                        .text_height
                        .as_ref()
                        .or_else(|| row_format.and_then(|f| f.text_height.as_ref()))
                        .and_then(&len)
                        .or(default_text_height)
                        .filter(|h| h.is_finite() && *h > 0.0);
                    let Some(text_height) = resolved_height else {
                        continue;
                    };
                    let (h, v) = cell
                        .format
                        .alignment
                        .or_else(|| row_format.and_then(|f| f.alignment))
                        .unwrap_or((Horizontal::Center, Vertical::Middle));
                    let margin = text_height / 2.0;
                    let x = match h {
                        Horizontal::Left => x0 + margin,
                        Horizontal::Right => x1 - margin,
                        Horizontal::Center | Horizontal::Middle => (x0 + x1) / 2.0,
                    };
                    let y = match v {
                        Vertical::Top => y0 + margin,
                        Vertical::Bottom | Vertical::Baseline => y1 - margin,
                        Vertical::Middle => (y0 + y1) / 2.0,
                    };
                    let mut plain = String::new();
                    for span in spans {
                        match &span.piece {
                            Piece::Text(text) => plain.push_str(text),
                            Piece::Paragraph => plain.push(' '),
                            _ => {}
                        }
                    }
                    let text_style = cell
                        .format
                        .text_style
                        .clone()
                        .or_else(|| row_format.and_then(|f| f.text_style.clone()));
                    let text_color = cell
                        .format
                        .text_color
                        .or_else(|| row_format.and_then(|f| f.text_color));
                    texts.push(TableTextPart {
                        cell: [row, column],
                        position: at(x, y),
                        rotation: degrees + cell.rotation,
                        height: text_height,
                        alignment: (h, v),
                        width: (x1 - x0 - text_height).max(0.0),
                        plain,
                        formatted: content.clone(),
                        text_style,
                        text_color,
                    });
                }
                CellContent::Symbol(key) => {
                    symbols.push(TableSymbolPart {
                        cell: [row, column],
                        position: at((x0 + x1) / 2.0, (y0 + y1) / 2.0),
                        rotation: degrees + cell.rotation,
                        key: key.clone(),
                    });
                }
            }
        }
    }
    Some(TableParts {
        borders,
        texts,
        symbols,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{DimensionText, Format, ModelLength, ModelPoint, SpaceId, Span};

    const MODEL: SpaceId = SpaceId(0);

    fn pt(x: f64, y: f64) -> ModelPoint {
        ModelPoint { space: MODEL, x, y }
    }

    fn model_len(value: f64) -> Length {
        Length::Model(ModelLength {
            space: MODEL,
            value,
        })
    }

    #[test]
    fn angular_lines_selects_the_quadrant_containing_the_arc_point() {
        let make = |ax: f64, ay: f64| Dimension {
            kind: DimensionKind::AngularLines {
                first: [pt(-10.0, 0.0), pt(10.0, 0.0)],
                second: [
                    pt(-5.0, -5.0 * 3.0_f64.sqrt()),
                    pt(5.0, 5.0 * 3.0_f64.sqrt()),
                ],
                arc: pt(ax, ay),
            },
            measurement: None,
            label: Label::Measurement,
            text: DimensionText {
                position: None,
                moved: false,
                rotation: None,
                attachment: None,
            },
            style: "Standard".to_owned(),
            block: None,
        };
        let xy = |p: &ModelPoint| Some([p.x, p.y]);
        let len = |l: &Length| match l {
            Length::Model(m) => Some(m.value),
            Length::Paper(p) => Some(p.0),
        };
        let acute = dimension(&make(8.0, 2.0), xy, len, DimensionOffsets::default()).unwrap();
        assert!((acute.display_value - 60.0).abs() < 1e-9);
        assert!((acute.raw_measurement - core::f64::consts::FRAC_PI_3).abs() < 1e-9);

        let obtuse = dimension(&make(-2.0, 8.0), xy, len, DimensionOffsets::default()).unwrap();
        assert!((obtuse.display_value - 120.0).abs() < 1e-9);
        assert!((obtuse.raw_measurement - 2.0 * core::f64::consts::FRAC_PI_3).abs() < 1e-9);
    }

    #[test]
    fn format_measurement_keeps_trailing_zeros_before_decimal_point() {
        assert_eq!(format_measurement(10.0, 0), "10");
        assert_eq!(format_measurement(10.0, 2), "10");
        assert_eq!(format_measurement(10.5, 2), "10.5");
        assert_eq!(format_measurement(-0.0001, 2), "0");
    }

    #[test]
    fn dimension_mtext_replaces_measurement_placeholder_and_below_line_break() {
        let kind = DimensionKind::Linear {
            first: pt(0.0, 0.0),
            second: pt(25.0, 0.0),
            line: pt(0.0, 5.0),
            rotation: 0.0,
            oblique: 0.0,
        };
        let label = Label::Override {
            content: "W=<>\\X(参考)".to_owned(),
            spans: vec![Span {
                piece: Piece::Measurement,
                format: Format::default(),
            }],
        };
        assert_eq!(
            dimension_mtext(&kind, &label, 25.0, 2).as_deref(),
            Some("W=25\\P(参考)")
        );
        assert_eq!(dimension_mtext(&kind, &Label::Hidden, 25.0, 2), None);
        let _ = model_len(1.0);
    }
}
