//! IFC2X3 TC1・IFC4 ADD2 TC1・IFC4.3 ADD2 の IfcHalfSpaceSolid の AgreementFlag、
//! IfcBoxedHalfSpace の囲みの座標系、IfcPolygonalBoundedHalfSpace の境界の説明に従う。
//! 囲み箱は IFC2X3 では基準面、IFC4 以降では要素の座標系にある。元値を変換しない。

use super::*;
use zumen_building::{EnclosureCoordinates, HalfSpaceLimit, PlaneHalfSpace};

/// 平面の半空間の元の定義。境界の配置・単位・側の指定を適用せずに持つ
#[derive(Clone, Debug)]
pub struct HalfSpaceGeometry<'f> {
    /// L1 に依存しない定義
    pub definition: PlaneHalfSpace,
    /// 多角形で制限する場合だけ持つ境界。閉鎖と幾何の妥当性の全面的な検査結果ではない
    pub boundary: Option<CurveGraph>,
    /// 配置・箱の座標と寸法の元の単位
    pub length_unit: Option<Unit>,
    /// 直接読んだ元のレコード。意味の値から分ける
    pub source: HalfSpaceSource<'f>,
}

/// 半空間の借用出所。曲線内の出所は CurveGraph が持つ
#[derive(Clone, Debug)]
pub struct HalfSpaceSource<'f> {
    /// 半空間
    pub solid: &'f Record,
    /// 基準面
    pub surface: &'f Record,
    /// 直接読んだ配置・点・方向・囲み箱・単位。読み取り順に同じものも残す
    pub records: Vec<&'f Record>,
}

impl IfcFile {
    /// 形の葉から、無限・箱付き・多角形で制限する平面の半空間を必要時に取得する。
    ///
    /// 箱は計算用の囲みとして保持する。版による座標系の違いを保持し、面との積に変えない。
    /// 多角形の境界は 2D の Polyline・CompositeCurve と、IFC4.3 の IndexedPolyCurve。
    /// 合成の区間は Polyline または Line/Circle を切った曲線とする。閉鎖は Polyline と
    /// IndexedPolyCurve の始終点だけ確認し、合成の区間同士の接続・自己交差・基準面との
    /// 向きは未評価。停止・不正・未対応時に部分結果を返さない。
    pub fn half_space<'f>(
        &'f self,
        node: &ShapeNode<'_>,
        cx: &mut Cx,
    ) -> Result<HalfSpaceGeometry<'f>, Error> {
        cx.with_diagnostics_in_record_order(|cx| {
            let mut reader = Reader {
                file: self,
                cx,
                current: node.source.id().unwrap_or(0),
                records: Vec::new(),
                shape_diagnostics: true,
            };
            reader.check(Kind::Work, 1)?;
            let solid = self
                .instance(reader.current)
                .filter(|r| std::ptr::eq(*r, node.source))
                .ok_or_else(|| reader.invalid("half-space-source"))?;
            if solid.form() != crate::Form::Entity || solid.parts().len() != 1 {
                return Err(reader.unsupported_kind("complex"));
            }
            reader.check(Kind::Work, self.schema().len() as u64)?;
            let Some(version) = Version::of(self.schema()) else {
                reader.alloc(self.schema().len(), 3)?;
                reader
                    .cx
                    .report(Diagnostic::new(codes::IFC_VERSION_UNSUPPORTED).with(
                        "schema",
                        String::from_utf8_lossy(self.schema()).into_owned(),
                    ));
                return Err(Error::Unsupported);
            };
            let mut entered = 0;
            let result = reader.plane_half_space(solid, version, &mut entered);
            for _ in 0..entered {
                reader.cx.leave();
            }
            result
        })
    }
}

impl<'f> Reader<'f, '_, '_> {
    fn plane_half_space(
        &mut self,
        solid: &'f Record,
        version: Version,
        entered: &mut u64,
    ) -> Result<HalfSpaceGeometry<'f>, Error> {
        let count = if self.is(solid, b"IFCHALFSPACESOLID") {
            2
        } else if self.is(solid, b"IFCBOXEDHALFSPACE") {
            3
        } else if self.is(solid, b"IFCPOLYGONALBOUNDEDHALFSPACE") {
            4
        } else {
            return Err(self.unsupported_kind("half-space"));
        };
        if solid.parts().first().map_or(0, |p| p.args().len()) != count {
            return Err(self.invalid("attributes"));
        }
        self.enter(entered)?;
        self.check(Kind::Elements, 1)?;
        let solid_id = solid.id().unwrap_or(0);
        let agreement = match arg(solid, 2) {
            Some(Value::Enum(raw)) if self.file.bytes(raw.clone()).eq_ignore_ascii_case(b"T") => {
                true
            }
            Some(Value::Enum(raw)) if self.file.bytes(raw.clone()).eq_ignore_ascii_case(b"F") => {
                false
            }
            _ => return Err(self.invalid("agreement")),
        };
        self.current = self.target(solid, 1)?;
        self.enter(entered)?;
        self.check(Kind::Elements, 1)?;
        let surface = self.take_typed(self.current)?;
        if !self.is(surface, b"IFCPLANE") {
            return Err(self.unsupported_kind("base-surface"));
        }
        if surface.parts().first().map_or(0, |p| p.args().len()) != 1 {
            return Err(self.invalid("attributes"));
        }
        let plane_id = self.target(surface, 1)?;
        let plane = self.placement(plane_id)?;
        self.current = solid_id;
        let (limit, boundary) = match count {
            2 => (HalfSpaceLimit::Unbounded, None),
            3 => {
                self.current = self.target(solid, 3)?;
                self.check(Kind::Elements, 1)?;
                let enclosure = self.take_typed(self.current)?;
                if !self.is(enclosure, b"IFCBOUNDINGBOX")
                    || enclosure.parts().first().map_or(0, |p| p.args().len()) != 4
                {
                    return Err(self.invalid("enclosure"));
                }
                let corner_id = self.target(enclosure, 1)?;
                let corner = self.vector(corner_id, true)?;
                self.current = enclosure.id().unwrap_or(0);
                let mut dimensions = [0.; 3];
                for (i, value) in dimensions.iter_mut().enumerate() {
                    *value = self.number(arg(enclosure, i + 2))?;
                    if *value <= 0. {
                        return Err(self.invalid("enclosure-dimension"));
                    }
                }
                (
                    HalfSpaceLimit::Box {
                        corner,
                        dimensions,
                        coordinates: if version == Version::Ifc2x3 {
                            EnclosureCoordinates::BaseSurface
                        } else {
                            EnclosureCoordinates::Object
                        },
                    },
                    None,
                )
            }
            _ => {
                let position_id = self.target(solid, 3)?;
                let position = self.placement(position_id)?;
                self.current = solid_id;
                let boundary_id = self.target(solid, 4)?;
                self.current = boundary_id;
                let graph = self.file.curve(boundary_id, self.cx)?;
                self.half_space_boundary(&graph, version)?;
                (
                    HalfSpaceLimit::Polygonal {
                        position,
                        boundary: graph.root,
                    },
                    Some(graph),
                )
            }
        };
        let records = core::mem::take(&mut self.records);
        let checked = self.strict_attributes(&records);
        self.records = records;
        checked?;
        let (length_unit, units) = face_length_unit(self.file, solid, version, self.cx)?;
        self.current = solid_id;
        self.alloc(units.len(), 32)?;
        self.records.extend(units);
        if length_unit.is_none() {
            self.cx.report(
                Diagnostic::new(codes::IFC_UNIT_MISSING)
                    .with("type", "IfcLengthMeasure")
                    .at(at_bytes(solid.span())),
            );
        }
        self.alloc(1, 512)?;
        Ok(HalfSpaceGeometry {
            definition: PlaneHalfSpace {
                plane,
                agreement,
                limit,
            },
            boundary,
            length_unit,
            source: HalfSpaceSource {
                solid,
                surface,
                records: core::mem::take(&mut self.records),
            },
        })
    }

    fn half_space_boundary(&mut self, graph: &CurveGraph, version: Version) -> Result<(), Error> {
        let root = graph
            .item(graph.root)
            .ok_or_else(|| self.invalid("boundary"))?;
        match &root.kind {
            CurveKind::Polyline(_) => self.closed(graph)?,
            CurveKind::Indexed { .. } if version == Version::Ifc4x3 => self.closed(graph)?,
            CurveKind::Composite { segments, .. } => {
                for segment in segments {
                    self.check(Kind::Work, 1)?;
                    let item = graph
                        .item(segment.curve)
                        .ok_or_else(|| self.invalid("boundary"))?;
                    self.current = item.source.instance.unwrap_or(self.current);
                    match &item.kind {
                        CurveKind::Polyline(_) => {}
                        CurveKind::Trimmed { basis, .. } => {
                            let basis =
                                graph.item(*basis).ok_or_else(|| self.invalid("boundary"))?;
                            if !matches!(
                                basis.kind,
                                CurveKind::Line { .. } | CurveKind::Circle { .. }
                            ) {
                                self.current = basis.source.instance.unwrap_or(self.current);
                                return Err(self.unsupported_kind("boundary-basis"));
                            }
                        }
                        _ => return Err(self.unsupported_kind("boundary-segment")),
                    }
                }
            }
            _ => return Err(self.unsupported_kind("boundary")),
        }
        for item in &graph.items {
            self.check(Kind::Work, 1)?;
            self.current = item.source.instance.unwrap_or(self.current);
            // curve() が保持するすべての出所で、複合インスタンスをこの入口では受け付けない。
            for id in item.source.instance.iter().chain(&item.source.records) {
                self.check(Kind::Work, 1)?;
                self.current = *id;
                if self
                    .file
                    .instance(*id)
                    .is_none_or(|r| r.form() != crate::Form::Entity || r.parts().len() != 1)
                {
                    return Err(self.unsupported_kind("complex-boundary"));
                }
            }
            self.current = item.source.instance.unwrap_or(self.current);
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
                _ => {}
            }
        }
        Ok(())
    }
}
