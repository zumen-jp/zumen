//! 点・線・円・円弧・楕円・ポリライン・スプライン・SOLID を、図面モデルの形にする。
//! 厚さと線種の生成は、`dxf.entity` の拡張に控える（決定 0142）。

use super::{Lifted, Lifter, POINT, Values, check_spline, invalid, non_planar, text};
use crate::db::{Data, Object, Value};
use zumen_base::{Diagnostic, codes};
use zumen_drawing::{
    Boundary, Edge, Fill, Hatch, HatchLoop, Islands, KnotParameterization, Length, Polyline, Shape,
    Spline, Symbol, SymbolKey, Vertex,
};

impl Lifter<'_> {
    /// SOLID と TRACE（DXF Reference「SOLID」「TRACE」）。4 つの角（10〜13）を 1・2・4・3 の順に結んだ輪を
    /// 塗る。3 つ目と 4 つ目の角が同じなら三角形とする。厚さ（39）は、`dxf.entity` の拡張に控える（決定 0142）
    pub(super) fn solid<P>(
        &self,
        object: &Object,
        point: &impl Fn(f64, f64) -> P,
    ) -> Lifted<Hatch<P>> {
        let corner = |x: i16| self.planar_point(object, x, &|x, y| [x, y]);
        let [first, second, third] = [corner(10)?, corner(11)?, corner(12)?];
        let fourth = if object.first(13).is_some() {
            corner(13)?
        } else {
            third
        };
        let corners = if fourth == third {
            vec![first, second, third]
        } else {
            vec![first, second, fourth, third]
        };
        Ok(Hatch {
            loops: vec![HatchLoop {
                boundary: Boundary::Polyline {
                    vertices: corners
                        .into_iter()
                        .map(|[x, y]| Vertex {
                            point: point(x, y),
                            bulge: 0.0,
                        })
                        .collect(),
                    closed: true,
                },
                external: true,
                outermost: true,
                derived: false,
                text_box: false,
                sources: Vec::new(),
            }],
            islands: Islands::Nested,
            fill: Fill::Solid,
            associative: false,
            seeds: Vec::new(),
        })
    }

    /// POINT（DXF Reference「POINT」）。名前空間 `dxf.point` の記号として置き、点の表し方（`$PDMODE`）が 0 なら
    /// `point` を、ほかはその番号を名前にする。角度（50）は X 軸の角度（度）で、記号の回転にする。点の大きさ
    /// （`$PDSIZE`）は、まだ移さない（決定 0140）
    pub(super) fn point<P>(
        &self,
        object: &Object,
        point: &impl Fn(f64, f64) -> P,
    ) -> Lifted<Symbol<P>> {
        Ok(Symbol {
            key: SymbolKey {
                namespace: POINT,
                name: match self.point_mode {
                    0 => "point".to_owned(),
                    mode => mode.to_string(),
                },
            },
            position: self.planar_point(object, 10, point)?,
            scale: [1.0; 2],
            rotation: self.real(object, 50, Some(0.0))?,
            grid: None,
            attributes: Vec::new(),
            clip: None,
        })
    }

    /// LINE（DXF Reference「LINE」）。Z のないものは 0 とみなす（推定。docs/formats/dxf.md）
    pub(super) fn line<P>(
        &self,
        object: &Object,
        point: &impl Fn(f64, f64) -> P,
    ) -> Lifted<Shape<P>> {
        let end = |x: i16| -> Lifted<[f64; 3]> {
            let z = self.real(object, x + 20, Some(0.0))?;
            Ok([
                self.real(object, x, None)?,
                self.real(object, x + 10, None)?,
                z,
            ])
        };
        let (start, end) = (end(10)?, end(11)?);
        if start[2] != 0.0 || end[2] != 0.0 {
            return Err(non_planar(&object.kind));
        }
        Ok(Shape::Line {
            start: point(start[0], start[1]),
            end: point(end[0], end[1]),
        })
    }

    /// `code` で始まる点（X・Y と、あれば Z）の並び。Z が 0 でなければ平面に移さない
    pub(super) fn points<P>(
        &self,
        kind: &[u8],
        values: &[Value],
        code: i16,
        point: &impl Fn(f64, f64) -> P,
    ) -> Lifted<Vec<P>> {
        let mut values = Values { kind, values };
        let mut points = Vec::new();
        while let Some(next) = values.next() {
            if next != code {
                values.values = values.values.get(1..).unwrap_or_default();
                continue;
            }
            let (x, y) = (values.real(code)?, values.real(code + 10)?);
            if values.next() == Some(code + 20) && values.real(code + 20)? != 0.0 {
                return Err(non_planar(kind));
            }
            points.push(point(x, y));
        }
        Ok(points)
    }

    /// 半径（40）。正でなければ読めないとする
    pub(super) fn radius(&self, object: &Object) -> Lifted<f64> {
        let radius = self.real(object, 40, None)?;
        if radius > 0.0 {
            Ok(radius)
        } else {
            Err(invalid(&object.kind, 40))
        }
    }

    /// ELLIPSE（DXF Reference「ELLIPSE」）。長軸の端（11・21）は中心からの向きで、比（40）は 0 より大きく 1 以下。
    /// 始まりと終わりのパラメーター（41・42）はラジアンなので、度に直す
    pub(super) fn ellipse<P>(
        &self,
        object: &Object,
        point: &impl Fn(f64, f64) -> P,
    ) -> Lifted<Edge<P>> {
        let kind = object.kind.as_slice();
        if self.real(object, 30, Some(0.0))? != 0.0 || self.real(object, 31, Some(0.0))? != 0.0 {
            return Err(non_planar(kind));
        }
        let [x, y] = [self.real(object, 10, None)?, self.real(object, 20, None)?];
        let [dx, dy] = [self.real(object, 11, None)?, self.real(object, 21, None)?];
        if [dx, dy] == [0.0, 0.0] {
            return Err(invalid(kind, 11));
        }
        let ratio = self.real(object, 40, None)?;
        if !(ratio > 0.0 && ratio <= 1.0) {
            return Err(invalid(kind, 40));
        }
        Ok(Edge::Elliptic {
            center: point(x, y),
            major: point(x + dx, y + dy),
            ratio,
            start: self.real(object, 41, Some(0.0))?.to_degrees(),
            end: self
                .real(object, 42, Some(core::f64::consts::TAU))?
                .to_degrees(),
            counterclockwise: true,
        })
    }

    /// LWPOLYLINE（DXF Reference「LWPOLYLINE」）。頂点（10・20）ごとに、続く始まりと終わりの幅（40・41）と
    /// 膨らみ（42）を読む。頂点の数（90）と合わなければ読めないとする。標高（38）が 0 でなければ平面に移さない。
    /// 線種の生成の印（70 の 128）と厚さ（39）は、`dxf.entity` の拡張に控える（決定 0142）
    pub(super) fn lwpolyline<P: Copy>(
        &self,
        object: &Object,
        point: &impl Fn(f64, f64) -> P,
        length: &impl Fn(f64) -> Length,
    ) -> Lifted<Polyline<P>> {
        let kind = object.kind.as_slice();
        if self.real(object, 38, Some(0.0))? != 0.0 {
            return Err(non_planar(kind));
        }
        let mut corners: Vec<([f64; 2], [Option<f64>; 2], f64)> = Vec::new();
        let mut values = Values {
            kind,
            values: &object.values,
        };
        while let Some(code) = values.next() {
            match code {
                10 => {
                    let at = [values.real(10)?, values.real(20)?];
                    corners.push((at, [None; 2], 0.0));
                }
                40..=42 => {
                    let value = values.real(code)?;
                    let (_, widths, bulge) =
                        corners.last_mut().ok_or_else(|| invalid(kind, code))?;
                    match code {
                        40 => widths[0] = Some(value),
                        41 => widths[1] = Some(value),
                        _ => *bulge = value,
                    }
                }
                _ => values.values = values.values.get(1..).unwrap_or_default(),
            }
        }
        if usize::try_from(self.int(object, 90, -1)?).ok() != Some(corners.len()) {
            return Err(invalid(kind, 90));
        }
        let any_width = corners
            .iter()
            .any(|(_, widths, _)| widths.iter().any(Option::is_some));
        let mut vertex_widths = Vec::new();
        let mut vertices = Vec::new();
        for ([x, y], [start, end], bulge) in corners {
            if any_width {
                vertex_widths.push([length(start.unwrap_or(0.0)), length(end.unwrap_or(0.0))]);
            }
            vertices.push(Vertex {
                point: point(x, y),
                bulge,
            });
        }
        let width = match self.real(object, 43, Some(0.0))? {
            0.0 => None,
            width if width > 0.0 => Some(length(width)),
            _ => return Err(invalid(kind, 43)),
        };
        let flags = self.int(object, 70, 0)?;
        Ok(Polyline {
            vertices,
            closed: flags & 1 != 0,
            width,
            vertex_widths,
            line_type_generation: flags & 128 != 0,
        })
    }

    /// 古い POLYLINE と、その VERTEX（DXF Reference「POLYLINE」「VERTEX」）。印（70）の 1 は閉じている、8 は 3D
    /// の折れ線。曲線（2）かスプライン（4）で当てはめたものと、網（16）・多面（64）は、まだ移さない。頂点は VERTEX
    /// の点（10・20）と膨らみ（42）と、始まりと終わりの幅（40・41。なければ 0）。どの頂点の幅も同じなら一定の
    /// 幅にし、0 なら幅を持たない。POLYLINE の既定の幅（40・41）は、新しい頂点のためのものなので使わない。
    /// 標高（POLYLINE の 30）か頂点の Z（30）が 0 でなければ平面に移さない。線種の生成（70 の 128）は折れ線の値にし
    /// （決定 0157）、厚さ（39）は `dxf.entity` の拡張に控える（決定 0142）
    pub(super) fn polyline<P: Copy>(
        &self,
        object: &Object,
        vertices: &[&Object],
        point: &impl Fn(f64, f64) -> P,
        length: &impl Fn(f64) -> Length,
    ) -> Lifted<Polyline<P>> {
        let kind = object.kind.as_slice();
        let flags = self.int(object, 70, 0)?;
        if flags & (2 | 4 | 16 | 64) != 0 {
            return Err(Box::new(
                Diagnostic::new(codes::DXF_LIFT_DROPPED).with("kind", text(kind)),
            ));
        }
        if self.real(object, 30, Some(0.0))? != 0.0 {
            return Err(non_planar(kind));
        }
        let mut corners = Vec::new();
        for vertex in vertices {
            let widths = [
                self.real(vertex, 40, Some(0.0))?,
                self.real(vertex, 41, Some(0.0))?,
            ];
            if widths.iter().any(|width| *width < 0.0) {
                return Err(invalid(&vertex.kind, 40));
            }
            if self.real(vertex, 30, Some(0.0))? != 0.0 {
                return Err(non_planar(kind));
            }
            let at = point(self.real(vertex, 10, None)?, self.real(vertex, 20, None)?);
            corners.push((at, widths, self.real(vertex, 42, Some(0.0))?));
        }
        let first = corners.first().map(|(_, widths, _)| *widths);
        let uniform = first.filter(|[start, end]| {
            start == end
                && corners
                    .iter()
                    .all(|(_, widths, _)| *widths == [*start, *end])
        });
        let (width, vertex_widths) = match uniform {
            Some([width, _]) => ((width > 0.0).then(|| length(width)), Vec::new()),
            None => (
                None,
                corners
                    .iter()
                    .map(|(_, [start, end], _)| [length(*start), length(*end)])
                    .collect(),
            ),
        };
        Ok(Polyline {
            vertices: corners
                .into_iter()
                .map(|(point, _, bulge)| Vertex { point, bulge })
                .collect(),
            closed: flags & 1 != 0,
            width,
            vertex_widths,
            line_type_generation: flags & 128 != 0,
        })
    }

    /// SPLINE（DXF Reference「SPLINE」）。印（70）の 1 は閉じている、2 は周期的、4 は有理。次数（71）・節点
    /// （40）・制御点（10・20・30）・重み（41）・当てはめる点（11・21・31）・接線（12・13）・当てはめの許容差（44）と、
    /// 節点の取り方（[`knot_parameterization`]）を持つ（決定 0157）。節点・制御点・当てはめる点の数（72〜74）と
    /// 合わなければ読めないとする。節点と制御点の許容差（42・43）は、`dxf.entity` の拡張に控える（決定 0142）
    pub(super) fn spline<P>(
        &self,
        object: &Object,
        point: &impl Fn(f64, f64) -> P,
    ) -> Lifted<Spline<P>> {
        let kind = object.kind.as_slice();
        let flags = self.int(object, 70, 0)?;
        let degree = u32::try_from(self.int(object, 71, -1)?).map_err(|_| invalid(kind, 71))?;
        let reals = |code: i16| -> Lifted<Vec<f64>> {
            object
                .values
                .iter()
                .filter(|value| value.code == code)
                .map(|value| match value.data {
                    Data::Real(value) if value.is_finite() => Ok(value),
                    _ => Err(invalid(kind, code)),
                })
                .collect()
        };
        let tangent = |code: i16| -> Lifted<Option<[f64; 2]>> {
            if object.first(code).is_none() {
                return Ok(None);
            }
            if self.real(object, code + 20, Some(0.0))? != 0.0 {
                return Err(non_planar(kind));
            }
            Ok(Some([
                self.real(object, code, None)?,
                self.real(object, code + 10, None)?,
            ]))
        };
        let spline = Spline {
            degree,
            knots: reals(40)?,
            control: self.points(kind, &object.values, 10, point)?,
            weights: reals(41)?,
            fit: self.points(kind, &object.values, 11, point)?,
            start_tangent: tangent(12)?,
            end_tangent: tangent(13)?,
            closed: flags & 1 != 0,
            periodic: flags & 2 != 0,
            fit_tolerance: match object.first(44) {
                None => None,
                Some(Data::Real(tolerance)) if tolerance.is_finite() && *tolerance >= 0.0 => {
                    Some(*tolerance)
                }
                Some(_) => return Err(invalid(kind, 44)),
            },
            knot_parameterization: knot_parameterization(flags),
        };
        for (code, count) in [
            (72, spline.knots.len()),
            (73, spline.control.len()),
            (74, spline.fit.len()),
        ] {
            if object.first(code).is_some()
                && usize::try_from(self.int(object, code, -1)?).ok() != Some(count)
            {
                return Err(invalid(kind, code));
            }
        }
        check_spline(kind, &spline)?;
        Ok(spline)
    }
}

/// SPLINE の印（70）の、当てはめる点で決まるスプライン（1024）の節点の取り方。32 は弦の長さ、64 は弦の長さの
/// 平方根、128 は等しい間隔、256 は元の節点。DXF Reference の表にない印で、AutoCAD が書く値による（推定。試料で
/// 128 を確かめた。決定 0157）。DWG の読み手も、R2013 からの節点の取り方を同じ印にして置く
pub(crate) fn knot_parameterization(flags: i64) -> Option<KnotParameterization> {
    if flags & 1024 == 0 {
        return None;
    }
    match flags & (32 | 64 | 128 | 256) {
        32 => Some(KnotParameterization::Chord),
        64 => Some(KnotParameterization::SquareRoot),
        128 => Some(KnotParameterization::Uniform),
        256 => Some(KnotParameterization::Custom),
        _ => None,
    }
}
