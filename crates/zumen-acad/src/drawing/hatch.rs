//! HATCH の境界・模様・グラデーションを、図面モデルの塗りにする。

use super::{Lifted, Lifter, Values, check_spline, invalid, non_planar, source};
use crate::db::{AcadDb, Data, Object, Origin};
use zumen_drawing::{
    Boundary, Edge, Fill, Gradient, Hatch, HatchLoop, Islands, Length, Pattern, PatternLine,
    PatternOrigin, Spline, Vertex,
};

impl Lifter<'_> {
    /// HATCH（DXF Reference「HATCH」「Boundary Path Data」「Pattern Data」）。境界の輪の数（91）からは、値を
    /// 並びの順に読む。標高（30）が 0 でなければ平面に移さない。SPLINE の辺を持つものは、まだ移さない。
    /// 境界を作った図形（330）は、輪ごとの数（97）と合わなければ読めないとする。密度の画素の大きさ（47）は
    /// 表示のための値なので移さない。グラデーション（450 が
    /// 1）は、色の RGB（421）だけを移し、色の番号（63）は移さない（推定。docs/formats/dxf.md）
    pub(super) fn hatch<P: Copy>(
        &self,
        db: &AcadDb,
        object: &Object,
        point: &impl Fn(f64, f64) -> P,
        length: &impl Fn(f64) -> Length,
    ) -> Lifted<Hatch<P>> {
        // DWG の読み手は、SPLINE の辺の当てはめる点を置かない
        let fit_data = self.fit_data && !matches!(object.origin, Origin::Dwg { .. });
        let kind = object.kind.as_slice();
        if self.real(object, 30, Some(0.0))? != 0.0 {
            return Err(non_planar(kind));
        }
        let solid = match self.int(object, 70, 0)? {
            0 => false,
            1 => true,
            _ => return Err(invalid(kind, 70)),
        };
        let associative = self.int(object, 71, 0)? != 0;
        let start = object
            .values
            .iter()
            .position(|value| value.code == 91)
            .ok_or_else(|| invalid(kind, 91))?;
        let mut values = Values {
            kind,
            values: object.values.get(start..).unwrap_or_default(),
        };
        let mut loops = Vec::new();
        for _ in 0..values.count(91)? {
            let flags = values.int(92)?;
            let boundary = if flags & 2 != 0 {
                let bulges = values.int(72)? != 0;
                let closed = values.int(73)? != 0;
                let mut vertices = Vec::new();
                for _ in 0..values.count(93)? {
                    let at = point(values.real(10)?, values.real(20)?);
                    let bulge = if bulges && values.next() == Some(42) {
                        values.real(42)?
                    } else {
                        0.0
                    };
                    vertices.push(Vertex { point: at, bulge });
                }
                Boundary::Polyline { vertices, closed }
            } else {
                let mut edges = Vec::new();
                for _ in 0..values.count(93)? {
                    edges.push(self.hatch_edge(&mut values, fit_data, point, length)?);
                }
                Boundary::Edges(edges)
            };
            let count = values.count(97)?;
            let mut sources = Vec::new();
            while sources.len() < count && values.next() == Some(330) {
                let Data::Handle(handle) = values.take(330)? else {
                    return Err(invalid(kind, 330));
                };
                let found = db.get(*handle).ok_or_else(|| invalid(kind, 330))?;
                sources.push(source(found));
            }
            if sources.len() != count {
                return Err(invalid(kind, 97));
            }
            loops.push(HatchLoop {
                boundary,
                external: flags & 1 != 0,
                outermost: flags & 16 != 0,
                derived: flags & 4 != 0,
                text_box: flags & 8 != 0,
                sources,
            });
        }
        let islands = match values.int(75)? {
            0 => Islands::Nested,
            1 => Islands::Outer,
            2 => Islands::Ignore,
            _ => return Err(invalid(kind, 75)),
        };
        let origin = match values.int(76)? {
            0 => PatternOrigin::UserDefined,
            1 => PatternOrigin::Predefined,
            2 => PatternOrigin::Custom,
            _ => return Err(invalid(kind, 76)),
        };
        // 塗りでも、パターンの欄を書く形式がある。塗りなら読むだけにする
        let pattern = if values.next() == Some(52) {
            let angle = values.real(52)?;
            let scale = values.real(41)?;
            let double = values.int(77)? != 0;
            let mut lines = Vec::new();
            for _ in 0..values.count(78)? {
                let angle = values.real(53)?;
                let base = [values.real(43)?, values.real(44)?];
                let offset = [values.real(45)?, values.real(46)?];
                let mut dashes = Vec::new();
                for _ in 0..values.count(79)? {
                    dashes.push(values.real(49)?);
                }
                lines.push(PatternLine {
                    angle,
                    base,
                    offset,
                    dashes,
                });
            }
            Some(Pattern {
                name: self.string(object, 2)?.unwrap_or_default(),
                origin,
                angle,
                scale,
                double,
                lines,
            })
        } else {
            None
        };
        if values.next() == Some(47) {
            values.real(47)?;
        }
        let mut seeds = Vec::new();
        if values.next() == Some(98) {
            for _ in 0..values.count(98)? {
                seeds.push(point(values.real(10)?, values.real(20)?));
            }
        }
        let fill = match (self.int(object, 450, 0)?, solid, pattern) {
            (0, true, _) => Fill::Solid,
            (0, false, Some(pattern)) => Fill::Pattern(pattern),
            (0, false, None) => return Err(invalid(kind, 52)),
            (1, _, _) => Fill::Gradient(self.gradient(object)?),
            _ => return Err(invalid(kind, 450)),
        };
        Ok(Hatch {
            loops,
            islands,
            fill,
            associative,
            seeds,
        })
    }

    /// HATCH の境界の辺。種類（72）が 1 なら線分、2 なら円弧、3 なら楕円弧（長軸の端は中心からの向き）、4 なら
    /// SPLINE
    fn hatch_edge<P: Copy>(
        &self,
        values: &mut Values,
        fit_data: bool,
        point: &impl Fn(f64, f64) -> P,
        length: &impl Fn(f64) -> Length,
    ) -> Lifted<Edge<P>> {
        let kind = values.kind;
        let edge = match values.int(72)? {
            1 => Edge::Line {
                start: point(values.real(10)?, values.real(20)?),
                end: point(values.real(11)?, values.real(21)?),
            },
            2 => {
                let center = point(values.real(10)?, values.real(20)?);
                let radius = values.real(40)?;
                if radius <= 0.0 {
                    return Err(invalid(kind, 40));
                }
                Edge::Arc {
                    center,
                    radius: length(radius),
                    start: values.real(50)?,
                    end: values.real(51)?,
                    counterclockwise: values.int(73)? != 0,
                }
            }
            3 => {
                let [x, y] = [values.real(10)?, values.real(20)?];
                let [dx, dy] = [values.real(11)?, values.real(21)?];
                let ratio = values.real(40)?;
                if !(ratio > 0.0 && ratio <= 1.0) {
                    return Err(invalid(kind, 40));
                }
                Edge::Elliptic {
                    center: point(x, y),
                    major: point(x + dx, y + dy),
                    ratio,
                    start: values.real(50)?,
                    end: values.real(51)?,
                    counterclockwise: values.int(73)? != 0,
                }
            }
            // 次数（94）・有理（73）・周期的（74）・節点（95・40）・制御点（96・10・20）と、有理なら重み（42）。
            // 2010 以降の DXF は、当てはめる点（97・11・21）と、あれば接線（12・22・13・23）が続く
            4 => {
                let degree = u32::try_from(values.int(94)?).map_err(|_| invalid(kind, 94))?;
                let rational = values.int(73)? != 0;
                let periodic = values.int(74)? != 0;
                let knots = values.count(95)?;
                let controls = values.count(96)?;
                let mut spline = Spline {
                    degree,
                    knots: Vec::new(),
                    control: Vec::new(),
                    weights: Vec::new(),
                    fit: Vec::new(),
                    start_tangent: None,
                    end_tangent: None,
                    closed: false,
                    periodic,
                    fit_tolerance: None,
                    knot_parameterization: None,
                };
                for _ in 0..knots {
                    spline.knots.push(values.real(40)?);
                }
                for _ in 0..controls {
                    spline
                        .control
                        .push(point(values.real(10)?, values.real(20)?));
                    if rational {
                        spline.weights.push(values.real(42)?);
                    }
                }
                if fit_data {
                    let fit = values.count(97)?;
                    for _ in 0..fit {
                        spline.fit.push(point(values.real(11)?, values.real(21)?));
                    }
                    if values.next() == Some(12) {
                        spline.start_tangent = Some([values.real(12)?, values.real(22)?]);
                    }
                    if values.next() == Some(13) {
                        spline.end_tangent = Some([values.real(13)?, values.real(23)?]);
                    }
                }
                check_spline(kind, &spline)?;
                Edge::Spline(spline)
            }
            _ => return Err(invalid(kind, 72)),
        };
        Ok(edge)
    }

    /// HATCH のグラデーション（450〜470）。回転（460）はラジアンなので度に直す。1 色（452 が 1）なら、色の
    /// 濃さ（462）を持つ
    fn gradient(&self, object: &Object) -> Lifted<Gradient> {
        let kind = object.kind.as_slice();
        let start = object
            .values
            .iter()
            .position(|value| value.code == 450)
            .unwrap_or_default();
        let mut colors = Vec::new();
        for value in object.values.iter().skip(start) {
            match (value.code, &value.data) {
                (421, Data::Int(rgb @ 0..=0xFF_FFFF)) => {
                    let [_, r, g, b] = (*rgb as u32).to_be_bytes();
                    colors.push([r, g, b]);
                }
                (421, _) => return Err(invalid(kind, 421)),
                _ => {}
            }
        }
        let tint = match self.int(object, 452, 0)? {
            0 => None,
            _ => Some(self.real(object, 462, None)?),
        };
        Ok(Gradient {
            name: self.string(object, 470)?.unwrap_or_default(),
            angle: self.real(object, 460, Some(0.0))?.to_degrees(),
            shift: self.real(object, 461, Some(0.0))?,
            colors,
            tint,
        })
    }
}
