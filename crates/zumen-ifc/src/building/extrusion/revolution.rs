//! IFC2X3 TC1・IFC4 ADD2 TC1・IFC4.3 ADD2 の IfcRevolvedAreaSolid の属性と
//! AxisStartInXY/AxisDirectionInXY の説明、IfcAxis1Placement の属性・次元の説明に従う。
//! 軸と断面の交差や、丸みを含む輪郭は評価せず、元の値と出所を返す。

use super::*;
use zumen_building::{ProfileRevolution, RevolutionAxis};

/// 形の葉から取得した回転体。定義と出所を分け、配置・写像・単位換算は未適用
#[derive(Clone, Debug)]
pub struct RevolvedArea<'f> {
    /// L1 に依存しない回転体の定義
    pub definition: ProfileRevolution,
    /// 任意断面だけが持つ外輪。パラメーター付き断面の輪郭は生成しない
    pub boundary: Option<CurveGraph>,
    /// 断面・軸・配置の元の長さの単位
    pub length_unit: Option<Unit>,
    /// 直接読んだ元のレコード
    pub source: RevolutionSource<'f>,
}

/// 回転体の出所。同じ IfcFile の元のレコードを借用し、意味値には含めない
#[derive(Clone, Debug)]
pub struct RevolutionSource<'f> {
    /// IfcRevolvedAreaSolid
    pub solid: &'f Record,
    /// 回す面の断面
    pub profile: &'f Record,
    /// 直接読んだ配置・軸・点・方向・単位。曲線内の出所は CurveGraph が持つ
    pub records: Vec<&'f Record>,
}

impl IfcFile {
    /// ProductShape の葉から、回転体の断面・軸・角度・単位を必要時に取得する。
    ///
    /// 押し出しと同じ 5 種類の断面を扱う。元の配置と方向比を保持し、立体は評価しない。
    /// 軸は XY 平面内の原点と非ゼロの方向を要求する。角度は非負の有限値とし、単位が
    /// 解けたときだけ 1 回転以下を確認する。単位がなければ None と診断を返す。
    /// 軸と断面の内部の交差は未評価。停止・不正・未対応では部分結果を返さない。
    pub fn revolved_area<'f>(
        &'f self,
        node: &ShapeNode<'_>,
        cx: &mut Cx,
    ) -> Result<RevolvedArea<'f>, Error> {
        cx.with_diagnostics_in_record_order(|cx| {
            let mut reader = Reader {
                file: self,
                cx,
                current: node.source.id().unwrap_or(0),
                records: Vec::new(),
                revolution: true,
            };
            reader.check(Kind::Work, 1)?;
            let solid = self
                .instance(reader.current)
                .filter(|r| std::ptr::eq(*r, node.source))
                .ok_or_else(|| reader.invalid("revolution-source"))?;
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
            let result = reader.revolved(solid, version, &mut entered);
            for _ in 0..entered {
                reader.cx.leave();
            }
            result
        })
    }
}

impl<'f> Reader<'f, '_, '_> {
    fn revolution_axis(&mut self, id: u64) -> Result<RevolutionAxis, Error> {
        self.current = id;
        let record = self.take_typed(id)?;
        if !self.is(record, b"IFCAXIS1PLACEMENT")
            || record.parts().first().map_or(0, |p| p.args().len()) != 2
        {
            return Err(self.invalid("revolution-axis"));
        }
        let location_id = self.target(record, 1)?;
        let location = self.vector(location_id, true)?;
        self.current = id;
        // 省略した Axis の既定の z 軸は XY 平面内にないため、回転軸には使えない。
        let direction_id = self.target(record, 2)?;
        let direction = self.vector(direction_id, false)?;
        self.current = id;
        if location[2] != 0. || direction[2] != 0. {
            return Err(self.invalid("revolution-axis-plane"));
        }
        Ok(RevolutionAxis {
            location,
            direction,
        })
    }

    fn revolved(
        &mut self,
        solid: &'f Record,
        version: Version,
        entered: &mut u64,
    ) -> Result<RevolvedArea<'f>, Error> {
        if !self.is(solid, b"IFCREVOLVEDAREASOLID") {
            return Err(self.unsupported_kind("solid"));
        }
        if solid.parts().first().map_or(0, |p| p.args().len()) != 4 {
            return Err(self.invalid("attributes"));
        }
        self.enter(entered)?;
        let solid_id = solid.id().unwrap_or(0);
        let profile_id = self.target(solid, 1)?;
        self.current = profile_id;
        let profile = self.take_typed(profile_id)?;
        self.enter(entered)?;
        let (area, boundary) = if self.is(profile, b"IFCARBITRARYCLOSEDPROFILEDEF") {
            if profile.parts().first().map_or(0, |p| p.args().len()) != 3 {
                return Err(self.invalid("attributes"));
            }
            if !matches!(arg(profile, 1), Some(Value::Enum(raw)) if self.file.bytes(raw.clone()).eq_ignore_ascii_case(b"AREA"))
            {
                return Err(self.invalid("profile-type"));
            }
            let name = self.profile_name(profile)?;
            let curve_id = self.target(profile, 3)?;
            self.current = curve_id;
            let graph = self.file.curve(curve_id, self.cx)?;
            self.closed(&graph)?;
            (
                AreaProfile {
                    name,
                    position: None,
                    kind: AreaProfileKind::ArbitraryClosed {
                        boundary: graph.root,
                    },
                },
                Some(graph),
            )
        } else {
            (self.parameterized_profile(profile, version, solid)?, None)
        };
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
        let axis_id = self.target(solid, 3)?;
        let axis = self.revolution_axis(axis_id)?;
        self.current = solid_id;
        let value = self.number(arg(solid, 4))?;
        if value < 0. {
            return Err(self.invalid("revolution-angle"));
        }
        let records = core::mem::take(&mut self.records);
        let checked = self.strict_attributes(&records);
        self.records = records;
        checked?;
        let length_unit = self.named_unit(version, solid, false)?;
        let unit = self.named_unit(version, solid, true)?;
        self.current = solid_id;
        if let Some(unit) = &unit {
            let scale = unit
                .scale
                .unwrap_or_else(|| super::super::prefix_scale(unit));
            // 元値には適用しない。有限な上限との比較なので積の overflow は範囲外と判定できる。
            if value * scale > core::f64::consts::TAU {
                return Err(self.invalid("revolution-angle"));
            }
        }
        for (missing, name) in [
            (length_unit.is_none(), "IfcLengthMeasure"),
            (unit.is_none(), "IfcPlaneAngleMeasure"),
        ] {
            if missing {
                self.cx.report(
                    Diagnostic::new(codes::IFC_UNIT_MISSING)
                        .with("type", name)
                        .at(at_bytes(solid.span())),
                );
            }
        }
        self.alloc(1, 768)?;
        Ok(RevolvedArea {
            definition: ProfileRevolution {
                profile: area,
                position,
                axis,
                angle: Measured { value, unit },
            },
            boundary,
            length_unit,
            source: RevolutionSource {
                solid,
                profile,
                records: core::mem::take(&mut self.records),
            },
        })
    }
}
