//! IFC2X3 TC1・IFC4 ADD2 TC1・IFC4.3 ADD2 の IfcExtrudedAreaSolid の属性と ValidExtrusionDirection、
//! IfcSweptAreaSolid の SweptAreaType、IfcArbitraryClosedProfileDef の CurveDim・閉鎖に従う。
//! 原点・軸・向き・深さは元の値。要素の配置・写像・単位換算・形の評価は行わない。

use zumen_building::{
    AxisPlacement3, ClosedProfile, Coordinates, CurveGraph, CurveKind, Extrusion, IndexedSegment,
};

use super::{Version, arg, curve::parallel, decode, reference};
use crate::{
    Cx, Diagnostic, Error, IfcFile, Kind, Record, ShapeNode, Value, at_bytes, codes, stopped,
};

/// 既存の形の葉から取得した押し出し・断面と曲線。出所を意味の値から分ける。
///
/// boundary の length_unit は、深さ・立体の原点にも使う。配置・写像は未適用。
/// 全体の PartialEq/Hash は定義しない。意味の比較に boundary の CurveSource を含めない。
#[derive(Clone, Debug)]
pub struct ExtrudedArea<'f> {
    /// L1 に依存しない押し出しと断面の定義
    pub definition: Extrusion,
    /// 断面の外輪。共有する基の曲線を含む既存の CurveGraph
    pub boundary: CurveGraph,
    /// 元のレコード。定義と単位の意味値には含めない
    pub source: ExtrusionSource<'f>,
}

/// 押し出しの出所。同じ IfcFile の元のレコードを借用する
#[derive(Clone, Debug)]
pub struct ExtrusionSource<'f> {
    /// IfcExtrudedAreaSolid
    pub solid: &'f Record,
    /// IfcArbitraryClosedProfileDef
    pub profile: &'f Record,
    /// 直接読んだ位置・点・向きなどの元のレコード。曲線内の出所は CurveGraph が持つ
    pub records: Vec<&'f Record>,
}

impl IfcFile {
    /// ProductShape の形の葉から、押し出し・閉じた断面・曲線をまとめて必要時に取得する。
    ///
    /// 同じ IfcFile から取得した ShapeNode を渡す。既存の ShapeLinks と product_shape() は変更しない。
    /// IfcExtrudedAreaSolid と IfcArbitraryClosedProfileDef、2D の Polyline・IndexedPolyCurve・Circle が対象。
    /// 原点・軸の比・方向・深さ・単位を未変換で持ち、元の STEP 引数の読み取りは利用側で行う必要がない。
    /// 開いた輪・次元・方向・深さなどの不正は Invalid、閉鎖を検証しない曲線とほかの断面は Unsupported と診断を返す。
    /// 自己交差・面積・弧の非共線性を評価した結果ではない。停止・不正時に部分結果は返さない。
    pub fn extruded_area<'f>(
        &'f self,
        node: &ShapeNode<'_>,
        cx: &mut Cx,
    ) -> Result<ExtrudedArea<'f>, Error> {
        cx.with_diagnostics_in_record_order(|cx| {
            let mut reader = Reader {
                file: self,
                cx,
                current: node.source.id().unwrap_or(0),
                records: Vec::new(),
            };
            reader.check(Kind::Work, 1)?;
            let source = self
                .instance(reader.current)
                .ok_or_else(|| reader.invalid("source"))?;
            if !std::ptr::eq(source, node.source) {
                return Err(reader.invalid("source"));
            }
            if source.parts().len() != 1 {
                return Err(reader.invalid("type"));
            }
            let Some(version) = Version::of(self.schema()) else {
                reader.check(Kind::Work, self.schema().len() as u64)?;
                reader.alloc(self.schema().len(), 3)?;
                reader
                    .cx
                    .report(Diagnostic::new(codes::IFC_VERSION_UNSUPPORTED).with(
                        "schema",
                        String::from_utf8_lossy(self.schema()).into_owned(),
                    ));
                return Err(Error::Unsupported);
            };
            let mut entered = 0u64;
            let result = reader.run(source, version, &mut entered);
            for _ in 0..entered {
                reader.cx.leave();
            }
            result
        })
    }
}

struct Reader<'f, 'c, 'b> {
    file: &'f IfcFile,
    cx: &'c mut Cx<'b>,
    current: u64,
    records: Vec<&'f Record>,
}

impl<'f> Reader<'f, '_, '_> {
    fn check(&mut self, kind: Kind, count: u64) -> Result<(), Error> {
        self.cx.check(kind, count).map_err(|stop| {
            stopped(
                self.cx,
                stop,
                self.file.instance(self.current).map(Record::span),
            )
        })
    }
    fn alloc(&mut self, count: usize, bytes: u64) -> Result<(), Error> {
        self.check(Kind::DecodedBytes, (count as u64).saturating_mul(bytes))
    }
    fn enter(&mut self, entered: &mut u64) -> Result<(), Error> {
        self.cx.enter().map_err(|stop| {
            stopped(
                self.cx,
                stop,
                self.file.instance(self.current).map(Record::span),
            )
        })?;
        *entered += 1;
        Ok(())
    }
    fn invalid(&mut self, reason: &'static str) -> Error {
        let place = self
            .file
            .instance(self.current)
            .map_or_else(zumen_base::Ref::default, |r| at_bytes(r.span()))
            .with_id(format!("i{}", self.current));
        self.cx.report(
            Diagnostic::new(codes::IFC_EXTRUSION_INVALID)
                .with("reason", reason)
                .at(place),
        );
        Error::Invalid
    }
    fn unsupported(&mut self, reason: &'static str) -> Error {
        let place = self
            .file
            .instance(self.current)
            .map_or_else(zumen_base::Ref::default, |r| at_bytes(r.span()))
            .with_id(format!("i{}", self.current));
        self.cx.report(
            Diagnostic::new(codes::IFC_EXTRUSION_UNSUPPORTED)
                .with("reason", reason)
                .at(place),
        );
        Error::Unsupported
    }
    fn is(&self, r: &Record, name: &[u8]) -> bool {
        self.file.kind(r).eq_ignore_ascii_case(name)
    }
    fn take(&mut self, id: u64) -> Result<&'f Record, Error> {
        self.check(Kind::Work, 1)?;
        let record = self
            .file
            .instance(id)
            .ok_or_else(|| self.invalid("reference"))?;
        if record.parts().len() != 1 {
            return Err(self.invalid("type"));
        }
        self.alloc(1, 32)?;
        self.records.push(record);
        Ok(record)
    }
    fn target(&mut self, record: &Record, at: usize) -> Result<u64, Error> {
        reference(arg(record, at)).ok_or_else(|| self.invalid("reference"))
    }
    fn number(&mut self, value: Option<&Value>) -> Result<f64, Error> {
        let Some(Value::Number(raw)) = value else {
            return Err(self.invalid("number"));
        };
        self.check(Kind::Work, raw.len() as u64)?;
        std::str::from_utf8(self.file.bytes(raw.clone()))
            .ok()
            .and_then(|s| s.parse::<f64>().ok())
            .filter(|n| n.is_finite())
            .ok_or_else(|| self.invalid("number"))
    }
    fn vector(&mut self, id: u64, point: bool) -> Result<[f64; 3], Error> {
        self.current = id;
        let r = self.take(id)?;
        if !self.is(
            r,
            if point {
                b"IFCCARTESIANPOINT"
            } else {
                b"IFCDIRECTION"
            },
        ) {
            return Err(self.invalid("type"));
        }
        let Some(Value::List(v)) = arg(r, 1) else {
            return Err(self.invalid("list"));
        };
        if v.len() != 3 {
            return Err(self.invalid("dimension"));
        }
        let mut result = [0.; 3];
        for (slot, value) in result.iter_mut().zip(v) {
            *slot = self.number(Some(value))?;
        }
        if !point && result.iter().all(|&n| n == 0.) {
            return Err(self.invalid("direction"));
        }
        Ok(result)
    }
    fn placement(&mut self, id: u64) -> Result<AxisPlacement3, Error> {
        self.current = id;
        let r = self.take(id)?;
        if !self.is(r, b"IFCAXIS2PLACEMENT3D") {
            return Err(self.invalid("placement"));
        }
        let location_id = self.target(r, 1)?;
        let location = self.vector(location_id, true)?;
        self.current = id;
        let axis = if matches!(arg(r, 2), Some(Value::Null)) {
            None
        } else {
            let at = self.target(r, 2)?;
            Some(self.vector(at, false)?)
        };
        self.current = id;
        let reference_direction = if matches!(arg(r, 3), Some(Value::Null)) {
            None
        } else {
            let at = self.target(r, 3)?;
            Some(self.vector(at, false)?)
        };
        self.current = id;
        if axis.is_some() != reference_direction.is_some()
            || axis
                .zip(reference_direction)
                .is_some_and(|(a, b)| parallel(a, b))
        {
            return Err(self.invalid("placement"));
        }
        Ok(AxisPlacement3 {
            location,
            axis,
            reference_direction,
        })
    }
    fn profile_name(&mut self, record: &Record) -> Result<Option<String>, Error> {
        match arg(record, 2) {
            Some(Value::Null) => Ok(None),
            Some(Value::Text { raw, .. }) => {
                self.check(Kind::Work, raw.len() as u64)?;
                self.alloc(raw.len(), 12)?;
                let (name, invalid) = decode(self.file.bytes(raw.clone()));
                if invalid {
                    self.cx.report(
                        Diagnostic::new(codes::IFC_TEXT_INVALID).at(at_bytes(record.span())),
                    );
                }
                Ok(Some(name))
            }
            _ => Err(self.invalid("text")),
        }
    }
    fn closed(&mut self, graph: &CurveGraph) -> Result<(), Error> {
        let root = graph
            .item(graph.root)
            .ok_or_else(|| self.invalid("reference"))?;
        match &root.kind {
            CurveKind::Polyline(points) => {
                self.points2(points)?;
                if points.len() < 4 {
                    return Err(self.invalid("count"));
                }
                if points.first() != points.last() {
                    return Err(self.invalid("closure"));
                }
            }
            CurveKind::Indexed {
                points, segments, ..
            } => {
                self.points2(points)?;
                let (first, last) = if let Some(segments) = segments {
                    let ends = |s: &IndexedSegment| match s {
                        IndexedSegment::Line(v) => (v.first().copied(), v.last().copied()),
                        IndexedSegment::Arc(v) => (v.first().copied(), v.last().copied()),
                    };
                    self.check(Kind::Work, segments.len() as u64)?;
                    let first = segments.first().and_then(|s| ends(s).0);
                    let last = segments.last().and_then(|s| ends(s).1);
                    let point = |index: Option<u32>| {
                        index
                            .and_then(|i| i.checked_sub(1))
                            .and_then(|i| points.get(i as usize))
                    };
                    if points.len() < 3 {
                        return Err(self.invalid("count"));
                    }
                    (point(first), point(last))
                } else {
                    if points.len() < 4 {
                        return Err(self.invalid("count"));
                    }
                    (points.first(), points.last())
                };
                if first.is_none() || first != last {
                    return Err(self.invalid("closure"));
                }
            }
            CurveKind::Circle { position, .. } => {
                if position.location.dimension() != 2 {
                    return Err(self.invalid("dimension"));
                }
            }
            CurveKind::Line { .. } => return Err(self.invalid("closure")),
            _ => return Err(self.unsupported("boundary")),
        }
        Ok(())
    }
    fn points2(&mut self, points: &[Coordinates]) -> Result<(), Error> {
        for point in points {
            self.check(Kind::Work, 1)?;
            if point.dimension() != 2 {
                return Err(self.invalid("dimension"));
            }
        }
        Ok(())
    }
    fn run(
        &mut self,
        solid: &'f Record,
        version: Version,
        entered: &mut u64,
    ) -> Result<ExtrudedArea<'f>, Error> {
        self.enter(entered)?;
        self.check(Kind::Work, 1)?;
        if !self.is(solid, b"IFCEXTRUDEDAREASOLID") {
            return Err(self.unsupported("solid"));
        }
        let solid_id = solid.id().ok_or_else(|| self.invalid("source"))?;
        let profile_id = self.target(solid, 1)?;
        let profile = self.take(profile_id)?;
        self.current = profile_id;
        self.enter(entered)?;
        if !self.is(profile, b"IFCARBITRARYCLOSEDPROFILEDEF") {
            return Err(self.unsupported("profile"));
        }
        match arg(profile, 1) {
            Some(Value::Enum(raw))
                if self.file.bytes(raw.clone()).eq_ignore_ascii_case(b"AREA") => {}
            _ => return Err(self.invalid("profile-type")),
        }
        let name = self.profile_name(profile)?;
        let curve_id = self.target(profile, 3)?;
        self.current = curve_id;
        let boundary = self.file.curve(curve_id, self.cx)?;
        self.closed(&boundary)?;
        self.current = solid_id;
        let position = if matches!(arg(solid, 2), Some(Value::Null)) {
            if version == Version::Ifc2x3 {
                return Err(self.invalid("placement"));
            }
            None
        } else {
            let id = self.target(solid, 2)?;
            Some(self.placement(id)?)
        };
        self.current = solid_id;
        let direction_id = self.target(solid, 3)?;
        let direction = self.vector(direction_id, false)?;
        if direction.get(2) == Some(&0.) {
            return Err(self.invalid("direction"));
        }
        self.current = solid_id;
        let depth = self.number(arg(solid, 4))?;
        if depth <= 0. {
            return Err(self.invalid("depth"));
        }
        if boundary.length_unit.is_none() {
            self.cx.report(
                Diagnostic::new(codes::IFC_UNIT_MISSING)
                    .with("type", "IfcLengthMeasure")
                    .at(at_bytes(solid.span())),
            );
        }
        self.alloc(1, 256)?;
        Ok(ExtrudedArea {
            definition: Extrusion {
                profile: ClosedProfile {
                    name,
                    boundary: boundary.root,
                },
                position,
                direction,
                depth,
            },
            boundary,
            source: ExtrusionSource {
                solid,
                profile,
                records: core::mem::take(&mut self.records),
            },
        })
    }
}
