//! buildingSMART IFC2X3 TC1 / IFC4 ADD2 TC1 / IFC4.3 ADD2 の自然文の属性・使用規則。
//! IfcMaterialDefinitionRepresentation の表現を文脈ごとに保つ。StyledRepresentation は一般の文脈も許す。
//! 材料の StyledItem の Item は省略し、形への直接の指定とは区別する。

use super::{ItemStyleSource, ItemSurfaceStyles, Reader, SurfaceStyleBinding, Version, arg};
use crate::{ContextGeometry, Cx, Error, Form, IfcFile, Kind, Record, Value};

/// 材料の表示の文脈。幾何の文脈でないものも元値を保つ
#[derive(Clone, Debug)]
pub enum MaterialStyleContext<'f> {
    /// 一般の表現の文脈
    General {
        /// 元の識別名
        identifier: Option<String>,
        /// 元の種類の名
        kind: Option<String>,
        /// 元の文脈
        source: &'f Record,
    },
    /// 幾何の文脈。位置と尺度は未適用
    Geometric(Box<ContextGeometry<'f>>),
}

/// 材料の StyledItem。形への直接の指定と混ぜない
#[derive(Clone, Debug)]
pub struct MaterialStyleItem<'f> {
    /// 元の表示の名前
    pub name: Option<String>,
    /// 元の並びで保持する面の表示
    pub styles: Vec<SurfaceStyleBinding<'f>>,
    /// 元の StyledItem。Item は省略されている
    pub source: &'f Record,
    /// 旧版の表示の割り当て
    pub assignments: Vec<&'f Record>,
}

/// 材料の1 つの文脈の表示
#[derive(Clone, Debug)]
pub struct MaterialRepresentation<'f> {
    /// 文脈の元値と出所
    pub context: MaterialStyleContext<'f>,
    /// 元の識別名
    pub identifier: Option<String>,
    /// 元の表現の種類の名
    pub representation_type: Option<String>,
    /// 元の並びの表示の項目
    pub items: Vec<MaterialStyleItem<'f>>,
    /// 元の StyledRepresentation
    pub source: &'f Record,
}

/// 1 つの材料の表示。材料の構成や要素への割り当ては別の参照である
#[derive(Clone, Debug)]
pub struct MaterialPresentation<'f> {
    /// 元の材料
    pub material: &'f Record,
    /// 表示の定義の名前
    pub name: Option<String>,
    /// 表示の定義の説明
    pub description: Option<String>,
    /// 各文脈の表示。定義のない材料では空
    pub representations: Vec<MaterialRepresentation<'f>>,
    /// 元の MaterialDefinitionRepresentation。表示のない材料では None
    pub source: Option<&'f Record>,
}

impl IfcFile {
    /// 材料の表示を、表現の文脈・元の値・出所を保って取得する。
    ///
    /// 材料への逆参照と、表示の文脈・StyledItem・面の表示を同じ Cx で読む。
    /// 表示のない材料は空の成功。未対応は Unsupported、不正と停止はエラーとし部分結果を返さない。
    /// 要素・型・材料の構成への割り当てと、表示する面への適用は、この取得では行わない。
    pub fn material_presentation(
        &self,
        instance: u64,
        cx: &mut Cx,
    ) -> Result<MaterialPresentation<'_>, Error> {
        cx.with_diagnostics_in_record_order(|cx| {
            let mut reader = Reader {
                file: self,
                cx,
                current: instance,
                colours: Vec::new(),
            };
            reader.check(Kind::Work, 1)?;
            let Some(version) = Version::of(self.schema()) else {
                reader.check(Kind::Work, self.schema().len() as u64)?;
                reader.alloc(self.schema().len(), 3)?;
                reader.cx.report(
                    crate::Diagnostic::new(crate::codes::IFC_VERSION_UNSUPPORTED).with(
                        "schema",
                        String::from_utf8_lossy(self.schema()).into_owned(),
                    ),
                );
                return Err(Error::Unsupported);
            };
            reader.within(instance, |reader, record| reader.material(record, version))
        })
    }
}

const REP_BYTES: usize = 2048;
const ITEM_BYTES: usize = 1024;
const _: () = assert!(size_of::<MaterialRepresentation<'static>>() <= REP_BYTES);
const _: () = assert!(size_of::<MaterialStyleItem<'static>>() <= ITEM_BYTES);
const _: () = assert!(size_of::<ContextGeometry<'static>>() <= 512);

impl<'f> Reader<'f, '_, '_> {
    fn material_capacity(&mut self, count: usize, bytes: usize) -> Result<(), Error> {
        if count > i32::MAX as usize / bytes {
            return Err(self.invalid("capacity"));
        }
        self.check(Kind::Work, count as u64)?;
        self.alloc(count, bytes as u64)
    }

    fn material(
        &mut self,
        material: &'f Record,
        version: Version,
    ) -> Result<MaterialPresentation<'f>, Error> {
        if material.form() != Form::Entity || !self.is(material, b"IFCMATERIAL") {
            return Err(self.invalid("material"));
        }
        self.arity(material, if version == Version::Ifc2x3 { 1 } else { 3 })?;
        let mut source = None;
        for record in self.file.records() {
            self.cx
                .check(Kind::Work, 1)
                .map_err(|stop| crate::stopped(self.cx, stop, Some(record.span())))?;
            if record.id().is_none() {
                continue;
            }
            let previous = self.current;
            self.current = record.id().unwrap_or(previous);
            for part in record.parts() {
                self.check(Kind::Work, 1)?;
                if !self
                    .file
                    .bytes(part.name())
                    .eq_ignore_ascii_case(b"IFCMATERIALDEFINITIONREPRESENTATION")
                {
                    continue;
                }
                let at = match part.args().len() {
                    4 => 3,
                    1 if record.parts().len() > 1 => 0,
                    _ => {
                        self.current = record.id().unwrap_or(self.current);
                        return Err(self.invalid("owner-layout"));
                    }
                };
                let target = self.target(part.args().get(at))?;
                if Some(target) == material.id() && source.replace(record).is_some() {
                    self.current = record.id().unwrap_or(self.current);
                    return Err(self.invalid("material-representation-count"));
                }
            }
            self.current = previous;
        }
        let mut result = MaterialPresentation {
            material,
            name: None,
            description: None,
            representations: Vec::new(),
            source,
        };
        let Some(source) = source else {
            return Ok(result);
        };
        self.within(source.id().unwrap_or(0), |reader, record| {
            reader.arity(record, 4)?;
            result.name = reader.text(arg(record, 1), record)?;
            result.description = reader.text(arg(record, 2), record)?;
            let Some(Value::List(representations)) = arg(record, 3) else {
                return Err(reader.invalid("list"));
            };
            if representations.is_empty() {
                return Err(reader.invalid("count"));
            }
            reader.material_capacity(representations.len(), REP_BYTES)?;
            result.representations = Vec::with_capacity(representations.len());
            for representation in representations {
                let id = reader.target(Some(representation))?;
                result.representations.push(
                    reader.within(id, |r, record| r.material_representation(record, version))?,
                );
            }
            Ok(())
        })?;
        Ok(result)
    }

    fn material_context(&mut self, record: &'f Record) -> Result<MaterialStyleContext<'f>, Error> {
        if record.form() != Form::Entity {
            return Err(self.invalid("type"));
        }
        if self.is(record, b"IFCREPRESENTATIONCONTEXT") {
            self.arity(record, 2)?;
            Ok(MaterialStyleContext::General {
                identifier: self.text(arg(record, 1), record)?,
                kind: self.text(arg(record, 2), record)?,
                source: record,
            })
        } else if self.is(record, b"IFCGEOMETRICREPRESENTATIONCONTEXT")
            || self.is(record, b"IFCGEOMETRICREPRESENTATIONSUBCONTEXT")
        {
            let context = self
                .file
                .representation_context(record.id().unwrap_or(0), self.cx)?;
            self.material_capacity(1, 512)?;
            Ok(MaterialStyleContext::Geometric(Box::new(context)))
        } else {
            Err(self.invalid("context"))
        }
    }

    fn material_representation(
        &mut self,
        record: &'f Record,
        version: Version,
    ) -> Result<MaterialRepresentation<'f>, Error> {
        if record.form() != Form::Entity || !self.is(record, b"IFCSTYLEDREPRESENTATION") {
            return Err(self.invalid("material-representation"));
        }
        self.arity(record, 4)?;
        let context_id = self.target(arg(record, 1))?;
        let context = self.within(context_id, |reader, context| {
            reader.material_context(context)
        })?;
        let identifier = self.text(arg(record, 2), record)?;
        let representation_type = self.text(arg(record, 3), record)?;
        let values = self.list(arg(record, 4), usize::MAX)?;
        self.material_capacity(values.len(), ITEM_BYTES)?;
        let mut items = Vec::with_capacity(values.len());
        for value in values {
            let id = self.target(Some(value))?;
            items.push(self.within(id, |reader, styled| {
                if styled.form() != Form::Entity || !reader.is(styled, b"IFCSTYLEDITEM") {
                    return Err(reader.invalid("styled-item"));
                }
                if !matches!(arg(styled, 1), Some(Value::Null)) {
                    return Err(reader.invalid("material-item"));
                }
                let mut values = ItemSurfaceStyles {
                    name: None,
                    styles: Vec::new(),
                    source: ItemStyleSource {
                        item: styled,
                        styled_item: Some(styled),
                        assignments: Vec::new(),
                    },
                };
                reader.styled_values(styled, version, &mut values)?;
                Ok(MaterialStyleItem {
                    name: values.name,
                    styles: values.styles,
                    source: styled,
                    assignments: values.source.assignments,
                })
            })?);
        }
        Ok(MaterialRepresentation {
            context,
            identifier,
            representation_type,
            items,
            source: record,
        })
    }
}
