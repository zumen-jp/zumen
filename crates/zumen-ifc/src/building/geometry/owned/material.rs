//! 材料の表示を所有し、文脈の同一性を GeometryRef に写す。
//! 材料の構成の属性の拡充は別の lift で扱い、ここでは表示までの参照の経路だけを読む。

use super::*;
use crate::building::{Version, arg, reference as target};
use crate::{Form, MaterialStyleContext, Value};
use zumen_building::{
    GeometryMaterialAssignment, GeometryMaterialBinding, GeometryMaterialContext,
    GeometryMaterialItem, GeometryMaterialMember, GeometryMaterialPresentation,
    GeometryMaterialRepresentation, GeometryMaterials, MaterialAssignmentLevel,
    MaterialContextMatch,
};

const REP_BYTES: usize = 1024;
const MEMBER_BYTES: usize = 512;
const _: () = assert!(size_of::<GeometryMaterialRepresentation>() <= REP_BYTES);
const _: () = assert!(size_of::<GeometryMaterialMember>() <= MEMBER_BYTES);
const _: () = assert!(size_of::<GeometryMaterialAssignment>() <= MEMBER_BYTES);
const _: () = assert!(size_of::<GeometryMaterialItem>() <= MEMBER_BYTES);
const _: () = assert!(size_of::<zumen_building::RepresentationContext>() <= 512);

impl<'f> Own<'f, '_, '_> {
    pub(super) fn materials(
        &mut self,
        product: &'f Record,
        data: &GeometryData,
    ) -> Result<Option<Acquired<GeometryMaterials>>, Error> {
        let assigned = match self
            .file
            .material_assignments(product.id().unwrap_or(0), self.cx)
        {
            Ok(value) => value,
            Err(Error::Unsupported) => return Ok(Some(Acquired::Unsupported)),
            Err(error) => return Err(error),
        };
        if assigned.assignments.is_empty() {
            return Ok(None);
        }
        self.current = assigned.owner;
        let mut source = source_of(assigned.owner);
        self.optional(&mut source, "type_relation", assigned.type_relation)?;
        self.capacity(assigned.assignments.len(), MEMBER_BYTES)?;
        let mut assignments = Vec::with_capacity(assigned.assignments.len());
        for assignment in assigned.assignments {
            self.current = assignment.source;
            let mut source = source_of(assignment.source);
            self.group(&mut source, "selection", &[assignment.selection])?;
            let kind = self.material_kind(assignment.selection)?;
            let members = self.material_members(assignment.selection, data)?;
            assignments.push(GeometryMaterialAssignment {
                kind,
                members,
                source,
            });
        }
        Ok(Some(Acquired::Value(GeometryMaterials {
            level: if assigned.type_relation.is_some() {
                MaterialAssignmentLevel::Type
            } else {
                MaterialAssignmentLevel::Occurrence
            },
            assignments,
            source,
        })))
    }

    fn material_kind(&mut self, record: &Record) -> Result<String, Error> {
        let kind = self.file.kind(record);
        self.capacity(kind.len(), 6)?;
        Ok(String::from_utf8_lossy(kind).to_ascii_uppercase())
    }

    fn material_record(&mut self, id: u64) -> Result<&'f Record, Error> {
        check(self.cx, Some(self.current), Kind::Work, 1)?;
        let record = self.file.instance(id).ok_or_else(|| {
            super::super::invalid(self.cx, Some(self.current), "material-reference")
        })?;
        self.current = record;
        if record.form() != Form::Entity || record.parts().len() != 1 {
            return Err(super::super::invalid(
                self.cx,
                Some(record),
                "material-type",
            ));
        }
        Ok(record)
    }

    fn material_target(&mut self, record: &'f Record, at: usize) -> Result<&'f Record, Error> {
        self.current = record;
        let id = target(arg(record, at))
            .ok_or_else(|| super::super::invalid(self.cx, Some(record), "material-reference"))?;
        self.material_record(id)
    }

    fn material_arity(&mut self, record: &Record, old: usize, new: usize) -> Result<(), Error> {
        check(self.cx, Some(record), Kind::Work, 1)?;
        let expected = if Version::of(self.file.schema()) == Some(Version::Ifc2x3) {
            old
        } else {
            new
        };
        if record.parts().first().map(|p| p.args().len()) != Some(expected) {
            return Err(super::super::invalid(
                self.cx,
                Some(record),
                "material-arity",
            ));
        }
        Ok(())
    }

    fn material_list(&mut self, record: &'f Record) -> Result<&'f [Value], Error> {
        let Some(Value::List(values)) = arg(record, 1) else {
            return Err(super::super::invalid(
                self.cx,
                Some(record),
                "material-list",
            ));
        };
        if values.is_empty() {
            return Err(super::super::invalid(
                self.cx,
                Some(record),
                "material-count",
            ));
        }
        self.current = record;
        self.capacity(values.len(), MEMBER_BYTES)?;
        Ok(values)
    }

    fn material_members(
        &mut self,
        selection: &'f Record,
        data: &GeometryData,
    ) -> Result<Vec<GeometryMaterialMember>, Error> {
        let selection = self.material_record(selection.id().unwrap_or(0))?;
        // IfcMaterialList.Materials と IfcMaterialLayerSet.MaterialLayers は LIST。重複と順序を保つ。
        if self
            .file
            .kind(selection)
            .eq_ignore_ascii_case(b"IFCMATERIALLIST")
        {
            self.material_arity(selection, 1, 1)?;
            let values = self.material_list(selection)?;
            let mut members = Vec::with_capacity(values.len());
            for value in values {
                self.current = selection;
                let id = target(Some(value)).ok_or_else(|| {
                    super::super::invalid(self.cx, Some(selection), "material-reference")
                })?;
                let material = self.material_record(id)?;
                members.push(self.material_member(material, &[selection], data)?);
            }
            return Ok(members);
        }
        let mut set = selection;
        let usage = self
            .file
            .kind(selection)
            .eq_ignore_ascii_case(b"IFCMATERIALLAYERSETUSAGE");
        if usage {
            self.material_arity(selection, 4, 5)?;
            set = self.material_target(selection, 1)?;
            if !self
                .file
                .kind(set)
                .eq_ignore_ascii_case(b"IFCMATERIALLAYERSET")
            {
                return Err(super::super::invalid(
                    self.cx,
                    Some(set),
                    "material-layer-set",
                ));
            }
        }
        if self
            .file
            .kind(set)
            .eq_ignore_ascii_case(b"IFCMATERIALLAYERSET")
        {
            self.material_arity(set, 2, 3)?;
            let values = self.material_list(set)?;
            let mut members = Vec::with_capacity(values.len());
            for value in values {
                self.current = set;
                let id = target(Some(value)).ok_or_else(|| {
                    super::super::invalid(self.cx, Some(set), "material-reference")
                })?;
                let layer = self.material_record(id)?;
                let route = [selection, set, layer];
                let route = if usage { &route[..] } else { &route[1..] };
                if !self
                    .file
                    .kind(layer)
                    .eq_ignore_ascii_case(b"IFCMATERIALLAYER")
                {
                    members.push(self.material_member(layer, route, data)?);
                    continue;
                }
                self.material_arity(layer, 3, 7)?;
                if matches!(arg(layer, 1), Some(Value::Null)) {
                    let mut source = source_of(layer);
                    self.group(&mut source, "route", route)?;
                    members.push(GeometryMaterialMember {
                        kind: self.material_kind(layer)?,
                        presentation: None,
                        source,
                    });
                } else {
                    let material = self.material_target(layer, 1)?;
                    members.push(self.material_member(material, route, data)?);
                }
            }
            return Ok(members);
        }
        self.capacity(1, MEMBER_BYTES)?;
        Ok(vec![self.material_member(selection, &[], data)?])
    }

    fn material_member(
        &mut self,
        material: &'f Record,
        route: &[&Record],
        data: &GeometryData,
    ) -> Result<GeometryMaterialMember, Error> {
        self.current = material;
        let mut source = source_of(material);
        self.group(&mut source, "route", route)?;
        let kind = self.material_kind(material)?;
        let presentation = if kind == "IFCMATERIAL" {
            match self
                .file
                .material_presentation(material.id().unwrap_or(0), self.cx)
            {
                Ok(value) => Acquired::Value(self.material_presentation(value, data)?),
                Err(Error::Unsupported) => Acquired::Unsupported,
                Err(error) => return Err(error),
            }
        } else if matches!(
            kind.as_str(),
            "IFCMATERIALLIST"
                | "IFCMATERIALLAYER"
                | "IFCMATERIALLAYERSET"
                | "IFCMATERIALLAYERSETUSAGE"
        ) {
            return Err(super::super::invalid(
                self.cx,
                Some(material),
                "material-type",
            ));
        } else if kind.starts_with("IFCMATERIAL") {
            self.capacity(kind.len(), 1)?;
            self.cx.report(
                crate::Diagnostic::new(crate::codes::IFC_STYLE_UNSUPPORTED)
                    .with("kind", kind.clone())
                    .at(crate::at_bytes(material.span())),
            );
            Acquired::Unsupported
        } else {
            return Err(super::super::invalid(
                self.cx,
                Some(material),
                "material-type",
            ));
        };
        Ok(GeometryMaterialMember {
            kind,
            presentation: Some(presentation),
            source,
        })
    }

    fn material_bindings(
        &mut self,
        context: &Record,
        data: &GeometryData,
    ) -> Result<Vec<GeometryMaterialBinding>, Error> {
        let mut bindings = Vec::new();
        for (at, node) in data.nodes.iter().enumerate() {
            check(self.cx, Some(self.current), Kind::Work, 1)?;
            if !matches!(node.links, GeometryLinks::Representation { .. }) {
                continue;
            }
            let mut exact = false;
            let mut parent = false;
            for group in &node.source.groups {
                check(
                    self.cx,
                    Some(self.current),
                    Kind::Work,
                    (group.instances.len() as u64) + 1,
                )?;
                let matches = group.instances.iter().any(|id| Some(*id) == context.id());
                exact |= group.role == "context" && matches;
                parent |= group.role == "parent" && matches;
            }
            if exact || parent {
                self.capacity(1, 16)?;
                bindings.reserve_exact(1);
                bindings.push(GeometryMaterialBinding {
                    representation: GeometryRef(at as u32),
                    context: if exact {
                        MaterialContextMatch::Exact
                    } else {
                        MaterialContextMatch::Parent
                    },
                });
            }
        }
        Ok(bindings)
    }

    fn material_presentation(
        &mut self,
        value: crate::MaterialPresentation<'f>,
        data: &GeometryData,
    ) -> Result<GeometryMaterialPresentation, Error> {
        self.current = value.material;
        let mut source = source_of(value.material);
        self.optional(&mut source, "definition", value.source)?;
        self.capacity(value.representations.len(), REP_BYTES)?;
        let mut representations = Vec::with_capacity(value.representations.len());
        for rep in value.representations {
            self.current = rep.source;
            let mut source = source_of(rep.source);
            let (context, record) = match rep.context {
                MaterialStyleContext::General {
                    identifier,
                    kind,
                    source,
                } => (
                    GeometryMaterialContext::General { identifier, kind },
                    source,
                ),
                MaterialStyleContext::Geometric(value) => {
                    self.optional(&mut source, "parent", value.source.parent)?;
                    self.group(&mut source, "records", &value.source.records)?;
                    self.capacity(1, 512)?;
                    (
                        GeometryMaterialContext::Geometric {
                            definition: Box::new(value.definition),
                            length_unit: value.length_unit,
                        },
                        value.source.context,
                    )
                }
            };
            self.group(&mut source, "context", &[record])?;
            let bindings = self.material_bindings(record, data)?;
            self.capacity(rep.items.len(), MEMBER_BYTES)?;
            let mut items = Vec::with_capacity(rep.items.len());
            for item in rep.items {
                self.current = item.source;
                let mut source = source_of(item.source);
                self.group(&mut source, "assignments", &item.assignments)?;
                items.push(GeometryMaterialItem {
                    name: item.name,
                    surfaces: self.surfaces(item.styles)?,
                    source,
                });
            }
            representations.push(GeometryMaterialRepresentation {
                context,
                identifier: rep.identifier,
                kind: rep.representation_type,
                items,
                bindings,
                source,
            });
        }
        Ok(GeometryMaterialPresentation {
            name: value.name,
            description: value.description,
            representations,
            source,
        })
    }
}

impl Lifter<'_> {
    pub(super) fn materials_used(
        &mut self,
        materials: Option<&Acquired<GeometryMaterials>>,
        cx: &mut Cx,
    ) -> Result<(), Error> {
        let Some(Acquired::Value(materials)) = materials else {
            return Ok(());
        };
        // 型と構成は参照しただけなので、属性を lift したと数えない。
        for group in &materials.source.groups {
            for &id in &group.instances {
                self.geometry_record_used(id, cx)?
            }
        }
        for assignment in &materials.assignments {
            check(cx, None, Kind::Work, 1)?;
            if let Some(id) = assignment.source.instance {
                self.geometry_record_used(id, cx)?
            }
            for member in &assignment.members {
                check(cx, None, Kind::Work, 1)?;
                let Some(Acquired::Value(p)) = &member.presentation else {
                    continue;
                };
                self.geometry_source_used(&p.source, cx)?;
                for rep in &p.representations {
                    self.geometry_source_used(&rep.source, cx)?;
                    for item in &rep.items {
                        self.geometry_source_used(&item.source, cx)?;
                        for surface in &item.surfaces {
                            self.geometry_source_used(&surface.source, cx)?
                        }
                    }
                }
            }
        }
        Ok(())
    }
}
