//! buildingSMART IfcRelAssociatesMaterial の使用規則と IfcRelDefinesByType の属性。
//! 要素の材料が型の材料を上書きする。割り当てを選ぶだけで、構成や表示の評価は行わない。

use super::{Reader, Version, arg};
use crate::building::{Category, category};
use crate::style_owners::Owner;
use crate::{Cx, Error, Form, IfcFile, Kind, Record, Value};

/// 材料の割り当て。複数ある場合も元の並びと出所を保つ
#[derive(Clone, Debug)]
pub struct MaterialAssociation<'f> {
    /// 元の RelAssociatesMaterial
    pub source: &'f Record,
    /// 単一の材料、材料の構成、またはその使用位置
    pub selection: &'f Record,
}

/// 要素または型から選んだ材料の割り当て
#[derive(Clone, Debug)]
pub struct MaterialAssignments<'f> {
    /// 呼び出しで指定した要素
    pub product: &'f Record,
    /// 選んだ割り当ての所有者。要素またはその型
    pub owner: &'f Record,
    /// 型から継承した場合の RelDefinesByType
    pub type_relation: Option<&'f Record>,
    /// 所有者からの割り当て。指定がなければ空
    pub assignments: Vec<MaterialAssociation<'f>>,
}

impl IfcFile {
    /// 要素の材料の割り当てを取得し、指定がないときだけ型を参照する。
    ///
    /// 要素の割り当ての内容が空・未対応でも型へ読み替えない。複数の割り当ては勝者を決めずに保つ。
    /// 同じ Cx で逆参照を調べ、不正・未対応・停止では部分結果を返さない。
    pub fn material_assignments(
        &self,
        instance: u64,
        cx: &mut Cx,
    ) -> Result<MaterialAssignments<'_>, Error> {
        cx.with_diagnostics_in_record_order(|cx| {
            let mut reader = Reader {
                file: self,
                cx,
                current: instance,
                colours: Vec::new(),
            };
            reader.check(Kind::Work, 1)?;
            let Some(_) = Version::of(self.schema()) else {
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
            reader.within(instance, |reader, product| {
                if product.form() != Form::Entity
                    || !matches!(
                        category(self.kind(product), self.schema()),
                        Some(Category::Element | Category::Spatial | Category::Grid)
                    )
                {
                    return Err(reader.invalid("product"));
                }
                let mut result = MaterialAssignments {
                    product,
                    owner: product,
                    type_relation: None,
                    assignments: reader.associations(instance)?,
                };
                if !result.assignments.is_empty() {
                    return Ok(result);
                }
                let relations = reader.assignment_relations(instance, false)?;
                if relations.len() > 1 {
                    return Err(reader.invalid("type-count"));
                }
                let Some(relation) = relations.first().copied() else {
                    return Ok(result);
                };
                reader.within(relation.id().unwrap_or(0), |reader, relation| {
                    reader.arity(relation, 6)?;
                    let target = reader.target(arg(relation, 6))?;
                    reader.within(target, |reader, owner| {
                        if !matches!(
                            category(self.kind(owner), self.schema()),
                            Some(Category::Type | Category::Style)
                        ) {
                            return Err(reader.unsupported(owner));
                        }
                        result.owner = owner;
                        result.type_relation = Some(relation);
                        result.assignments = reader.associations(target)?;
                        Ok(result)
                    })
                })
            })
        })
    }
}

impl<'f> Reader<'f, '_, '_> {
    fn assignment_relations(
        &mut self,
        owner: u64,
        material: bool,
    ) -> Result<Vec<&'f Record>, Error> {
        let (kind, parent, owner_kind) = if material {
            (
                b"IFCRELASSOCIATESMATERIAL".as_slice(),
                b"IFCRELASSOCIATES".as_slice(),
                Owner::Material,
            )
        } else {
            (
                b"IFCRELDEFINESBYTYPE".as_slice(),
                b"IFCRELDEFINES".as_slice(),
                Owner::Type,
            )
        };
        let mut result = Vec::new();
        for record in self.file.owner_records(owner_kind, self.cx)? {
            self.cx
                .check(Kind::Work, 1)
                .map_err(|stop| crate::stopped(self.cx, stop, Some(record.span())))?;
            let Some(id) = record.id() else { continue };
            let previous = self.current;
            self.current = id;
            for part in record.parts() {
                self.check(Kind::Work, 1)?;
                if !self.file.bytes(part.name()).eq_ignore_ascii_case(kind) {
                    continue;
                }
                // 複合の実体も所有者を調べる。関係する複合の実体を見落として型へフォールバックしない。
                let related = if part.args().len() == 6 {
                    part.args().get(4)
                } else if record.parts().len() > 1 {
                    if part.args().len() == 2 {
                        part.args().first()
                    } else if part.args().len() == 1 {
                        let mut related = None;
                        for inherited in record.parts() {
                            self.check(Kind::Work, 1)?;
                            if self
                                .file
                                .bytes(inherited.name())
                                .eq_ignore_ascii_case(parent)
                            {
                                related = match inherited.args().len() {
                                    1 => inherited.args().first(),
                                    5 => inherited.args().get(4),
                                    _ => return Err(self.invalid("owner-layout")),
                                };
                            }
                        }
                        related
                    } else {
                        return Err(self.invalid("owner-layout"));
                    }
                } else {
                    return Err(self.invalid("owner-layout"));
                };
                let members = self.list(related, usize::MAX)?;
                self.check(Kind::Work, members.len() as u64)?;
                if members
                    .iter()
                    .any(|v| matches!(v, Value::Ref(id) if *id == owner))
                {
                    if record.parts().len() != 1 || record.form() != Form::Entity {
                        return Err(self.invalid("type"));
                    }
                    if result.len() >= i32::MAX as usize / 32 {
                        return Err(self.invalid("capacity"));
                    }
                    self.alloc(1, 32)?;
                    result.reserve_exact(1);
                    result.push(record);
                }
            }
            self.current = previous;
        }
        Ok(result)
    }

    fn associations(&mut self, owner: u64) -> Result<Vec<MaterialAssociation<'f>>, Error> {
        let relations = self.assignment_relations(owner, true)?;
        self.material_capacity(relations.len(), 64)?;
        let mut result = Vec::with_capacity(relations.len());
        for relation in relations {
            let assignment = self.within(relation.id().unwrap_or(0), |reader, record| {
                reader.arity(record, 6)?;
                let target = reader.target(arg(record, 6))?;
                reader.within(target, |reader, selection| {
                    if selection.form() != Form::Entity {
                        return Err(reader.invalid("material"));
                    }
                    Ok(MaterialAssociation {
                        source: record,
                        selection,
                    })
                })
            })?;
            result.push(assignment);
        }
        Ok(result)
    }
}

const _: () = assert!(size_of::<MaterialAssociation<'static>>() <= 64);
