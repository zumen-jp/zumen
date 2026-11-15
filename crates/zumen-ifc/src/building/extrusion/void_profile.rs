//! IfcArbitraryProfileDefWithVoids の OuterCurve / InnerCurves と Informal Propositions。
//! 2D の閉じた輪を元のまま保持する。輪同士の交差・包含・接触と、合成の区間の接続は未評価。

use super::*;
use std::collections::BTreeSet;
use zumen_building::CurveTransition;

impl<'f> Reader<'f, '_, '_> {
    pub(super) fn void_profile(
        &mut self,
        profile: &'f Record,
        version: Version,
    ) -> Result<(AreaProfile, CurveGraph), Error> {
        self.shape_diagnostics = true;
        self.current = profile.id().unwrap_or(0);
        if profile.form() != crate::Form::Entity || profile.parts().len() != 1 {
            return Err(self.unsupported_kind("complex-profile"));
        }
        if profile.parts().first().map_or(0, |p| p.args().len()) != 4 {
            return Err(self.invalid("attributes"));
        }
        if !matches!(arg(profile, 1), Some(Value::Enum(raw)) if self.file.bytes(raw.clone()).eq_ignore_ascii_case(b"AREA"))
        {
            return Err(self.invalid("profile-type"));
        }
        let name = self.profile_name(profile)?;
        let outer = self.target(profile, 3)?;
        let Some(Value::List(values)) = arg(profile, 4) else {
            return Err(self.invalid("list"));
        };
        if values.is_empty() {
            return Err(self.invalid("count"));
        }
        self.check(Kind::Work, values.len() as u64)?;
        self.check(Kind::Elements, values.len() as u64)?;
        // 参照の列と、重複を調べる木の節を確保する前に予約する。
        self.alloc(values.len(), 264)?;
        let mut inner = Vec::with_capacity(values.len());
        let mut seen = BTreeSet::new();
        for value in values {
            self.check(Kind::Work, 1)?;
            let id = reference(Some(value)).ok_or_else(|| self.invalid("reference"))?;
            if id == outer || !seen.insert(id) {
                return Err(self.invalid("profile-boundaries"));
            }
            inner.push(id);
        }
        let (graph, holes) =
            super::super::curve::profile_boundaries(self.file, outer, &inner, version, self.cx)?;
        // 曲線の各値は既存の経路で読む。この入口では、補助レコードを含む単純な形式と 2D を追加検査する。
        for item in &graph.items {
            self.current = item.source.instance.unwrap_or(0);
            self.check(Kind::Work, 1)?;
            for id in item.source.instance.iter().chain(&item.source.records) {
                self.current = *id;
                self.check(Kind::Work, 1)?;
                if self
                    .file
                    .instance(*id)
                    .is_none_or(|r| r.form() != crate::Form::Entity || r.parts().len() != 1)
                {
                    return Err(self.unsupported_kind("complex-boundary"));
                }
            }
            self.current = item.source.instance.unwrap_or(0);
            match &item.kind {
                CurveKind::Polyline(points) | CurveKind::Indexed { points, .. } => {
                    self.points2(points)?
                }
                CurveKind::Line { point, .. } if point.dimension() != 2 => {
                    return Err(self.invalid("dimension"));
                }
                CurveKind::Circle { position, .. } if position.location.dimension() != 2 => {
                    return Err(self.invalid("dimension"));
                }
                CurveKind::Unsupported(_) => return Err(self.unsupported_kind("boundary")),
                _ => {}
            }
        }
        self.void_boundary(&graph, graph.root)?;
        for &hole in &holes {
            self.void_boundary(&graph, hole)?;
        }
        self.current = profile.id().unwrap_or(0);
        self.alloc(1, 320)?;
        Ok((
            AreaProfile {
                name,
                position: None,
                kind: AreaProfileKind::ArbitraryWithVoids {
                    boundary: graph.root,
                    holes,
                },
            },
            graph,
        ))
    }

    fn void_boundary(&mut self, graph: &CurveGraph, at: CurveRef) -> Result<(), Error> {
        let item = graph.item(at).ok_or_else(|| self.invalid("reference"))?;
        self.current = item.source.instance.unwrap_or(0);
        if let CurveKind::Composite { segments, .. } = &item.kind {
            // IfcCompositeCurve.ClosedCurve の自然言語の説明は最後の Transition を閉鎖の申告とする。
            // 全区間で不連続を拒否するが、宣言どおりに点や接線がつながることはまだ評価しない。
            if segments.is_empty() {
                return Err(self.invalid("count"));
            }
            for segment in segments {
                self.check(Kind::Work, 1)?;
                if segment.transition == CurveTransition::Discontinuous {
                    return Err(self.invalid("closure"));
                }
            }
            Ok(())
        } else {
            self.closed_at(graph, at)
        }
    }

    pub(super) fn void_length_unit(
        &mut self,
        solid: &'f Record,
        version: Version,
    ) -> Result<Option<Unit>, Error> {
        let (unit, records) = face_length_unit(self.file, solid, version, self.cx)?;
        self.current = solid.id().unwrap_or(0);
        self.alloc(records.len(), 32)?;
        self.records.extend(records);
        Ok(unit)
    }
}
