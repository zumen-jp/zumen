//! 3 版の IfcMappedItem / IfcRepresentationMap / IfcCartesianTransformationOperator3D と
//! 非一様な派生型の属性の説明に従う。基底軸・尺度の既定値・変換行列は評価しない。

use super::*;
use zumen_building::{CartesianMapping3, MappingScale3, ShapeMapping3};

/// 共有する形の 3D の写像。元の形は借用する表現から別にたどる
#[derive(Clone, Debug)]
pub struct MappedShape3<'f> {
    /// L1 に依存しない配置・軸・尺度
    pub definition: ShapeMapping3,
    /// 原点の元の長さの単位
    pub length_unit: Option<Unit>,
    /// 読んだ元のレコード。意味の値から分ける
    pub source: MappingSource<'f>,
}

/// 共有写像の借用出所。写像元の表現を複製しない
#[derive(Clone, Debug)]
pub struct MappingSource<'f> {
    /// 写像を使う項目
    pub item: &'f Record,
    /// 写像元
    pub map: &'f Record,
    /// 写像元の表現。内容の探索や複製はこの取得では行わない
    pub representation: &'f Record,
    /// 写像先の変換
    pub operator: &'f Record,
    /// 直接読んだ配置・点・方向・単位。読み取り順に同じものも残す
    pub records: Vec<&'f Record>,
}

impl IfcFile {
    /// IfcMappedItem から、3D の写像元の配置と写像先の原点・軸・尺度を必要時に取得する。
    ///
    /// 2D の写像は未対応と診断する。元の比・尺度の省略・単位を保持し、基底軸の構築・
    /// 正規化・鏡映の判定・尺度の既定値・変換は未適用。原点と軸は有限な 3 成分、
    /// 指定した尺度は正を要求する。IFC2X3 の写像先の零方向比は未評価で保持する。
    /// 写像元の表現は借用だけを返す。自己参照・深い共有写像の探索は product_shape() が検査する。
    pub fn mapped_shape3<'f>(
        &'f self,
        node: &ShapeNode<'_>,
        cx: &mut Cx,
    ) -> Result<MappedShape3<'f>, Error> {
        cx.with_diagnostics_in_record_order(|cx| {
            let mut reader = Reader {
                file: self,
                cx,
                current: node.source.id().unwrap_or(0),
                records: Vec::new(),
                shape_diagnostics: true,
            };
            reader.check(Kind::Work, 1)?;
            let item = self
                .instance(reader.current)
                .filter(|r| std::ptr::eq(*r, node.source))
                .ok_or_else(|| reader.invalid("mapping-source"))?;
            if item.form() != crate::Form::Entity || item.parts().len() != 1 {
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
            let result = reader.mapping3(item, version, &mut entered);
            for _ in 0..entered {
                reader.cx.leave();
            }
            result
        })
    }
}

impl<'f> Reader<'f, '_, '_> {
    fn mapping_axis(
        &mut self,
        record: &Record,
        at: usize,
        version: Version,
    ) -> Result<Option<[f64; 3]>, Error> {
        self.current = record.id().unwrap_or(0);
        if matches!(arg(record, at), Some(Value::Null)) {
            return Ok(None);
        }
        self.current = self.target(record, at)?;
        let direction = self.take_typed(self.current)?;
        if !self.is(direction, b"IFCDIRECTION")
            || direction.parts().first().map_or(0, |p| p.args().len()) != 1
        {
            return Err(self.invalid("direction"));
        }
        let Some(Value::List(values)) = arg(direction, 1) else {
            return Err(self.invalid("list"));
        };
        if values.len() != 3 {
            return Err(self.invalid("dimension"));
        }
        let value = [
            self.number(values.first())?,
            self.number(values.get(1))?,
            self.number(values.get(2))?,
        ];
        if version != Version::Ifc2x3 && value == [0.; 3] {
            return Err(self.invalid("direction"));
        }
        Ok(Some(value))
    }
    fn mapping_scale(&mut self, record: &Record, at: usize) -> Result<Option<f64>, Error> {
        self.current = record.id().unwrap_or(0);
        if matches!(arg(record, at), Some(Value::Null)) {
            return Ok(None);
        }
        let value = self.number(arg(record, at))?;
        if value <= 0. {
            return Err(self.invalid("mapping-scale"));
        }
        Ok(Some(value))
    }
    fn mapping3(
        &mut self,
        item: &'f Record,
        version: Version,
        entered: &mut u64,
    ) -> Result<MappedShape3<'f>, Error> {
        if !self.is(item, b"IFCMAPPEDITEM") {
            return Err(self.unsupported_kind("mapping"));
        }
        if item.parts().first().map_or(0, |p| p.args().len()) != 2 {
            return Err(self.invalid("attributes"));
        }
        self.enter(entered)?;
        self.check(Kind::Elements, 1)?;
        self.current = self.target(item, 1)?;
        let map = self.take_typed(self.current)?;
        if !self.is(map, b"IFCREPRESENTATIONMAP")
            || map.parts().first().map_or(0, |p| p.args().len()) != 2
        {
            return Err(self.invalid("mapping-source"));
        }
        self.enter(entered)?;
        self.check(Kind::Elements, 1)?;
        let origin_id = self.target(map, 1)?;
        self.current = origin_id;
        self.check(Kind::Work, 1)?;
        let origin_record = self
            .file
            .instance(origin_id)
            .ok_or_else(|| self.invalid("reference"))?;
        if self.is(origin_record, b"IFCAXIS2PLACEMENT2D") {
            return Err(self.unsupported_kind("mapping-dimension"));
        }
        let origin = self.placement(origin_id)?;
        self.current = map.id().unwrap_or(0);
        self.current = self.target(map, 2)?;
        let representation = self.take_typed(self.current)?;
        if !self.is(representation, b"IFCSHAPEREPRESENTATION") {
            return Err(self.unsupported_kind("representation"));
        }
        self.current = item.id().unwrap_or(0);
        self.current = self.target(item, 2)?;
        let operator = self.take_typed(self.current)?;
        let non_uniform = if self.is(operator, b"IFCCARTESIANTRANSFORMATIONOPERATOR3D") {
            false
        } else if self.is(operator, b"IFCCARTESIANTRANSFORMATIONOPERATOR3DNONUNIFORM") {
            true
        } else {
            return Err(self.unsupported_kind("mapping-operator"));
        };
        if operator.parts().first().map_or(0, |p| p.args().len()) != if non_uniform { 7 } else { 5 }
        {
            return Err(self.invalid("attributes"));
        }
        self.check(Kind::Elements, 1)?;
        let axis1 = self.mapping_axis(operator, 1, version)?;
        let axis2 = self.mapping_axis(operator, 2, version)?;
        self.current = operator.id().unwrap_or(0);
        let origin_id = self.target(operator, 3)?;
        let local_origin = self.vector(origin_id, true)?;
        let x = self.mapping_scale(operator, 4)?;
        let axis3 = self.mapping_axis(operator, 5, version)?;
        let scale = if non_uniform {
            MappingScale3::NonUniform {
                x,
                y: self.mapping_scale(operator, 6)?,
                z: self.mapping_scale(operator, 7)?,
            }
        } else {
            MappingScale3::Uniform(x)
        };
        let records = core::mem::take(&mut self.records);
        let checked = self.strict_attributes(&records);
        self.records = records;
        checked?;
        let (length_unit, units) = face_length_unit(self.file, item, version, self.cx)?;
        self.current = item.id().unwrap_or(0);
        self.alloc(units.len(), 32)?;
        self.records.extend(units);
        if length_unit.is_none() {
            self.cx.report(
                Diagnostic::new(codes::IFC_UNIT_MISSING)
                    .with("type", "IfcLengthMeasure")
                    .at(at_bytes(item.span())),
            );
        }
        self.alloc(1, 512)?;
        Ok(MappedShape3 {
            definition: ShapeMapping3 {
                origin,
                target: CartesianMapping3 {
                    local_origin,
                    axis1,
                    axis2,
                    axis3,
                    scale,
                },
            },
            length_unit,
            source: MappingSource {
                item,
                map,
                representation,
                operator,
                records: core::mem::take(&mut self.records),
            },
        })
    }
}
