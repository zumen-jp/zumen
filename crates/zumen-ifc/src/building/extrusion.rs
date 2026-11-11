//! IFC2X3 TC1・IFC4 ADD2 TC1・IFC4.3 ADD2 の IfcExtrudedAreaSolid の属性と ValidExtrusionDirection、
//! IfcSweptAreaSolid の SweptAreaType、IfcArbitraryClosedProfileDef の CurveDim・閉鎖に従う。
//! パラメーター付き断面は IfcParameterizedProfileDef・IfcRectangleProfileDef・IfcCircleProfileDef、
//! IfcAxis2Placement2D の LocationIs2D/RefDirIs2D と版ごとの IfcDirection の WHERE に従う。
//! 仕様の版ごとの参照と未評価の境界は docs/formats/ifc.md の「断面を共通の経路で取得する」にある。
//! 原点・軸・向き・深さは元の値。要素の配置・写像・単位換算・形の評価は行わない。

use zumen_building::{
    AreaProfile, AreaProfileKind, AxisPlacement2, AxisPlacement3, ClosedProfile, Coordinates,
    CurveGraph, CurveKind, Extrusion, IndexedSegment, ProfileExtrusion, Unit,
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

/// 要素の形の葉から取得した断面と押し出し。配置・写像・換算は未適用。
///
/// 全体の PartialEq/Hash は定義しない。意味の比較には definition と単位、
/// boundary の曲線の定義を使い、CurveSource と借用出所を含めない。
#[derive(Clone, Debug)]
pub struct ProfiledExtrusion<'f> {
    /// L1 に依存しない定義
    pub definition: ProfileExtrusion,
    /// 任意断面だけが持つ外輪。パラメーター付き断面から曲線を生成しない
    pub boundary: Option<CurveGraph>,
    /// 深さ・断面寸法・配置の座標の元の単位
    pub length_unit: Option<Unit>,
    /// 単位を含め直接読んだ元のレコード
    pub source: ExtrusionSource<'f>,
}

/// 押し出しの出所。同じ IfcFile の元のレコードを借用する
#[derive(Clone, Debug)]
pub struct ExtrusionSource<'f> {
    /// IfcExtrudedAreaSolid
    pub solid: &'f Record,
    /// 断面。extruded_area() では IfcArbitraryClosedProfileDef
    pub profile: &'f Record,
    /// 直接読んだ位置・点・向きなどの元のレコード。曲線内の出所は CurveGraph が持つ
    pub records: Vec<&'f Record>,
}

impl IfcFile {
    /// ProductShape の葉から、任意の閉じた外輪・矩形・円の断面と押し出しを取得する。
    ///
    /// 利用側で STEP 引数を読まず、元寸法・名前・2D/3D の配置・単位・出所に到達できる。
    /// 既存 extruded_area() の型・値・予算の契約は変更しない。
    /// 穴・I 形・中空の断面などは、種類と場所を診断し Unsupported を返す。
    /// 複合インスタンスは属性配置を解釈せず拒否する。評価や Building への統合ではない。
    pub fn extrusion<'f>(
        &'f self,
        node: &ShapeNode<'_>,
        cx: &mut Cx,
    ) -> Result<ProfiledExtrusion<'f>, Error> {
        cx.with_diagnostics_in_record_order(|cx| {
            let mut reader = Reader {
                file: self,
                cx,
                current: node.source.id().unwrap_or(0),
                records: Vec::new(),
            };
            reader.check(Kind::Work, 1)?;
            let solid = self
                .instance(reader.current)
                .ok_or_else(|| reader.invalid("source"))?;
            if !std::ptr::eq(solid, node.source) {
                return Err(reader.invalid("source"));
            }
            if solid.parts().len() != 1 {
                return Err(reader.unsupported_kind("complex"));
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
            let mut entered = 0;
            let result = reader.profiled(solid, version, &mut entered);
            for _ in 0..entered {
                reader.cx.leave();
            }
            result
        })
    }

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
    // IfcParameterizedProfileDef.Position / IfcPlacement.Location / IfcAxis2Placement2D.RefDirIs2D。
    // IfcDirection.MagnitudeGreaterZero は IFC4 以降だけ。2X3 の零比は未評価のまま保持する。
    fn placement2(&mut self, id: u64, version: Version) -> Result<AxisPlacement2, Error> {
        self.current = id;
        let record = self.take_typed(id)?;
        if !self.is(record, b"IFCAXIS2PLACEMENT2D")
            || record.parts().first().map_or(0, |p| p.args().len()) != 2
        {
            return Err(self.invalid("placement"));
        }
        let location_id = self.target(record, 1)?;
        let location = self.vector2(location_id, true, version)?;
        self.current = id;
        let reference_direction = match arg(record, 2) {
            Some(Value::Null) => None,
            _ => {
                let direction_id = self.target(record, 2)?;
                Some(self.vector2(direction_id, false, version)?)
            }
        };
        Ok(AxisPlacement2 {
            location,
            reference_direction,
        })
    }

    fn vector2(&mut self, id: u64, point: bool, version: Version) -> Result<[f64; 2], Error> {
        self.current = id;
        let record = self.take_typed(id)?;
        if !self.is(
            record,
            if point {
                b"IFCCARTESIANPOINT"
            } else {
                b"IFCDIRECTION"
            },
        ) || record.parts().first().map_or(0, |p| p.args().len()) != 1
        {
            return Err(self.invalid("type"));
        }
        let Some(Value::List(values)) = arg(record, 1) else {
            return Err(self.invalid("list"));
        };
        if values.len() != 2 {
            return Err(self.invalid("dimension"));
        }
        let result = [self.number(values.first())?, self.number(values.get(1))?];
        if !point && version != Version::Ifc2x3 && result == [0., 0.] {
            return Err(self.invalid("direction"));
        }
        Ok(result)
    }

    // 単位の意味の読み取りは既存 Lifter と共有し、走査・参照・文字の確保を先に数える。
    // 再帰する換算は対象にせず、既存の SI または一段の正の換算だけを保持する。
    fn length_unit(&mut self, version: Version, solid: &'f Record) -> Result<Option<Unit>, Error> {
        let mut project = None;
        for record in self.file.records() {
            // ヘッダー・番号なし・重複した番号も、走査から隠れて無予算にならないように数える。
            self.cx
                .check(Kind::Work, 1)
                .map_err(|stop| stopped(self.cx, stop, Some(record.span())))?;
            let Some(id) = record.id() else { continue };
            if !self
                .file
                .instance(id)
                .is_some_and(|first| std::ptr::eq(first, record))
            {
                continue;
            }
            self.current = id;
            self.check(Kind::Work, record.parts().len() as u64)?;
            let names = record
                .parts()
                .iter()
                .fold(0u64, |n, p| n.saturating_add(p.name().len() as u64));
            self.check(Kind::Work, names)?;
            if record.parts().iter().any(|p| {
                self.file
                    .bytes(p.name())
                    .eq_ignore_ascii_case(b"IFCPROJECT")
            }) {
                if record.parts().len() != 1 {
                    return Err(self.unsupported_kind("complex-unit-project"));
                }
                self.alloc(1, 32)?;
                self.records.push(record);
                project = Some(record);
                break;
            }
        }
        let Some(project) = project else {
            return Ok(None);
        };
        self.current = project.id().unwrap_or(0);
        if project.parts().first().map_or(0, |p| p.args().len()) != 9 {
            return Err(self.invalid("attributes"));
        }
        let assignment_id = match arg(project, 9) {
            Some(Value::Null) => return Ok(None),
            value => reference(value).ok_or_else(|| self.invalid("unit"))?,
        };
        self.current = assignment_id;
        let assignment = self.take_typed(assignment_id)?;
        if !self.is(assignment, b"IFCUNITASSIGNMENT")
            || assignment.parts().first().map_or(0, |p| p.args().len()) != 1
        {
            return Err(self.invalid("unit"));
        }
        let Some(Value::List(values)) = arg(assignment, 1) else {
            return Err(self.invalid("unit"));
        };
        self.check(Kind::Work, values.len() as u64)?;
        self.check(Kind::DecodedBytes, super::ENTITIES.len() as u64 * 256)?;
        let mut lifter = super::Lifter::new(self.file, version);
        let mut length = None;
        for value in values {
            self.current = assignment_id;
            let id = reference(Some(value)).ok_or_else(|| self.invalid("reference"))?;
            self.current = id;
            let record = self.take_typed(id)?;
            self.check(Kind::Work, record.span().len() as u64)?;
            // この取得操作が求める長さ以外の単位は評価しない。元の割り当てとレコードは残す。
            if !matches!(arg(record, 2),Some(Value::Enum(raw)) if self.file.bytes(raw.clone()).eq_ignore_ascii_case(b"LENGTHUNIT"))
            {
                continue;
            }
            self.check(Kind::DecodedBytes, 256 + record.span().len() as u64 * 12)?;
            if self.is(record, b"IFCCONVERSIONBASEDUNIT") {
                if record.parts().first().map_or(0, |p| p.args().len()) != 4 {
                    return Err(self.invalid("attributes"));
                }
                let factor_id = self.target(record, 4)?;
                self.current = factor_id;
                let factor = self.take_typed(factor_id)?;
                if !self.is(factor, b"IFCMEASUREWITHUNIT")
                    || factor.parts().first().map_or(0, |p| p.args().len()) != 2
                {
                    return Err(self.invalid("unit"));
                }
                let scalar = match arg(factor, 1) {
                    Some(Value::Typed(part)) if part.args().len() == 1 => part.args().first(),
                    scalar => scalar,
                };
                if self.number(scalar)? <= 0. {
                    return Err(self.invalid("unit"));
                }
                let base_id = self.target(factor, 2)?;
                self.current = base_id;
                let base = self.take_typed(base_id)?;
                if !self.is(base, b"IFCSIUNIT") {
                    return Err(self.unsupported_kind("unit-base"));
                }
                if !matches!(arg(base,2),Some(Value::Enum(raw)) if self.file.bytes(raw.clone()).eq_ignore_ascii_case(b"LENGTHUNIT"))
                {
                    return Err(self.invalid("unit"));
                }
                let bytes = (factor.span().len() as u64).saturating_add(base.span().len() as u64);
                self.check(Kind::Work, bytes)?;
                self.check(Kind::DecodedBytes, 512 + bytes.saturating_mul(12))?;
            }
            self.current = id;
            let unit = lifter.unit(id, self.cx);
            lifter.halted(self.cx, Some(record.span()))?;
            if let Some(unit) = unit {
                if unit.scale.is_some_and(|n| !n.is_finite() || n <= 0.) {
                    return Err(self.invalid("unit"));
                }
                if unit.kind == "LENGTHUNIT" {
                    if length.is_some() {
                        return Err(self.invalid("unit"));
                    }
                    length = Some(unit);
                }
            } else {
                return Err(self.unsupported_kind("unit"));
            }
        }
        self.current = solid.id().unwrap_or(0);
        Ok(length)
    }

    fn profiled(
        &mut self,
        solid: &'f Record,
        version: Version,
        entered: &mut u64,
    ) -> Result<ProfiledExtrusion<'f>, Error> {
        if !self.is(solid, b"IFCEXTRUDEDAREASOLID") {
            return Err(self.unsupported_kind("solid"));
        }
        if solid.parts().first().map_or(0, |p| p.args().len()) != 4 {
            return Err(self.invalid("attributes"));
        }
        let profile_id = self.target(solid, 1)?;
        self.current = profile_id;
        let profile = self.take_typed(profile_id)?;
        if self.is(profile, b"IFCARBITRARYCLOSEDPROFILEDEF") {
            if profile.parts().first().map_or(0, |p| p.args().len()) != 3 {
                return Err(self.invalid("attributes"));
            }
            self.current = solid.id().unwrap_or(0);
            let old = self.run(solid, version, entered)?;
            self.strict_attributes(&old.source.records)?;
            let length_unit = self.length_unit(version, solid)?;
            self.current = solid.id().unwrap_or(0);
            self.alloc(1, 320)?;
            let mut source = old.source;
            self.alloc(self.records.len(), 32)?;
            source.records.append(&mut self.records);
            return Ok(ProfiledExtrusion {
                definition: ProfileExtrusion {
                    profile: AreaProfile {
                        name: old.definition.profile.name,
                        position: None,
                        kind: AreaProfileKind::ArbitraryClosed {
                            boundary: old.boundary.root,
                        },
                    },
                    position: old.definition.position,
                    direction: old.definition.direction,
                    depth: old.definition.depth,
                },
                boundary: Some(old.boundary),
                length_unit,
                source,
            });
        }
        self.current = solid.id().unwrap_or(0);
        self.enter(entered)?;
        self.current = profile_id;
        self.enter(entered)?;
        let count = if self.is(profile, b"IFCRECTANGLEPROFILEDEF") {
            5
        } else if self.is(profile, b"IFCCIRCLEPROFILEDEF") {
            4
        } else {
            return Err(self.unsupported_kind("profile"));
        };
        if profile.parts().first().map_or(0, |p| p.args().len()) != count {
            return Err(self.invalid("attributes"));
        }
        if !matches!(arg(profile,1),Some(Value::Enum(raw)) if self.file.bytes(raw.clone()).eq_ignore_ascii_case(b"AREA"))
        {
            return Err(self.invalid("profile-type"));
        }
        let name = self.profile_name(profile)?;
        let position = if matches!(arg(profile, 3), Some(Value::Null)) {
            if version == Version::Ifc2x3 {
                return Err(self.invalid("placement"));
            }
            None
        } else {
            let id = self.target(profile, 3)?;
            Some(self.placement2(id, version)?)
        };
        self.current = profile_id;
        let first = self.number(arg(profile, 4))?;
        if first <= 0. {
            return Err(self.invalid("profile-dimension"));
        }
        let kind = if count == 5 {
            let second = self.number(arg(profile, 5))?;
            if second <= 0. {
                return Err(self.invalid("profile-dimension"));
            }
            AreaProfileKind::Rectangle {
                x_dim: first,
                y_dim: second,
            }
        } else {
            AreaProfileKind::Circle { radius: first }
        };
        let solid_id = solid.id().unwrap_or(0);
        self.current = solid_id;
        let sweep_position = if matches!(arg(solid, 2), Some(Value::Null)) {
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
        if direction[2] == 0. {
            return Err(self.invalid("direction"));
        }
        self.current = solid_id;
        let depth = self.number(arg(solid, 4))?;
        if depth <= 0. {
            return Err(self.invalid("depth"));
        }
        let records = core::mem::take(&mut self.records);
        let checked = self.strict_attributes(&records);
        self.records = records;
        checked?;
        let length_unit = self.length_unit(version, solid)?;
        self.current = solid_id;
        if length_unit.is_none() {
            self.cx.report(
                Diagnostic::new(codes::IFC_UNIT_MISSING)
                    .with("type", "IfcLengthMeasure")
                    .at(at_bytes(solid.span())),
            );
        }
        self.alloc(1, 320)?;
        Ok(ProfiledExtrusion {
            definition: ProfileExtrusion {
                profile: AreaProfile {
                    name,
                    position,
                    kind,
                },
                position: sweep_position,
                direction,
                depth,
            },
            boundary: None,
            length_unit,
            source: ExtrusionSource {
                solid,
                profile,
                records: core::mem::take(&mut self.records),
            },
        })
    }

    fn take_typed(&mut self, id: u64) -> Result<&'f Record, Error> {
        self.check(Kind::Work, 1)?;
        let record = self
            .file
            .instance(id)
            .ok_or_else(|| self.invalid("reference"))?;
        if record.parts().len() != 1 {
            return Err(self.unsupported_kind("complex"));
        }
        self.alloc(1, 32)?;
        self.records.push(record);
        Ok(record)
    }

    fn strict_attributes(&mut self, records: &[&Record]) -> Result<(), Error> {
        for record in records {
            self.current = record.id().unwrap_or(0);
            self.check(Kind::Work, 1)?;
            let count = if self.is(record, b"IFCCARTESIANPOINT") || self.is(record, b"IFCDIRECTION")
            {
                1
            } else if self.is(record, b"IFCAXIS2PLACEMENT2D") {
                2
            } else if self.is(record, b"IFCAXIS2PLACEMENT3D")
                || self.is(record, b"IFCARBITRARYCLOSEDPROFILEDEF")
            {
                3
            } else {
                continue;
            };
            if record.parts().first().map_or(0, |p| p.args().len()) != count {
                return Err(self.invalid("attributes"));
            }
        }
        Ok(())
    }

    fn unsupported_kind(&mut self, reason: &'static str) -> Error {
        let kind = self
            .file
            .instance(self.current)
            .map(|r| self.file.kind(r))
            .unwrap_or(b"");
        if let Err(error) = self
            .check(Kind::Work, kind.len() as u64)
            .and_then(|_| self.alloc(kind.len() + reason.len() + 1, 4))
        {
            return error;
        }
        let reason = format!("{reason}:{}", String::from_utf8_lossy(kind));
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
