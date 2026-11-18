//! 新しい非連動のハッチング。DXF Reference「HATCH」「Boundary Path Data」「Pattern Data」の順に並べる。

use super::{Born, Frame, Value, int, positive, real, text};
use zumen_base::{Cx, Ref};
use zumen_drawing::{Boundary, Edge, Fill, Hatch, Islands, PatternOrigin, Shape};
use zumen_sdk::Error;

impl Born {
    /// 拒否した追加の、条件に当たるフィールドと値。API が扱わない境界を Rust から渡した場合も知らせる。
    pub(crate) fn hatch_reason<P>(&self, shape: &Shape<P>) -> (String, String) {
        let Shape::Hatch(hatch) = shape else {
            return ("geometry".into(), "not_hatch".into());
        };
        if !self.version.is_empty() && !self.since(b"AC1012") {
            return (
                "version".into(),
                String::from_utf8_lossy(&self.version).into_owned(),
            );
        }
        if hatch.associative {
            return ("geometry/associative".into(), "true".into());
        }
        if hatch.loops.is_empty() {
            return ("geometry/loops".into(), "0".into());
        }
        for (index, path) in hatch.loops.iter().enumerate() {
            let at = format!("geometry/loops/{index}");
            if path.derived {
                return (format!("{at}/derived"), "true".into());
            }
            if path.text_box {
                return (format!("{at}/text_box"), "true".into());
            }
            if !path.sources.is_empty() {
                return (format!("{at}/sources"), path.sources.len().to_string());
            }
            match &path.boundary {
                Boundary::Polyline { vertices, closed } => {
                    if !closed {
                        return (format!("{at}/boundary/closed"), "false".into());
                    }
                    if vertices.len() < 2 {
                        return (format!("{at}/boundary/points"), vertices.len().to_string());
                    }
                }
                Boundary::Edges(edges) => {
                    if let [Edge::Arc { start, end, .. }] = edges.as_slice() {
                        if (*end - *start).abs() != 360.0 {
                            return (format!("{at}/boundary/sweep"), (end - start).to_string());
                        }
                    } else {
                        return (format!("{at}/boundary"), "edges".into());
                    }
                }
            }
        }
        if let Fill::Gradient(_) = &hatch.fill {
            return ("geometry/fill".into(), "gradient".into());
        }
        if let Fill::Pattern(pattern) = &hatch.fill {
            if pattern.name.is_empty() || pattern.name.chars().any(char::is_control) {
                return ("geometry/pattern/name".into(), pattern.name.clone());
            }
            if !pattern.scale.is_finite() || pattern.scale <= 0.0 {
                return ("geometry/pattern/scale".into(), pattern.scale.to_string());
            }
            if pattern.lines.is_empty() || pattern.lines.len() > i16::MAX as usize {
                return (
                    "geometry/pattern/lines".into(),
                    pattern.lines.len().to_string(),
                );
            }
            for (index, line) in pattern.lines.iter().enumerate() {
                if line.offset == [0.0, 0.0] {
                    return (
                        format!("geometry/pattern/lines/{index}/offset"),
                        "[0,0]".into(),
                    );
                }
                if line.dashes.len() > i16::MAX as usize {
                    return (
                        format!("geometry/pattern/lines/{index}/dashes"),
                        line.dashes.len().to_string(),
                    );
                }
            }
        }
        // 座標の系や長さ、既存の画層・スタイルなどが書けない場合。個々の無効な数値は API の検証でも返す。
        (
            "geometry".into(),
            "unrepresentable_coordinates_pattern_or_style".into(),
        )
    }
    /// 閉じた折れ線と円の輪、単色と明示された模様だけを書く。境界への参照や勾配は新しく作らない。
    pub(super) fn hatching<P>(
        &self,
        values: &mut Vec<Value>,
        frame: &impl Frame<P>,
        hatch: &Hatch<P>,
    ) -> Option<()> {
        if (!self.version.is_empty() && !self.since(b"AC1012"))
            || hatch.associative
            || hatch.loops.is_empty()
        {
            return None;
        }
        let pattern = match &hatch.fill {
            Fill::Solid => None,
            Fill::Pattern(pattern)
                if !pattern.name.is_empty()
                    && !pattern.name.chars().any(char::is_control)
                    && pattern.scale.is_finite()
                    && pattern.scale > 0.0
                    && !pattern.lines.is_empty()
                    && pattern.lines.len() <= i16::MAX as usize
                    && frame.hatch_pattern_supported() =>
            {
                Some(pattern)
            }
            _ => return None,
        };
        values.extend([
            text(100, "AcDbHatch"),
            real(10, 0.0)?,
            real(20, 0.0)?,
            real(30, 0.0)?,
            // 押し出しの向き。AutoCAD 2027 は、DXF の HATCH に 210 がないと図面を開かない（#684）
            real(210, 0.0)?,
            real(220, 0.0)?,
            real(230, 1.0)?,
            self.string(2, pattern.map_or("SOLID", |pattern| pattern.name.as_str()))?,
            int(70, i64::from(pattern.is_none())),
            int(71, 0),
            int(91, i32::try_from(hatch.loops.len()).ok()?.into()),
        ]);
        for boundary in &hatch.loops {
            if boundary.derived || boundary.text_box || !boundary.sources.is_empty() {
                return None;
            }
            let flags = i64::from(boundary.external) | (i64::from(boundary.outermost) << 4);
            match &boundary.boundary {
                Boundary::Polyline { vertices, closed } if *closed && vertices.len() >= 2 => {
                    // 各頂点の bulge を必ず書き、DWG と DXF の並びをそろえる。
                    let bulges = vertices.iter().any(|vertex| vertex.bulge != 0.0);
                    values.extend([
                        int(92, flags | 2),
                        int(72, i64::from(bulges)),
                        int(73, 1),
                        int(93, i32::try_from(vertices.len()).ok()?.into()),
                    ]);
                    for vertex in vertices {
                        let [x, y] = frame.point(&vertex.point)?;
                        values.extend([real(10, x)?, real(20, y)?]);
                        if bulges {
                            values.push(real(42, vertex.bulge)?);
                        }
                    }
                }
                Boundary::Edges(edges) => {
                    let [
                        Edge::Arc {
                            center,
                            radius,
                            start,
                            end,
                            counterclockwise,
                        },
                    ] = edges.as_slice()
                    else {
                        return None;
                    };
                    if !start.is_finite() || !end.is_finite() || (*end - *start).abs() != 360.0 {
                        return None;
                    }
                    let [x, y] = frame.point(center)?;
                    values.extend([
                        int(92, flags),
                        int(93, 1),
                        int(72, 2),
                        real(10, x)?,
                        real(20, y)?,
                        real(40, positive(frame, radius)?)?,
                        real(50, *start)?,
                        real(51, *end)?,
                        int(73, i64::from(*counterclockwise)),
                    ]);
                }
                _ => return None,
            }
            values.push(int(97, 0));
        }
        values.push(int(
            75,
            match hatch.islands {
                Islands::Nested => 0,
                Islands::Outer => 1,
                Islands::Ignore => 2,
            },
        ));
        values.push(int(
            76,
            pattern.map_or(1, |pattern| match pattern.origin {
                PatternOrigin::UserDefined => 0,
                PatternOrigin::Predefined => 1,
                PatternOrigin::Custom => 2,
            }),
        ));
        if let Some(pattern) = pattern {
            values.extend([
                real(52, pattern.angle)?,
                real(41, pattern.scale)?,
                int(77, i64::from(pattern.double)),
                int(78, i16::try_from(pattern.lines.len()).ok()?.into()),
            ]);
            for line in &pattern.lines {
                if line.offset == [0.0, 0.0] {
                    return None;
                }
                values.extend([
                    real(53, line.angle)?,
                    real(43, line.base[0])?,
                    real(44, line.base[1])?,
                    real(45, line.offset[0])?,
                    real(46, line.offset[1])?,
                    int(79, i16::try_from(line.dashes.len()).ok()?.into()),
                ]);
                for &dash in &line.dashes {
                    values.push(real(49, dash)?);
                }
            }
        }
        values.push(int(98, i32::try_from(hatch.seeds.len()).ok()?.into()));
        for seed in &hatch.seeds {
            let [x, y] = frame.point(seed)?;
            values.extend([real(10, x)?, real(20, y)?]);
        }
        Some(())
    }
}

/// 外側の輪の数は呼出し元で数え済み。内側の列をたどる前にも作業量を数える。
pub(super) fn work<P>(hatch: &Hatch<P>, cx: &mut Cx, at: &Ref) -> Result<(), Error> {
    for boundary in &hatch.loops {
        let count = match &boundary.boundary {
            Boundary::Polyline { vertices, .. } => vertices.len(),
            Boundary::Edges(edges) => edges.len(),
        }
        .saturating_add(boundary.sources.len());
        super::super::work(cx, count as u64, at)?;
    }
    if let Fill::Pattern(pattern) = &hatch.fill {
        super::super::work(
            cx,
            (pattern.name.len() as u64).saturating_add(pattern.lines.len() as u64),
            at,
        )?;
        for line in &pattern.lines {
            super::super::work(cx, line.dashes.len() as u64, at)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use zumen_base::{Budget, CancelToken, Diagnostics, Kind, Limit, Limits};
    use zumen_drawing::{Pattern, PatternLine};

    #[test]
    fn nested_pattern_data_is_charged_before_writing() {
        let hatch = |dashes| {
            Shape::Hatch(Hatch::<()> {
                loops: Vec::new(),
                islands: Islands::Nested,
                associative: false,
                seeds: Vec::new(),
                fill: Fill::Pattern(Pattern {
                    name: "TEST".into(),
                    origin: PatternOrigin::Custom,
                    angle: 0.0,
                    scale: 1.0,
                    double: false,
                    lines: vec![PatternLine {
                        angle: 0.0,
                        base: [0.0, 0.0],
                        offset: [0.0, 1.0],
                        dashes,
                    }],
                }),
            })
        };
        for (values, accepted) in [(Vec::new(), true), (vec![1.0; 64], false)] {
            let mut budget = Budget::new(Limits::default().with(Kind::Work, Limit::Finite(32)));
            let mut diagnostics = Diagnostics::new();
            let cancel = CancelToken::new();
            let result = super::super::work(
                &hatch(values),
                &mut Cx::new(&mut budget, &mut diagnostics, &cancel),
                &Ref::default(),
            );
            assert_eq!(result.is_ok(), accepted);
            if !accepted {
                assert!(matches!(result, Err(Error::Stopped(_))));
            }
        }
        let cancel = CancelToken::new();
        cancel.cancel();
        assert!(matches!(
            super::super::work(
                &hatch(Vec::new()),
                &mut Cx::new(&mut Budget::default(), &mut Diagnostics::new(), &cancel),
                &Ref::default()
            ),
            Err(Error::Stopped(_))
        ));
    }
}
